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
    _locks: Vec<fs::File>,
}
impl Printer {
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
            _locks: locks,
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
    fn identity(&self) -> serde_json::Value {
        serde_json::json!({"host":self.host.as_str()})
    }
    async fn png(&mut self, page: &Page, repeat: bool) -> Result<Vec<u8>> {
        let zpl = page.zpl()?;
        let request_hash = hash(zpl.as_bytes());
        let key = hash(format!("{}:{request_hash}:{repeat}", self.host).as_bytes());
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
        if self.preview_object.is_none() {
            self.preview_object = Some(zebra_http_api::PreviewObject::new()?);
        }
        let png = zebra_http_api::zpl_to_png_with_object(
            self.client.clone(),
            self.host.clone(),
            &zpl,
            &self.username,
            &self.password,
            self.preview_object.as_ref().unwrap(),
        )
        .await?;
        ensure!(
            png.starts_with(b"\x89PNG\r\n\x1a\n"),
            "preview response is not PNG"
        );
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
