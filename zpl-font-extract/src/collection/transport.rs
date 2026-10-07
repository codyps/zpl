//! Bounded read-only SGD font inventory and object download.
//! file.dir / file.type: Zebra ZPL II/ZBI2/SGD Programming Guide.
//! Protected resident FNT reads use the observed trailing-dot file selector.
use super::*;
use eyre::{ensure, eyre};
use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};
pub fn exchange(address: &str, command: &str, expected: Option<usize>) -> Result<Vec<u8>> {
    let max = expected.unwrap_or(1024 * 1024);
    ensure!(max <= 32 * 1024 * 1024, "object exceeds limit");
    let addr = address
        .to_socket_addrs()?
        .next()
        .ok_or_else(|| eyre!("printer address has no IP"))?;
    let mut conn = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
    conn.set_write_timeout(Some(Duration::from_secs(5)))?;
    conn.write_all(command.as_bytes())?;
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut data = vec![];
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        ensure!(!remaining.is_zero(), "SGD response deadline exceeded");
        conn.set_read_timeout(Some(remaining.min(Duration::from_secs(3))))?;
        let mut buf = [0u8; 16384];
        match conn.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                data.extend_from_slice(&buf[..n]);
                ensure!(
                    data.len() <= max,
                    "SGD response exceeds declared size/limit"
                );
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
    if let Some(size) = expected {
        ensure!(
            data.len() == size,
            "font length differs: expected {size}, received {}",
            data.len()
        );
    }
    ensure!(!data.is_empty(), "empty SGD response");
    Ok(data)
}
pub fn directory(bytes: &[u8], drive: char) -> Result<Vec<(String, usize)>> {
    let text = std::str::from_utf8(bytes)?;
    ensure!(
        text.trim().starts_with('"')
            && text.trim().ends_with('"')
            && text.contains(&format!("- DIR {drive}:")),
        "incomplete or unexpected file.dir response"
    );
    let mut entries = vec![];
    let mut names = BTreeSet::new();
    for line in text.lines() {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        if fields.first() != Some(&"*") {
            continue;
        }
        ensure!(fields.len() >= 3, "malformed file.dir row");
        let name = fields[1];
        if !name.ends_with(".FNT") {
            continue;
        }
        ensure!(
            selector(name) && name.starts_with(&format!("{drive}:")) && names.insert(name),
            "invalid/duplicate directory font"
        );
        let size: usize = fields[2].parse()?;
        ensure!(
            (116..=32 * 1024 * 1024).contains(&size),
            "invalid directory font size"
        );
        entries.push((name.to_string(), size));
    }
    Ok(entries)
}
pub fn collect(address: &str, drives: &str, out: &Path) -> Result<Collection> {
    ensure!(!out.exists(), "output exists");
    ensure!(
        !drives.is_empty()
            && drives.chars().all(|c| "ERZ".contains(c))
            && drives.chars().collect::<BTreeSet<_>>().len() == drives.len(),
        "drives must be distinct E/R/Z letters"
    );
    fs::create_dir_all(out)?;
    let mut fonts = vec![];
    let mut skipped = vec![];
    let mut manifests = vec![];
    for drive in drives.chars() {
        let bytes = exchange(
            address,
            &format!("! U1 do \"file.dir\" \"{drive}:\"\r\n"),
            None,
        )?;
        fs::write(out.join(format!("directory-{drive}.txt")), &bytes)?;
        manifests.push(json!({"drive":drive,"sha256":hash(&bytes)}));
        for (name, size) in directory(&bytes, drive)? {
            eprintln!("Read {name} ({size} bytes)");
            let data = exchange(
                address,
                &format!("! U1 do \"file.type\" \"{name}.\"\r\n"),
                Some(size),
            )?;
            fs::write(out.join(name.replace(':', "-")), &data)?;
            if data[4] != 1 {
                skipped.push(
                    json!({"name":name,"sha256":hash(&data),"reason":"not a type-1 bitmap font"}),
                );
                continue;
            }
            fonts.push(super::fnt::decode(&name, &data)?);
        }
    }
    fonts.sort_by(|a, b| a.name.cmp(&b.name));
    Collection::new(
        fonts,
        json!({"printer":address,"directories":manifests,"skipped":skipped,"mapping_completeness":"unprobed"}),
    )
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
        "{}{\"device.unique_id\":null,\"appl.name\":null}\r\n",
        None,
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
