//! Bounded read-only printer identity queries for preview cache scoping.
use super::*;
use eyre::{ensure, eyre};
use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};
fn exchange(address: &str) -> Result<Vec<u8>> {
    let addr = address
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| eyre!("printer address has no IP"))?;
    let mut conn = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
    conn.set_write_timeout(Some(Duration::from_secs(5)))?;
    conn.write_all(b"{}{\"device.unique_id\":null,\"appl.name\":null}\r\n")?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut data = vec![];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        ensure!(!remaining.is_zero(), "identity response deadline exceeded");
        conn.set_read_timeout(Some(remaining.min(Duration::from_secs(3))))?;
        let mut buf = [0u8; 16384];
        match conn.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                data.extend_from_slice(&buf[..n]);
                ensure!(data.len() <= 64 * 1024, "identity response exceeds 64 KiB");
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ) =>
            {
                break
            }
            Err(e) => return Err(e.into()),
        }
    }
    ensure!(!data.is_empty(), "empty identity response");
    Ok(data)
}
/// JSON settings channel: Zebra Programming Guide pp.609–610. Query only
/// identity fields, never a complete configuration that may contain secrets.
pub fn identity(host: &str, port: u16) -> Result<Value> {
    ensure!(port != 0, "invalid identity port");
    let address = (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| eyre!("no identity address"))?;
    let bytes = exchange(&address.to_string())?;
    let value: Value = serde_json::from_slice(&bytes)?;
    validate_identity(&value)?;
    Ok(value)
}
pub fn validate_identity(value: &Value) -> Result<()> {
    ensure!(
        ["device.unique_id", "appl.name"]
            .iter()
            .all(|k| value[k].as_str().is_some_and(|s| !s.is_empty())),
        "missing printer serial or firmware"
    );
    Ok(())
}
