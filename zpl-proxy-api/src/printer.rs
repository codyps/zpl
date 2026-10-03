//! SGD metadata reads for the configured HTTP preview printer.
use std::{
    net::SocketAddr,
    time::{Duration, Instant},
};

pub fn target(url: &reqwest::Url, address: Option<SocketAddr>) -> eyre::Result<String> {
    if let Some(address) = address {
        eyre::ensure!(address.port() != 0, "SGD port must be nonzero");
        return Ok(address.to_string());
    }
    let host = url
        .host_str()
        .ok_or_else(|| eyre::eyre!("printer URL needs a host"))?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    Ok(if host.contains(':') {
        format!("[{host}]:9100")
    } else {
        format!("{host}:9100")
    })
}

pub async fn identity(target: String) -> eyre::Result<zebra_sgd::PrinterIdentity> {
    let deadline = Instant::now() + Duration::from_secs(15);
    let addresses: Vec<_> =
        tokio::time::timeout(Duration::from_secs(5), tokio::net::lookup_host(target))
            .await??
            .collect();
    zpl_proxy_api::telemetry::spawn_blocking("printer.sgd", move || {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_uses_printer_host_and_raw_port_not_http_port_or_credentials() {
        assert_eq!(
            target(
                &"http://user:password@printer.local:8080/path/"
                    .parse()
                    .unwrap(),
                None
            )
            .unwrap(),
            "printer.local:9100"
        );
        assert_eq!(
            target(&"http://[::1]:8080/".parse().unwrap(), None).unwrap(),
            "[::1]:9100"
        );
        assert_eq!(
            target(
                &"http://printer/".parse().unwrap(),
                Some("127.0.0.1:9201".parse().unwrap())
            )
            .unwrap(),
            "127.0.0.1:9201"
        );
        assert!(target(
            &"http://printer/".parse().unwrap(),
            Some("127.0.0.1:0".parse().unwrap())
        )
        .is_err());
    }
}
