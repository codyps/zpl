//! Read-only SGD queries over the printer's bidirectional TCP connection.
//! Zebra Programming Guide: getvar, device.unique_id, appl.name and each
//! setting below. Queries are separate CRLF-terminated SGD commands, not ZPL.
//! https://docs.zebra.com/us/en/printers/software/zpl-pg/c-sgd-printer-commands.html
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf

use eyre::{bail, ensure, Result};
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::TcpStream,
    time::Instant,
};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PrinterIdentity {
    pub model: String,
    pub serial: String,
    pub firmware: String,
    /// Available SGD rendering settings, captured before submitting the preview.
    /// Keys are exact SGD names; unsupported settings are omitted.
    pub configuration: BTreeMap<String, String>,
}

// Deliberately query known rendering settings rather than allcv, which can
// disclose network settings and credentials. Do not query device.zuid: its
// first use can generate a new identifier. A serial number is sufficient here.
pub const RENDER_SETTINGS: &[&str] = &[
    "appl.link_os_version",
    "appl.bootblock",
    "head.resolution.in_dpi",
    "ezpl.print_width",
    "zpl.label_length",
    "ezpl.label_length_max",
    "media.speed",
    "media.darkness_mode",
    "print.tone",
    "zpl.relative_darkness",
    "head.darkness_switch",
    "media.printmode",
    "ezpl.print_method",
    "ezpl.media_type",
    "device.sensor_select",
    "zpl.label_orientation",
    "zpl.left_position",
    "zpl.format_prefix",
    "zpl.control_character",
    "zpl.delimiter",
    "zpl.zpl_mode",
    "device.command_override.active",
    "ezpl.tear_off",
    "ezpl.head_close_action",
    "ezpl.power_up_action",
    "print.legacy_compatibility",
];

/// Read a bounded quoted response. `None` means the printer explicitly returned
/// an unsupported/empty value; I/O, deadline and framing failures remain errors.
/// Replies need not end with a newline. Each read updates the timeout so a
/// trickling response cannot extend the caller's overall deadline indefinitely.
pub fn getvar(stream: &mut TcpStream, name: &str, deadline: Instant) -> Result<Option<String>> {
    ensure!(
        !name.is_empty()
            && name.len() <= 128
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._[]".contains(&b)),
        "invalid SGD setting name"
    );
    let remaining = || -> Result<_> {
        let remaining = deadline.saturating_duration_since(Instant::now());
        ensure!(!remaining.is_zero(), "SGD operation timed out");
        Ok(remaining)
    };
    stream.set_write_timeout(Some(remaining()?))?;
    stream.write_all(format!("! U1 getvar \"{name}\"\r\n").as_bytes())?;
    let mut value = Vec::new();
    let mut opened = false;
    for _ in 0..4096 {
        stream.set_read_timeout(Some(remaining()?))?;
        let mut byte = [0];
        stream.read_exact(&mut byte)?;
        match byte[0] {
            b'"' if !opened => opened = true,
            b'"' => {
                let value = String::from_utf8(value)?;
                return Ok((!value.trim().is_empty() && value != "?").then_some(value));
            }
            b'\r' | b'\n' if !opened => {}
            byte if opened && !byte.is_ascii_control() => value.push(byte),
            _ => bail!("malformed SGD response for {name}"),
        }
    }
    bail!("SGD response for {name} exceeds 4096 bytes")
}

pub fn get_required(stream: &mut TcpStream, name: &str, deadline: Instant) -> Result<String> {
    getvar(stream, name, deadline)?.ok_or_else(|| eyre::eyre!("unsupported SGD variable {name}"))
}

/// Caller owns connection setup and a single deadline for the whole snapshot.
/// Identity fields are required. Optional settings tolerate only explicit
/// unsupported replies; connection errors abort rather than misattribute values.
pub fn printer_identity(stream: &mut TcpStream, deadline: Instant) -> Result<PrinterIdentity> {
    let mut identity = PrinterIdentity {
        model: get_required(stream, "device.product_name", deadline)?,
        serial: get_required(stream, "device.unique_id", deadline)?,
        firmware: get_required(stream, "appl.name", deadline)?,
        configuration: BTreeMap::new(),
    };
    for value in [&identity.model, &identity.serial, &identity.firmware] {
        ensure!(
            value.len() <= 256 && value.bytes().all(|b| (32..=126).contains(&b)),
            "invalid printer identity value"
        );
    }
    for &name in RENDER_SETTINGS {
        if let Some(value) = getvar(stream, name, deadline)? {
            identity.configuration.insert(name.to_owned(), value);
        }
    }
    Ok(identity)
}

#[cfg(test)]
mod tests;
