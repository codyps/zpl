//! Network Zebra printer simulator. See README.md for the emulated subset.
mod framing;
mod http;
mod printer;
mod schema;
mod store;

use eyre::{ensure, eyre, Result};
pub use printer::Mode;
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
pub use store::{Detail, Entry};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{watch, Semaphore},
    task::JoinSet,
};

pub const MAX_REQUEST_BYTES: usize = 1_048_576;

#[derive(Clone, Debug)]
pub struct Config {
    pub model: String,
    pub serial: String,
    pub firmware: String,
    pub width: u32,
    pub height: u32,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            model: "ZD621 simulator".into(),
            serial: "SIM000000001".into(),
            firmware: "SIM.0.1.0".into(),
            width: 812,
            height: 1218,
        }
    }
}
impl Config {
    fn validate(&self) -> Result<()> {
        ensure!(
            (1..=4096).contains(&self.width)
                && (1..=4096).contains(&self.height)
                && u64::from(self.width) * u64::from(self.height) <= 10_000_000,
            "invalid simulator dimensions"
        );
        for value in [&self.model, &self.serial, &self.firmware] {
            ensure!(
                !value.is_empty()
                    && value.len() <= 256
                    && value.bytes().all(|b| (32..=126).contains(&b) && b != b'"'),
                "invalid simulator identity"
            );
        }
        Ok(())
    }
    fn options(&self) -> zpl::Options {
        zpl::Options {
            width: self.width,
            height: self.height,
            ..zpl::render::profiles::ZD621_203_DPI
        }
    }
    fn defaults(&self) -> BTreeMap<String, String> {
        [
            ("device.product_name", self.model.clone()),
            ("device.unique_id", self.serial.clone()),
            ("appl.name", self.firmware.clone()),
            ("head.resolution.in_dpi", "203".into()),
            ("ezpl.print_width", self.width.to_string()),
            ("zpl.label_length", self.height.to_string()),
            ("device.friendly_name", "Zebra simulator".into()),
            ("device.location", "".into()),
            ("device.company_name", "".into()),
            ("device.company_contact", "".into()),
            ("device.languages", "zpl".into()),
            ("device.pause", "off".into()),
            ("odometer.total_label_count", "0".into()),
            ("appl.link_os_version", "simulator".into()),
            ("media.speed", "6".into()),
            ("print.tone", "10".into()),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v))
        .collect()
    }
}

#[derive(Clone)]
pub struct Simulator {
    store: Arc<Mutex<store::Store>>,
    updates: watch::Sender<i64>,
    sequence: Arc<AtomicU64>,
    boot_id: Arc<str>,
    workers: Arc<Semaphore>,
}
impl Simulator {
    pub fn open(path: &str, config: Config) -> Result<Self> {
        let mut store = store::Store::open(path, config)?;
        let latest = store.latest_id()?;
        let boot_id = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        );
        Ok(Self {
            store: Arc::new(Mutex::new(store)),
            updates: watch::channel(latest).0,
            boot_id: boot_id.into(),
            sequence: Arc::new(AtomicU64::new(1)),
            workers: Arc::new(Semaphore::new(16)),
        })
    }
    pub fn router(&self) -> axum::Router {
        http::router(self.clone())
    }
    pub fn connection_id(&self) -> String {
        format!(
            "{}-{}",
            self.boot_id,
            self.sequence.fetch_add(1, Ordering::Relaxed)
        )
    }
    async fn blocking<T: Send + 'static>(
        &self,
        f: impl FnOnce(&mut store::Store) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let permit = self.workers.clone().acquire_owned().await?;
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let mut store = store
                .lock()
                .map_err(|_| eyre!("simulator state lock poisoned"))?;
            f(&mut store)
        })
        .await?
    }
    pub async fn submit(
        &self,
        raw: Vec<u8>,
        transport: String,
        connection: String,
        mode: Mode,
    ) -> Result<printer::Outcome> {
        let updates = self.updates.clone();
        self.blocking(move |store| {
            let outcome = store.submit(raw, &transport, &connection, mode)?;
            // Publish while serialized with commit so event IDs cannot regress
            // when submit futures are polled in a different order.
            updates.send_replace(outcome.id);
            Ok(outcome)
        })
        .await
    }
    pub async fn history(&self, after: Option<i64>, before: Option<i64>) -> Result<Vec<Entry>> {
        self.blocking(move |store| store.list(after, before)).await
    }
    pub async fn detail(&self, id: i64) -> Result<Option<Detail>> {
        self.blocking(move |store| store.detail(id)).await
    }
    pub async fn image(&self, id: i64, page: i32) -> Result<Option<Vec<u8>>> {
        self.blocking(move |store| store.image(id, page)).await
    }
}

/// Serve a raw printer port; `json_only` implements the separate JSON port.
/// A maximum of 64 sockets is serviced per listener. Partial requests are logged
/// on disconnect, read failure, 30-second idle timeout, limit, and shutdown.
pub async fn serve_tcp(
    listener: TcpListener,
    simulator: Simulator,
    json_only: bool,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let slots = Arc::new(Semaphore::new(64));
    let mut tasks = JoinSet::new();
    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            result = tasks.join_next(), if !tasks.is_empty() => {
                if let Some(result) = result { result??; }
            }
            permit = slots.clone().acquire_owned() => {
                let permit = permit?;
                let (stream, _) = tokio::select! { _ = shutdown.changed() => break, accepted = listener.accept() => accepted? };
                let app = simulator.clone(); let stop = shutdown.clone();
                tasks.spawn(async move { let _permit = permit; connection(stream, app, json_only, stop).await });
            }
        }
    }
    while let Some(result) = tasks.join_next().await {
        result??;
    }
    Ok(())
}
async fn connection(
    mut socket: TcpStream,
    app: Simulator,
    json_only: bool,
    mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
    let connection = app.connection_id();
    let transport = if json_only { "tcp-json" } else { "tcp" };
    let mut pending = Vec::new();
    let mut chunk = [0; 8192];
    loop {
        let read = tokio::select! {
            _ = shutdown.changed() => Err("server shutdown".to_owned()),
            result = tokio::time::timeout(Duration::from_secs(30), socket.read(&mut chunk)) => match result {
                Ok(Ok(n)) => Ok(n), Ok(Err(_)) => Err("connection read failed".into()), Err(_) => Err("connection idle timeout".into())
            }
        };
        let n = match read {
            Ok(n) if n > 0 => n,
            result => {
                if !pending.is_empty() {
                    let reason = match result {
                        Err(e) => e,
                        _ => "incomplete request at disconnect".into(),
                    };
                    app.submit(
                        pending,
                        transport.into(),
                        connection.clone(),
                        Mode::Invalid(reason),
                    )
                    .await?;
                }
                break;
            }
        };
        pending.extend_from_slice(&chunk[..n]);
        loop {
            let start = pending
                .iter()
                .position(|b| !b.is_ascii_whitespace())
                .unwrap_or(pending.len());
            pending.drain(..start);
            if pending.is_empty() {
                break;
            }
            let frame = if json_only && pending[0] != b'{' {
                Err("JSON port requires {}-introduced SGD JSON".into())
            } else {
                framing::frame_end(&pending)
            };
            match frame {
                Ok(Some(end)) if end <= MAX_REQUEST_BYTES => {
                    let raw = pending.drain(..end).collect();
                    let outcome = app
                        .submit(raw, transport.into(), connection.clone(), Mode::Auto)
                        .await?;
                    if !outcome.response.is_empty() {
                        match tokio::time::timeout(
                            Duration::from_secs(5),
                            socket.write_all(&outcome.response),
                        )
                        .await
                        {
                            Ok(Ok(())) => {}
                            _ => return Ok(()),
                        }
                    }
                }
                Ok(None) if pending.len() < MAX_REQUEST_BYTES => break,
                result => {
                    let reason = match result {
                        Err(e) => e,
                        _ => "request exceeds 1 MiB".into(),
                    };
                    // Store the bounded accepted prefix, close the connection;
                    // oversized traffic is never rendered or executed.
                    pending.truncate(MAX_REQUEST_BYTES);
                    app.submit(
                        pending,
                        transport.into(),
                        connection.clone(),
                        Mode::Invalid(reason),
                    )
                    .await?;
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}
