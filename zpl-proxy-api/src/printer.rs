//! Exclusive, named printer ownership and bounded recovery.
//!
//! Wire commands: Zebra Programming Guide, SGD getvar/device.unique_id/appl.name,
//! device.reset p.755; ^MC p.300; ^CI pp.155-157; ^CF/^BY/^FW and layout commands.
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use crate::{
    cache::{self, Attempt, Cache},
    telemetry,
    validation::RenderZpl,
};
use serde::Deserialize;
use std::{sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::{Mutex, Semaphore},
    time::{timeout, Instant},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub serial: String,
    pub firmware: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrinterConfig {
    pub url: String,
    /// Trusted operator endpoint for SGD identity/restart, never supplied by a request.
    pub control_address: String,
    pub width: u16,
    pub height: u16,
    #[serde(default)]
    pub headers: Vec<String>,
    #[serde(default)]
    pub serial: Option<String>,
}

#[derive(Debug)]
pub struct Unavailable;
impl std::fmt::Display for Unavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Printer unavailable; retry later")
    }
}
impl std::error::Error for Unavailable {}
#[derive(Debug)]
pub struct Rejected {
    pub cached: bool,
}
impl std::fmt::Display for Rejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Label repeatedly failed on this printer firmware")
    }
}
impl std::error::Error for Rejected {}

#[derive(Clone, Default)]
pub struct Ownership(Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>);

#[derive(Clone)]
pub struct Printer(Arc<Inner>);
struct Inner {
    name: String,
    config: PrinterConfig,
    client: reqwest::Client,
    url: reqwest::Url,
    cache: Cache,
    key: Vec<u8>,
    state: Mutex<PrinterState>,
    slots: Arc<Semaphore>,
    timing: Timing,
    owners: Ownership,
}
#[derive(Default)]
struct PrinterState {
    identity: Option<Identity>,
    initialized: bool,
    recovering: bool,
    retry_at: Option<Instant>,
}
#[derive(Clone)]
struct Timing {
    io: Duration,
    boot: Duration,
    settle: Duration,
    poll: Duration,
    cooldown: Duration,
}
impl Default for Timing {
    fn default() -> Self {
        Self {
            io: Duration::from_secs(30),
            boot: Duration::from_secs(120),
            settle: Duration::from_secs(10),
            poll: Duration::from_secs(2),
            cooldown: Duration::from_secs(300),
        }
    }
}

impl Printer {
    pub fn new(
        name: String,
        config: PrinterConfig,
        cache: Cache,
        namespace: &str,
        owners: Ownership,
    ) -> eyre::Result<Self> {
        Self::with_timing(name, config, cache, namespace, Timing::default(), owners)
    }
    fn with_timing(
        name: String,
        config: PrinterConfig,
        cache: Cache,
        namespace: &str,
        timing: Timing,
        owners: Ownership,
    ) -> eyre::Result<Self> {
        eyre::ensure!(
            !name.is_empty()
                && name.len() <= 80
                && name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)),
            "printer name must contain 1-80 ASCII letters, digits, '.', '_' or '-'"
        );
        eyre::ensure!(
            (8..=32000).contains(&config.width) && (8..=32000).contains(&config.height),
            "invalid printer canvas"
        );
        let url: reqwest::Url = config.url.parse()?;
        eyre::ensure!(
            matches!(url.scheme(), "http" | "https")
                && url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.path() == "/"
                && url.query().is_none()
                && url.fragment().is_none(),
            "printer URL must be an HTTP(S) origin without credentials"
        );
        let mut headers = reqwest::header::HeaderMap::new();
        for header in &config.headers {
            let (name, value) = header
                .split_once(':')
                .ok_or_else(|| eyre::eyre!("invalid printer header"))?;
            headers.insert(
                reqwest::header::HeaderName::from_bytes(name.as_bytes())?,
                value.trim().parse()?,
            );
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timing.io)
            .http1_title_case_headers()
            .default_headers(headers)
            .build()?;
        let namespace = format!(
            "managed-v1:{namespace}:{}:{}:{}",
            name, config.width, config.height
        );
        let key = cache::renderer_key(url.as_str(), &config.headers, &namespace);
        Ok(Self(Arc::new(Inner {
            name,
            config,
            client,
            url,
            cache,
            key,
            state: Mutex::new(PrinterState::default()),
            slots: Arc::new(Semaphore::new(8)),
            timing,
            owners,
        })))
    }
    pub fn name(&self) -> &str {
        &self.0.name
    }

    /// Admission is bounded before spawning disconnect-independent work. Every
    /// printer has its own queue; recovering one never locks another printer.
    pub async fn render(&self, input: String, refresh: bool) -> eyre::Result<(Vec<u8>, bool)> {
        let permit = self
            .0
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| Unavailable)?;
        let zpl =
            telemetry::spawn_blocking("render.validate", move || RenderZpl::parse(input)).await??;
        let this = self.clone();
        telemetry::spawn("printer.task", async move {
            let _permit = permit;
            this.render_owned(zpl, refresh).await
        })
        .await?
    }

    async fn render_owned(&self, zpl: RenderZpl, refresh: bool) -> eyre::Result<(Vec<u8>, bool)> {
        let mut state = self.0.state.lock().await;
        let mut attempt = self
            .0
            .cache
            .begin(zpl.as_str().as_bytes().to_vec(), self.0.key.clone(), true)
            .await?;
        self.0
            .cache
            .name_request(attempt.request_id, self.name().to_owned())
            .await?;
        let result = self
            .execute(&mut state, &mut attempt, zpl.as_str(), refresh)
            .await;
        match &result {
            Ok((png, false)) => self.0.cache.success(attempt, png.clone()).await?,
            Ok((_, true)) => {}
            Err(e) if e.downcast_ref::<Rejected>().is_some_and(|e| e.cached) => {}
            Err(e) => self.0.cache.failure(attempt, e.to_string()).await?,
        }
        result
    }

    async fn execute(
        &self,
        state: &mut PrinterState,
        attempt: &mut Attempt,
        zpl: &str,
        refresh: bool,
    ) -> eyre::Result<(Vec<u8>, bool)> {
        if state.recovering || state.retry_at.is_some_and(|when| Instant::now() < when) {
            return Err(Unavailable.into());
        }
        let identity = match self.identity().await {
            Ok(identity) => identity,
            Err(error) => {
                // A preview hang can also stall SGD. Recover using the last
                // observed identity, but require a fresh matching identity before
                // any preview or cache hit. Never fabricate firmware/serial data.
                let previous = match &state.identity {
                    Some(identity) => Some(identity.clone()),
                    None => self.0.cache.last_identity(self.name().into()).await?,
                };
                if let Some(previous) = previous {
                    self.claim_identity(&previous)?;
                    let id = self
                        .0
                        .cache
                        .preview_started(
                            attempt.request_id,
                            self.name().into(),
                            previous.clone(),
                            "identity-last-known",
                        )
                        .await?;
                    let kind = if error
                        .downcast_ref::<tokio::time::error::Elapsed>()
                        .is_some()
                    {
                        "hang"
                    } else {
                        "unavailable"
                    };
                    self.0
                        .cache
                        .preview_finished(
                            id,
                            Some(
                                "Live identity unavailable; recovery uses last observed identity"
                                    .into(),
                            ),
                            Some(kind),
                        )
                        .await?;
                    self.schedule_recovery(state, attempt, &previous, false);
                } else {
                    state.retry_at = Some(Instant::now() + self.0.timing.cooldown);
                }
                return Err(Unavailable.into());
            }
        };
        if self
            .0
            .config
            .serial
            .as_ref()
            .is_some_and(|serial| serial != &identity.serial)
            || state
                .identity
                .as_ref()
                .is_some_and(|old| old.serial != identity.serial)
        {
            return Err(eyre::eyre!(
                "Printer serial changed; operator intervention required"
            ));
        }
        self.claim_identity(&identity)?;
        if state.identity.as_ref() != Some(&identity) {
            state.initialized = false;
        }
        state.identity = Some(identity.clone());
        let key = cache::renderer_key(
            &identity.serial,
            std::slice::from_ref(&identity.firmware),
            &hex(&self.0.key),
        );
        let (resolved, error) = self
            .0
            .cache
            .resolve_printer(attempt.clone(), identity.clone(), key, refresh)
            .await?;
        *attempt = resolved;
        if error.is_some() {
            return Err(Rejected { cached: true }.into());
        }
        if let Some(png) = &attempt.cached_png {
            return Ok((png.clone(), true));
        }
        // First-use control clears residual preview state and verifies the HTTP
        // engine before accepting a user label. It never sends a physical print.
        if !state.initialized {
            if self
                .preview(attempt, &identity, "initialize", CONTROL)
                .await?
                .is_err()
            {
                self.recover(state, attempt, &identity, false).await?;
            }
            state.initialized = true;
        }
        let first = self.preview(attempt, &identity, "label", zpl).await?;
        let first_error = match first {
            Ok(png) => return Ok((png, false)),
            Err(error) => error,
        };
        // A successful control distinguishes label rejection from global failure.
        if first_error == PreviewFailure::Timeout
            || self
                .preview(attempt, &identity, "control", CONTROL)
                .await?
                .is_err()
        {
            self.recover(state, attempt, &identity, false).await?;
        }
        let retry = self.preview(attempt, &identity, "retry", zpl).await?;
        let retry_error = match retry {
            Ok(png) => return Ok((png, false)),
            Err(error) => error,
        };
        if retry_error == PreviewFailure::Timeout
            || self
                .preview(attempt, &identity, "control-after-retry", CONTROL)
                .await?
                .is_err()
        {
            // Repeated hangs also honor the five-minute restart limit. If a
            // restart must wait, background recovery quarantines the reproducible
            // label only after the printer produces a healthy control again.
            let quarantine =
                first_error == retry_error && first_error != PreviewFailure::Unavailable;
            self.recover(state, attempt, &identity, quarantine).await?;
        }
        // Network errors, authorization failures and malformed PNGs are never
        // durable label verdicts. Two matching rejections or reproduced timeouts
        // with healthy controls can be quarantined for this identity/configuration.
        if first_error != retry_error || first_error == PreviewFailure::Unavailable {
            return Err(Unavailable.into());
        }
        self.0
            .cache
            .quarantine(attempt, PERMANENT_ERROR.into())
            .await?;
        Err(Rejected { cached: false }.into())
    }

    fn claim_identity(&self, identity: &Identity) -> eyre::Result<()> {
        eyre::ensure!(
            self.0
                .config
                .serial
                .as_ref()
                .is_none_or(|serial| serial == &identity.serial),
            "Configured serial does not match observed identity"
        );
        // DNS aliases or separate interfaces must not create two preview
        // locks for the same physical printer.
        let mut owners = self
            .0
            .owners
            .0
            .lock()
            .map_err(|_| eyre::eyre!("Printer ownership lock poisoned"))?;
        let owner = owners
            .entry(identity.serial.clone())
            .or_insert_with(|| self.name().into());
        eyre::ensure!(
            owner == self.name(),
            "Printer serial is already assigned to another name"
        );
        Ok(())
    }

    async fn recover(
        &self,
        state: &mut PrinterState,
        attempt: &Attempt,
        identity: &Identity,
        quarantine: bool,
    ) -> eyre::Result<()> {
        // Durable cooldown prevents public requests or process restarts from
        // creating a reboot loop. The reservation covers this bounded cycle.
        if !self
            .0
            .cache
            .reserve_recovery(recovery_key(identity), self.0.timing.cooldown.as_secs())
            .await?
        {
            self.schedule_recovery(state, attempt, identity, quarantine);
            return Err(Unavailable.into());
        }
        if self.reboot_and_control(attempt, identity).await.is_err() {
            self.schedule_recovery(state, attempt, identity, quarantine);
            return Err(Unavailable.into());
        }
        Ok(())
    }

    fn schedule_recovery(
        &self,
        state: &mut PrinterState,
        attempt: &Attempt,
        identity: &Identity,
        quarantine: bool,
    ) {
        state.initialized = false;
        if state.recovering {
            return;
        }
        state.recovering = true;
        let this = self.clone();
        let attempt = attempt.clone();
        let identity = identity.clone();
        // Explicitly authorized: these are exclusively owned preview devices.
        // Keep recovering until previews work, but never restart more than once
        // per durable cooldown. A healthy control avoids an unnecessary reboot.
        telemetry::spawn("printer.background_recovery", async move {
            loop {
                tokio::time::sleep(this.0.timing.cooldown).await;
                let mut state = this.0.state.lock().await;
                if let Ok(current) = this.identity().await {
                    if current != identity {
                        // A new firmware or replacement must be evaluated under
                        // its own cache scope, never quarantined by an old verdict.
                        state.recovering = false;
                        state.retry_at = None;
                        return;
                    }
                    let control = this
                        .preview(&attempt, &identity, "background-control", CONTROL)
                        .await;
                    if control.is_err() {
                        continue;
                    } // No reboot on persistence failure.
                    if matches!(control, Ok(Ok(_))) {
                        if quarantine
                            && this
                                .0
                                .cache
                                .quarantine(&attempt, PERMANENT_ERROR.into())
                                .await
                                .is_err()
                        {
                            continue;
                        }
                        state.recovering = false;
                        state.initialized = true;
                        state.retry_at = None;
                        return;
                    }
                }
                match this
                    .0
                    .cache
                    .reserve_recovery(recovery_key(&identity), this.0.timing.cooldown.as_secs())
                    .await
                {
                    Ok(true) => {}
                    _ => continue,
                }
                if this.reboot_and_control(&attempt, &identity).await.is_ok() {
                    if quarantine
                        && this
                            .0
                            .cache
                            .quarantine(&attempt, PERMANENT_ERROR.into())
                            .await
                            .is_err()
                    {
                        continue;
                    }
                    state.recovering = false;
                    state.initialized = true;
                    state.retry_at = None;
                    return;
                }
            }
        });
    }

    async fn reboot_and_control(&self, attempt: &Attempt, identity: &Identity) -> eyre::Result<()> {
        let id = self
            .0
            .cache
            .preview_started(
                attempt.request_id,
                self.name().into(),
                identity.clone(),
                "restart",
            )
            .await?;
        let result = timeout(self.0.timing.boot, async {
            let mut stream = TcpStream::connect(&self.0.config.control_address).await?;
            // Guide p.755: setvar, not do. Do not retry an uncertain write.
            stream
                .write_all(b"! U1 setvar \"device.reset\" \"\"\r\n")
                .await?;
            stream.shutdown().await?;
            tokio::time::sleep(self.0.timing.settle).await;
            loop {
                if let Ok(current) = self.identity().await {
                    eyre::ensure!(&current == identity, "Identity changed during recovery");
                    if self
                        .preview(attempt, identity, "recovery-control", CONTROL)
                        .await?
                        .is_ok()
                    {
                        return Ok::<_, eyre::Report>(());
                    }
                }
                tokio::time::sleep(self.0.timing.poll).await;
            }
        })
        .await
        .map_err(|_| eyre::eyre!("Printer recovery timed out"))
        .and_then(|r| r);
        self.0
            .cache
            .preview_finished(
                id,
                result
                    .as_ref()
                    .err()
                    .map(|_| "Printer recovery failed".into()),
                result.as_ref().err().map(|_| "recovery_failed"),
            )
            .await?;
        result
    }

    async fn identity(&self) -> eyre::Result<Identity> {
        timeout(self.0.timing.io, async {
            let mut stream = TcpStream::connect(&self.0.config.control_address).await?;
            Ok(Identity {
                serial: getvar(&mut stream, "device.unique_id").await?,
                firmware: getvar(&mut stream, "appl.name").await?,
            })
        })
        .await
        .map_err(eyre::Report::from)?
    }

    // Outer error means persistence failed: never interpret it as a printer
    // fault or reboot because of it. Inner error is a recorded device failure.
    async fn preview(
        &self,
        attempt: &Attempt,
        identity: &Identity,
        phase: &'static str,
        zpl: &str,
    ) -> eyre::Result<Result<Vec<u8>, PreviewFailure>> {
        let id = self
            .0
            .cache
            .preview_started(
                attempt.request_id,
                self.name().into(),
                identity.clone(),
                phase,
            )
            .await?;
        let label = reset_label(zpl, self.0.config.width, self.0.config.height);
        let result =
            zebra_http_api::zpl_to_png(self.0.client.clone(), self.0.url.clone(), &label).await;
        let control = zpl == CONTROL;
        let width = self.0.config.width;
        let height = self.0.config.height;
        let result = match result {
            Ok(png) => {
                telemetry::spawn_blocking("printer.decode", move || {
                    let raster = raster_diff::Raster::decode_png(&png)
                        .map_err(|_| eyre::eyre!("Invalid preview PNG"))?;
                    if control {
                        eyre::ensure!(
                            raster.width == u32::from(width) && raster.height == u32::from(height),
                            "Control canvas mismatch"
                        );
                        eyre::ensure!(
                            raster.pixels.iter().enumerate().all(|(i, pixel)| {
                                let x = i % width as usize;
                                let y = i / width as usize;
                                *pixel == if x < 8 && y < 8 { 0 } else { 255 }
                            }),
                            "Control pixels mismatch"
                        );
                    }
                    Ok(png)
                })
                .await?
            }
            Err(error) => Err(error),
        };
        let result = result.map_err(|error| {
            if error.downcast_ref::<zebra_http_api::NoPreview>().is_some() {
                PreviewFailure::Rejected
            } else if error
                .downcast_ref::<reqwest::Error>()
                .is_some_and(|e| e.is_timeout())
            {
                PreviewFailure::Timeout
            } else {
                PreviewFailure::Unavailable
            }
        });
        let failure_kind = result.as_ref().err().map(|failure| match failure {
            PreviewFailure::Timeout => "hang",
            PreviewFailure::Rejected => "rejected",
            PreviewFailure::Unavailable => "unavailable",
        });
        self.0
            .cache
            .preview_finished(
                id,
                failure_kind.map(|kind| format!("Printer preview failed: {kind}")),
                failure_kind,
            )
            .await?;
        Ok(result)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PreviewFailure {
    Rejected,
    Timeout,
    Unavailable,
}

const PERMANENT_ERROR: &str = "Label repeatedly failed on this printer firmware";

const CONTROL: &str = "^XA^FO0,0^GB8,8,8^FS^XZ";

fn reset_label(zpl: &str, width: u16, height: u16) -> String {
    // Insert after the validated XA, never rewrite user field contents. Admission
    // forbids syntax/font remapping and retained bitmap changes. Explicit values
    // prevent omitted operands from inheriting the preceding request's settings.
    // Do not inject FB0: it suppresses text, and FB is field-local (FS clears it),
    // per the guide pp.186-187.
    let body = zpl
        .trim_start()
        .strip_prefix("^XA")
        .expect("validated label");
    format!("^XA^MCY^PW{width}^LL{height}^LH0,0^LS0^LT0^PON^PMN^LRN^FWN,0^CFA,9,5^BY2,3,10^CI27^FPH,0^FO0,0{body}")
}
fn recovery_key(identity: &Identity) -> Vec<u8> {
    // Restart throttling is physical-printer scoped, independent of render cache
    // namespace, firmware, credentials, address aliases and public name.
    cache::renderer_key(&identity.serial, &[], "printer-recovery-v1")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

async fn getvar(stream: &mut TcpStream, variable: &str) -> eyre::Result<String> {
    stream
        .write_all(format!("! U1 getvar \"{variable}\"\r\n").as_bytes())
        .await?;
    let mut opened = false;
    let mut value = Vec::new();
    for _ in 0..4096 {
        let byte = stream.read_u8().await?;
        match byte {
            b'"' if !opened => opened = true,
            b'"' => {
                let value = String::from_utf8(value)?;
                eyre::ensure!(
                    !value.trim().is_empty() && value != "?",
                    "Unsupported identity variable"
                );
                return Ok(value);
            }
            b'\r' | b'\n' if !opened => {}
            byte if opened && !byte.is_ascii_control() => value.push(byte),
            _ => eyre::bail!("Malformed identity response"),
        }
    }
    eyre::bail!("Identity response exceeds 4096 bytes")
}

#[cfg(test)]
mod tests;
