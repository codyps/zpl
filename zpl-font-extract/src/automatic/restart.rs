//! Bounded JSON settings-channel recovery for stalled HTTP previews.
use crate::collection::identity;
use eyre::{ensure, Result};
use std::{
    io::Write,
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

/// Zebra Programming Guide pp.609–610 (JSON port), p.755 (device.reset).
/// https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
pub fn reset(host: &str, port: u16) -> Result<()> {
    let address = (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| eyre::eyre!("no reset address"))?;
    let mut socket = TcpStream::connect_timeout(&address, Duration::from_secs(5))?;
    socket.set_write_timeout(Some(Duration::from_secs(5)))?;
    // Never retry this write: a failed write can still have reset the printer.
    socket.write_all(b"{}{\"device.reset\":\"\"}\r\n")?;
    Ok(())
}

pub async fn recover(
    client: &reqwest::Client,
    host: &reqwest::Url,
    port: u16,
    expected: &serde_json::Value,
    wait: Duration,
) -> Result<()> {
    let hostname = host.host_str().unwrap().trim_matches(['[', ']']);
    ensure!(
        identity::identity(hostname, port)? == *expected,
        "printer identity changed before reset"
    );
    reset(hostname, port)?;
    // Allow the reset to start; a response from the old HTTP instance is not readiness.
    tokio::time::sleep(Duration::from_secs(10).min(wait)).await;
    wait_ready(
        client,
        host,
        port,
        expected,
        wait.saturating_sub(Duration::from_secs(10)),
        Duration::from_secs(5),
    )
    .await
}

async fn wait_ready(
    client: &reqwest::Client,
    host: &reqwest::Url,
    port: u16,
    expected: &serde_json::Value,
    wait: Duration,
    interval: Duration,
) -> Result<()> {
    let hostname = host.host_str().unwrap().trim_matches(['[', ']']);
    let deadline = tokio::time::Instant::now() + wait;
    loop {
        ensure!(
            tokio::time::Instant::now() < deadline,
            "printer did not recover before deadline"
        );
        if let Ok(response) = client
            .get(host.clone())
            .timeout(Duration::from_secs(3))
            .send()
            .await
        {
            if response.status().is_success() {
                if let Ok(actual) = identity::identity(hostname, port) {
                    ensure!(actual == *expected, "printer identity changed after reset");
                    return Ok(());
                }
            }
        }
        eprintln!("Waiting for printer HTTP after reset...");
        tokio::time::sleep(
            interval.min(deadline.saturating_duration_since(tokio::time::Instant::now())),
        )
        .await;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn reset_sends_exact_json_once() {
        use std::{io::Read, net::TcpListener};
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut bytes = vec![];
            stream.read_to_end(&mut bytes).unwrap();
            assert_eq!(bytes, b"{}{\"device.reset\":\"\"}\r\n");
        });
        super::reset("127.0.0.1", port).unwrap();
        task.join().unwrap();
    }
}

#[cfg(test)]
mod readiness_tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    #[tokio::test]
    async fn readiness_retries_and_checks_identity() {
        let http = TcpListener::bind("127.0.0.1:0").unwrap();
        let host = reqwest::Url::parse(&format!("http://{}/", http.local_addr().unwrap())).unwrap();
        let server = std::thread::spawn(move || {
            for status in [503, 503, 200] {
                let (mut s, _) = http.accept().unwrap();
                let mut buf = [0; 4096];
                assert!(s.read(&mut buf).unwrap() > 0);
                write!(
                    s,
                    "HTTP/1.1 {status} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .unwrap();
            }
        });
        let json = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = json.local_addr().unwrap().port();
        let identity = serde_json::json!({"device.unique_id":"test","appl.name":"test"});
        let response = identity.to_string();
        let settings = std::thread::spawn(move || {
            let (mut s, _) = json.accept().unwrap();
            let mut bytes = [0; 4096];
            let n = s.read(&mut bytes).unwrap();
            assert!(!bytes[..n].windows(12).any(|w| w == b"device.reset"));
            s.write_all(response.as_bytes()).unwrap();
        });
        wait_ready(
            &reqwest::Client::new(),
            &host,
            port,
            &identity,
            Duration::from_secs(5),
            Duration::from_millis(1),
        )
        .await
        .unwrap();
        server.join().unwrap();
        settings.join().unwrap();
        assert!(wait_ready(
            &reqwest::Client::new(),
            &host,
            port,
            &identity,
            Duration::ZERO,
            Duration::ZERO
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("deadline"));
    }
}
