//! Bounded read-only printer identity queries for preview cache scoping.
use super::*;
use eyre::{ensure, eyre};
use std::{
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};
fn exchange(address: &str, request: &[u8]) -> Result<Vec<u8>> {
    let addr = address
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| eyre!("printer address has no IP"))?;
    let mut conn = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
    conn.set_write_timeout(Some(Duration::from_secs(5)))?;
    conn.write_all(request)?;
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
    let bytes = exchange(
        &address.to_string(),
        b"{}{\"device.unique_id\":null,\"appl.name\":null}\r\n",
    )?;
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

/// List font selectors only; never retrieve stored object contents.
pub fn fonts(host: &str, port: u16) -> Result<Vec<String>> {
    ensure!(port != 0, "invalid directory port");
    let address = (host, port)
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| eyre!("no directory address"))?;
    let mut names: BTreeSet<String> = "ABCDEFGH@".chars().map(|c| c.to_string()).collect();
    for drive in ['Z', 'E', 'R'] {
        let bytes = exchange(
            &address.to_string(),
            format!("! U1 do \"file.dir\" \"{drive}:\"\r\n").as_bytes(),
        )?;
        names.extend(directory_names(&bytes, drive)?);
    }
    ensure!(names.len() <= 256, "too many font selectors");
    Ok(names.into_iter().collect())
}
pub fn directory_names(bytes: &[u8], drive: char) -> Result<Vec<String>> {
    let text = std::str::from_utf8(bytes)?;
    ensure!(
        text.trim().starts_with('"')
            && text.trim().ends_with('"')
            && text.contains(&format!("- DIR {drive}:")),
        "incomplete font directory response"
    );
    let mut names = BTreeSet::new();
    for line in text.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.first() != Some(&"*") {
            continue;
        }
        ensure!(fields.len() >= 3, "malformed directory entry");
        if fields[1].ends_with(".FNT") {
            ensure!(
                selector(fields[1]) && fields[1].starts_with(&format!("{drive}:")),
                "invalid font selector"
            );
            ensure!(
                names.insert(fields[1].to_string()),
                "duplicate directory font"
            );
        }
    }
    Ok(names.into_iter().collect())
}
