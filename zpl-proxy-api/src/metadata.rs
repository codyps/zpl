//! Bounded read-only printer metadata snapshot.
use std::time::{Duration, Instant};

pub async fn identity(target: String) -> eyre::Result<zebra_sgd::PrinterIdentity> {
    let deadline = Instant::now() + Duration::from_secs(15);
    let addresses: Vec<_> =
        tokio::time::timeout(Duration::from_secs(5), tokio::net::lookup_host(target))
            .await??
            .collect();
    crate::telemetry::spawn_blocking("printer.sgd", move || {
        // Keep blocking socket reads off the executor. Connection setup and all
        // queries share one deadline. A disconnected client cannot cancel the
        // enclosing render/persistence task or release its printer lock early.
        let mut stream = None;
        for address in addresses {
            let remaining = deadline.saturating_duration_since(Instant::now());
            eyre::ensure!(!remaining.is_zero(), "SGD connection timed out");
            if let Ok(connection) = std::net::TcpStream::connect_timeout(
                &address,
                remaining.min(Duration::from_secs(5)),
            ) {
                stream = Some(connection);
                break;
            }
        }
        let mut stream =
            stream.ok_or_else(|| eyre::eyre!("unable to connect to printer SGD endpoint"))?;
        zebra_sgd::printer_identity(&mut stream, deadline)
    })
    .await?
}
