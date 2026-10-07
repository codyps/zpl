//! Printer transport and content-addressed, integrity-checked raw evidence cache.
use super::{model::*, probe::Page};
use eyre::{ensure, Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

#[allow(async_fn_in_trait)]
pub trait Capture {
    fn identity(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
    fn checkpoint(&mut self, _collection: &crate::collection::Collection) -> Result<()> {
        Ok(())
    }
    /// A repeat must be a separate printer transaction, never the first image's cache entry.
    async fn png(&mut self, page: &Page, repeat: bool) -> Result<Vec<u8>>;
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Receipt {
    schema: String,
    host: String,
    request_sha256: String,
    png_sha256: String,
    repeat: bool,
}
pub struct Printer {
    client: reqwest::Client,
    preview_object: Option<zebra_http_api::PreviewObject>,
    host: reqwest::Url,
    cache: PathBuf,
    offline: bool,
    delay: Duration,
    username: String,
    password: String,
    last: Option<std::time::Instant>,
    profile: Option<serde_json::Value>,
    _locks: Vec<fs::File>,
    restart: Option<(u16, Duration)>,
}
impl Printer {
    /// Scope evidence/cache keys to a separately obtained printer identity.
    pub fn with_identity(mut self, profile: serde_json::Value) -> Self {
        self.profile = Some(profile);
        self
    }
    /// Explicitly authorize bounded automatic reboot on transport timeout.
    pub fn with_restart(mut self, port: u16, wait: u64) -> Result<Self> {
        ensure!(
            port != 0 && (10..=1200).contains(&wait),
            "invalid restart port/wait"
        );
        self.restart = Some((port, Duration::from_secs(wait)));
        Ok(self)
    }
    async fn preview(&mut self, zpl: &str) -> Result<Vec<u8>> {
        if self.preview_object.is_none() {
            self.preview_object = Some(zebra_http_api::PreviewObject::new()?);
        }
        zebra_http_api::zpl_to_png_with_object(
            self.client.clone(),
            self.host.clone(),
            zpl,
            &self.username,
            &self.password,
            self.preview_object.as_ref().unwrap(),
        )
        .await
    }
    /// Verify HTTP before directory discovery, recovering an existing hang when authorized.
    pub async fn ensure_ready(&mut self) -> Result<()> {
        if self.offline {
            return Ok(());
        }
        let folder = self.cache.join("startup-control");
        let png = self
            .preview_with_recovery("^XA^PW832^LL256^LH0,0^FO20,20^GB20,20,20^FS^XZ", &folder)
            .await?;
        validate_control(&png)?;
        fs::create_dir_all(&folder)?;
        atomic_write(&folder.join("preview.png"), &png)
    }
    async fn preview_with_recovery(&mut self, zpl: &str, folder: &Path) -> Result<Vec<u8>> {
        let request_hash = hash(zpl.as_bytes());
        let recovery_path = folder.join("recovery.json");
        let previous_attempts = if recovery_path.exists() {
            serde_json::from_slice::<serde_json::Value>(&read(&recovery_path)?)?["attempts"]
                .as_u64()
                .unwrap_or(2)
        } else {
            0
        };
        ensure!(
            previous_attempts < 2,
            "quarantined preview page: {}",
            folder.display()
        );
        let mut attempts = previous_attempts;
        let png = loop {
            match self.preview(zpl).await {
                Ok(png) => break png,
                Err(error) => {
                    let recoverable = error
                        .downcast_ref::<reqwest::Error>()
                        .is_some_and(|e| e.is_timeout() || e.is_connect());
                    let Some((port, wait)) = self.restart.filter(|_| recoverable) else {
                        return Err(error);
                    };
                    let profile = self
                        .profile
                        .clone()
                        .ok_or_else(|| eyre::eyre!("restart requires printer identity"))?;
                    attempts += 1;
                    fs::create_dir_all(folder)?;
                    // Persist before reset; uncertain writes cannot cause unbounded retries on resume.
                    write_json(
                        &recovery_path,
                        &serde_json::json!({"attempts":attempts,"request_sha256":request_hash,"phase":"reset-attempted"}),
                    )?;
                    let cooldown = self.cache.join("last-reset.json");
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)?
                        .as_secs();
                    if cooldown.exists() {
                        let previous: u64 = serde_json::from_slice(&read(&cooldown)?)?;
                        tokio::time::sleep(Duration::from_secs(
                            60u64.saturating_sub(now.saturating_sub(previous)),
                        ))
                        .await;
                    }
                    write_json(
                        &cooldown,
                        &std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)?
                            .as_secs(),
                    )?;
                    eprintln!(
                        "Preview stalled; rebooting via JSON port {port} (attempt {attempts}/2)"
                    );
                    super::restart::recover(&self.client, &self.host, port, &profile, wait).await?;
                    self.preview_object = None;
                    let control = self
                        .preview("^XA^PW832^LL256^LH0,0^FO20,20^GB20,20,20^FS^XZ")
                        .await?;
                    validate_control(&control)?;
                    atomic_write(&folder.join("restart-control.png"), &control)?;
                    write_json(
                        &recovery_path,
                        &serde_json::json!({"attempts":attempts,"request_sha256":request_hash,"phase":"control-verified","control_sha256":hash(&control)}),
                    )?;
                    ensure!(
                        attempts < 2,
                        "repeatedly hanging preview quarantined after restoring printer: {}",
                        folder.display()
                    );
                }
            }
        };
        ensure!(
            png.starts_with(b"\x89PNG\r\n\x1a\n"),
            "preview response is not PNG"
        );
        Ok(png)
    }
    pub fn new(host: &str, cache: &Path, offline: bool, delay: f64, timeout: u64) -> Result<Self> {
        ensure!(
            delay.is_finite() && (0.0..=60.0).contains(&delay) && (1..=60).contains(&timeout),
            "delay must be 0..60 and timeout 1..60 seconds"
        );
        let host = reqwest::Url::parse(host)?;
        ensure!(
            ["http", "https"].contains(&host.scheme())
                && host.host_str().is_some()
                && host.username().is_empty()
                && host.password().is_none()
                && host.query().is_none()
                && host.fragment().is_none()
                && host.path() == "/",
            "host must be an HTTP(S) origin without credentials/path/query"
        );
        fs::create_dir_all(cache)?;
        let mut locks = vec![lock(&cache.join("capture.lock"))?];
        if !offline {
            locks.push(lock(&std::env::temp_dir().join(format!(
                "zpl-preview-{}.lock",
                hash(host.as_str().as_bytes())
            )))?);
        }
        let client = reqwest::Client::builder()
            .http1_title_case_headers()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(timeout))
            .build()?;
        Ok(Self {
            client,
            preview_object: None,
            host,
            cache: cache.into(),
            offline,
            delay: Duration::from_secs_f64(delay),
            username: std::env::var("ZPL_USERNAME").unwrap_or_default(),
            password: std::env::var("ZPL_PASSWORD").unwrap_or_default(),
            last: None,
            profile: None,
            _locks: locks,
            restart: None,
        })
    }
}
fn lock(path: &Path) -> Result<fs::File> {
    let f = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)?;
    f.try_lock()
        .wrap_err("another extraction is using this printer or cache")?;
    Ok(f)
}
impl Capture for Printer {
    fn checkpoint(&mut self, collection: &crate::collection::Collection) -> Result<()> {
        let mut c = collection.clone();
        c.seal()?;
        c.validate()?;
        write_json(&self.cache.join("recovery.json"), &c)
    }
    fn identity(&self) -> serde_json::Value {
        match &self.profile {
            Some(profile) => serde_json::json!({"host":self.host.as_str(),"profile":profile}),
            None => serde_json::json!({"host":self.host.as_str()}),
        }
    }
    async fn png(&mut self, page: &Page, repeat: bool) -> Result<Vec<u8>> {
        let zpl = page.zpl()?;
        let request_hash = hash(zpl.as_bytes());
        let scope = match &self.profile {
            Some(profile) => format!("{}:{}", self.host, hash(&serde_json::to_vec(profile)?)),
            None => self.host.to_string(),
        };
        let key = hash(format!("{scope}:{request_hash}:{repeat}").as_bytes());
        let folder = self.cache.join(key);
        let receipt_path = folder.join("capture.json");
        if receipt_path.exists() {
            let r: Receipt = serde_json::from_slice(&read(&receipt_path)?)?;
            let png = read(&folder.join("preview.png"))?;
            ensure!(
                r.schema == "zpl-preview-cache-v1"
                    && r.host == self.host.as_str()
                    && r.repeat == repeat
                    && r.request_sha256 == request_hash
                    && r.png_sha256 == hash(&png)
                    && read(&folder.join("request.zpl"))? == zpl.as_bytes(),
                "cached preview integrity mismatch: {}",
                folder.display()
            );
            return Ok(png);
        }
        ensure!(
            !self.offline,
            "missing cached preview for {} {} page {} (repeat={repeat})",
            page.font,
            page.stage,
            page.number
        );
        if let Some(last) = self.last {
            tokio::time::sleep(self.delay.saturating_sub(last.elapsed())).await;
        }
        eprintln!(
            "{} {} page {}{}",
            page.font,
            page.stage,
            page.number,
            if repeat { " repeat" } else { "" }
        );
        self.last = Some(std::time::Instant::now());
        let png = self.preview_with_recovery(&zpl, &folder).await?;
        // The receipt is committed last, so interrupted writes are never cache hits.
        fs::create_dir_all(&folder)?;
        atomic_write(&folder.join("request.zpl"), zpl.as_bytes())?;
        atomic_write(&folder.join("preview.png"), &png)?;
        write_json(&folder.join("plan.json"), page)?;
        write_json(
            &receipt_path,
            &Receipt {
                schema: "zpl-preview-cache-v1".into(),
                host: self.host.to_string(),
                request_sha256: request_hash,
                png_sha256: hash(&png),
                repeat,
            },
        )?;
        Ok(png)
    }
}

fn validate_control(control: &[u8]) -> Result<()> {
    let raster = raster_diff::Raster::decode_png_with_threshold(control, None)
        .map_err(|e| eyre::eyre!(e))?;
    ensure!(
        raster.width == 832 && raster.height == 256,
        "restart control dimensions differ"
    );
    for y in 0..256usize {
        for x in 0..832usize {
            ensure!(
                raster.pixels[y * 832 + x]
                    == if (20..40).contains(&x) && (20..40).contains(&y) {
                        0
                    } else {
                        255
                    },
                "restart control pixels differ"
            );
        }
    }
    Ok(())
}
