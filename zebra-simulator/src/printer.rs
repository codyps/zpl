//! Emulation contract, not captured firmware behavior. Protocol references:
//! Link-OS Addendum pp. 25–26; ZPL Programming Guide ~HS, ~HI, ^HH, ~PP, ~PS.
//! https://techdocs.zebra.com/link-os/2-13/webservices/content/link-os-pg-addendum-en.pdf
//! https://docs.zebra.com/us/en/printers/software/zpl-pg/c-zpl-zpl-commands.html
use crate::{
    schema::{labels, requests, settings},
    store::Store,
    MAX_REQUEST_BYTES,
};
use diesel::prelude::*;
use eyre::Result;
use std::collections::BTreeMap;
use zebra_sgd_parser::{parse_json_prefix, parse_sgd, Command};
use zpl::{
    output::{Limits as OutputLimits, Png},
    render::{render_with_limits, Limits},
};

pub enum Mode {
    Auto,
    Preview,
    Invalid(String),
}
pub struct Outcome {
    pub id: i64,
    pub response: Vec<u8>,
    pub error: Option<String>,
    pub pages: usize,
}
#[derive(Default)]
struct Reply {
    kind: &'static str,
    response: Vec<u8>,
    error: Option<String>,
    warnings: Vec<String>,
    images: Vec<Vec<u8>>,
}

const LIMITS: Limits = Limits {
    input_bytes: MAX_REQUEST_BYTES,
    labels: 16,
    pixels: 10_000_000,
    dimension: 4096,
    segments: 100_000,
    ..Limits::DEFAULT
};

impl Store {
    pub fn submit(
        &mut self,
        mut raw: Vec<u8>,
        transport: &str,
        connection: &str,
        mode: Mode,
    ) -> Result<Outcome> {
        let mut values = self.values()?;
        let old = values.clone();
        let mode = if raw.len() > MAX_REQUEST_BYTES {
            raw.truncate(MAX_REQUEST_BYTES);
            Mode::Invalid("request exceeds 1 MiB; retained bounded prefix".into())
        } else {
            mode
        };
        let input = raw.trim_ascii();
        let reply = match mode {
            Mode::Invalid(error) => Reply {
                kind: "invalid",
                error: Some(error),
                ..Reply::default()
            },
            Mode::Auto if input.starts_with(b"!") => match parse_sgd(input) {
                Ok(command) => classic(command, &mut values),
                Err(e) => Reply {
                    kind: "sgd",
                    error: Some(e.to_string()),
                    ..Reply::default()
                },
            },
            Mode::Auto if input.starts_with(b"{") => json(input, &mut values),
            Mode::Auto if is_control(input) => control(input, &mut values),
            Mode::Auto => print(&raw, &mut values, self.config.options(), false),
            Mode::Preview => print(&raw, &mut values, self.config.options(), true),
        };
        let pages = reply.images.len();
        let warnings = serde_json::to_string(&reply.warnings)?;
        let id = self.db.transaction::<_, diesel::result::Error, _>(|db| {
            diesel::insert_into(requests::table)
                .values((
                    requests::connection_id.eq(connection),
                    requests::transport.eq(transport),
                    requests::kind.eq(reply.kind),
                    requests::raw.eq(&raw),
                    requests::response.eq(&reply.response),
                    requests::error.eq(&reply.error),
                    requests::warnings.eq(&warnings),
                    requests::label_count.eq(pages as i32),
                ))
                .execute(db)?;
            let id: i64 = requests::table
                .select(requests::id)
                .order(requests::id.desc())
                .first(db)?;
            for (page, png) in reply.images.iter().enumerate() {
                diesel::insert_into(labels::table)
                    .values((
                        labels::request_id.eq(id),
                        labels::page.eq(page as i32),
                        labels::png.eq(png),
                    ))
                    .execute(db)?;
            }
            for (name, value) in &values {
                if old.get(name) != Some(value) {
                    diesel::update(settings::table.find(name))
                        .set(settings::value.eq(value))
                        .execute(db)?;
                }
            }
            Ok(id)
        })?;
        Ok(Outcome {
            id,
            response: reply.response,
            error: reply.error,
            pages,
        })
    }
}
fn get(name: &str, values: &BTreeMap<String, String>) -> String {
    if name == "allcv" {
        return values
            .iter()
            .map(|(k, v)| format!("{k} : {v}\r\n"))
            .collect();
    }
    values.get(name).cloned().unwrap_or_else(|| "?".into())
}
fn writable(name: &str) -> bool {
    matches!(
        name,
        "device.location"
            | "device.friendly_name"
            | "device.company_name"
            | "device.company_contact"
            | "ezpl.print_width"
            | "zpl.label_length"
            | "media.speed"
            | "print.tone"
            | "device.languages"
            | "device.pause"
    )
}
fn set(name: &str, value: &str, values: &mut BTreeMap<String, String>) -> Result<(), String> {
    if !writable(name) {
        return Err(format!("unsupported or read-only SGD setting: {name}"));
    }
    if value.len() > 256 || value.chars().any(|c| c.is_control() || c == '"') {
        return Err(format!("invalid value for {name}"));
    }
    let valid = match name {
        "ezpl.print_width" | "zpl.label_length" => {
            value.parse::<u32>().is_ok_and(|n| (1..=4096).contains(&n))
        }
        "media.speed" => value.parse::<u32>().is_ok_and(|n| (1..=14).contains(&n)),
        "print.tone" => value
            .parse::<f64>()
            .is_ok_and(|n| (0.0..=30.0).contains(&n)),
        "device.languages" => value == "zpl",
        "device.pause" => matches!(value, "on" | "off"),
        _ => true,
    };
    if !valid {
        return Err(format!("invalid value for {name}"));
    }
    values.insert(name.into(), value.into());
    Ok(())
}
fn action(name: &str, values: &mut BTreeMap<String, String>) -> Result<(), String> {
    match name {
        "device.pause" => {
            values.insert("device.pause".into(), "on".into());
        }
        "device.unpause" | "device.reset" => {
            values.insert("device.pause".into(), "off".into());
        }
        "formats.cancel_all" => {}
        _ => return Err(format!("unsupported SGD action: {name}")),
    }
    Ok(())
}
fn classic(command: Command, values: &mut BTreeMap<String, String>) -> Reply {
    let mut reply = Reply {
        kind: "sgd",
        ..Reply::default()
    };
    match command {
        Command::Get { name } => {
            reply.response = format!("\"{}\"\r\n", get(&name, values)).into_bytes();
        }
        Command::Set { name, value } => {
            reply.error = set(&name, &value, values).err();
        }
        Command::Do { name, .. } => {
            reply.error = action(&name, values).err();
            if reply.error.is_some() {
                reply.response = b"\"?\"\r\n".to_vec();
            }
        }
    }
    reply
}
fn json(raw: &[u8], values: &mut BTreeMap<String, String>) -> Reply {
    let mut reply = Reply {
        kind: "sgd-json",
        ..Reply::default()
    };
    let commands = match parse_json_prefix(raw) {
        Ok(Some((commands, consumed))) if raw[consumed..].iter().all(u8::is_ascii_whitespace) => {
            commands
        }
        result => {
            reply.error = Some(match result {
                Err(e) => e.to_string(),
                _ => "incomplete or trailing SGD JSON input".into(),
            });
            reply.response = b"{}\r\n".to_vec();
            return reply;
        }
    };
    let mut response = serde_json::Map::new();
    let mut errors = Vec::new();
    for command in commands {
        let (Command::Get { name } | Command::Set { name, .. }) = &command else {
            unreachable!()
        };
        if let Command::Set { value, .. } = &command {
            if let Err(e) = set(name, value, values) {
                errors.push(e);
            }
        }
        let value = if name == "allvalues" || name == "allconfig" {
            let map = values.iter().map(|(k,v)| (k.clone(), if name == "allconfig" { serde_json::json!({"value": v, "type": "string", "access": if writable(k) { "RW" } else { "R" }}) } else { serde_json::json!(v) })).collect();
            serde_json::Value::Object(map)
        } else if let Some(value) = values.get(name) {
            serde_json::json!(value)
        } else {
            let prefix = format!("{name}.");
            let branch: serde_json::Map<_, _> = values
                .iter()
                .filter(|(k, _)| k.starts_with(&prefix))
                .map(|(k, v)| (k.clone(), serde_json::json!(v)))
                .collect();
            if branch.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::Value::Object(branch)
            }
        };
        response.insert(name.clone(), value);
    }
    reply.response = serde_json::to_vec(&response).expect("JSON string serialization");
    reply.response.extend_from_slice(b"\r\n");
    if !errors.is_empty() {
        reply.error = Some(errors.join("; "));
    }
    reply
}
pub fn is_control(raw: &[u8]) -> bool {
    matches!(
        raw,
        b"~HS" | b"~HI" | b"^HH" | b"~PP" | b"~PS" | b"~JA" | b"~JR"
    )
}
fn control(raw: &[u8], values: &mut BTreeMap<String, String>) -> Reply {
    let mut reply = Reply {
        kind: "zpl-control",
        ..Reply::default()
    };
    match raw {
        // ~HS: three STX/ETX-delimited status records. No physical sensor errors
        // or print queue in this simulator; only the pause flag varies.
        b"~HS" => {
            let pause = if values["device.pause"] == "on" { 1 } else { 0 };
            reply.response = format!("\x02030,0,{pause},{:04},000,0,0,0,000,0,0,0\x03\r\n\x02000,0,0,0,1,2,6,0,00000000,1,000\x03\r\n\x021234,0\x03\r\n", values["zpl.label_length"].parse::<u32>().unwrap_or(1218)).into_bytes();
        }
        b"~HI" => {
            reply.response = format!(
                "\x02{},{},8,8192\x03\r\n",
                values["device.product_name"], values["appl.name"]
            )
            .into_bytes()
        }
        b"^HH" => reply.response = format!("\x02{}\x03\r\n", get("allcv", values)).into_bytes(),
        b"~PP" => {
            let _ = action("device.pause", values);
        }
        b"~PS" | b"~JR" => {
            let _ = action("device.unpause", values);
        }
        b"~JA" => {}
        _ => unreachable!(),
    }
    reply
}
fn print(
    raw: &[u8],
    values: &mut BTreeMap<String, String>,
    mut options: zpl::Options,
    preview: bool,
) -> Reply {
    let mut reply = Reply {
        kind: if preview { "preview" } else { "label" },
        ..Reply::default()
    };
    options.width = values["ezpl.print_width"].parse().unwrap_or(options.width);
    options.height = values["zpl.label_length"].parse().unwrap_or(options.height);
    let result = (|| -> Result<_, String> {
        if !preview && values["device.pause"] == "on" {
            return Err(
                "printer paused; no label printed (simulator has no deferred queue)".into(),
            );
        }
        let document = render_with_limits(raw, options, LIMITS).map_err(|e| e.to_string())?;
        if document.labels.is_empty() {
            return Err("request contains no printable labels; stored objects across requests are unsupported".into());
        }
        let images = document
            .labels
            .iter()
            .map(|scene| {
                Png.encode_with_limits(
                    scene,
                    OutputLimits {
                        pixels: LIMITS.pixels,
                        segments: LIMITS.segments,
                        ..OutputLimits::DEFAULT
                    },
                )
                .map_err(|e| e.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok((images, document.warnings))
    })();
    match result {
        Ok((images, warnings)) => {
            if !preview {
                let count = values["odometer.total_label_count"]
                    .parse::<u64>()
                    .unwrap_or(0)
                    .saturating_add(images.len() as u64);
                values.insert("odometer.total_label_count".into(), count.to_string());
            }
            reply.images = images;
            reply.warnings = warnings;
        }
        Err(e) => reply.error = Some(e),
    }
    reply
}
