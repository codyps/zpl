//! Preview-only capture of a fixed corpus; never sends a print operation.
use clap::Parser;
use eyre::{ensure, eyre, Result};
use serde_json::json;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
#[path = "../tests/barcode_support/mod.rs"]
mod support;

#[derive(Parser)]
struct Args {
    /// Fetch each case once, sequentially. Output must not already exist.
    #[arg(long)]
    capture: bool,
    #[arg(long, requires = "capture")]
    host: Option<String>,
    /// Record the independently checked model, DPI and firmware.
    #[arg(long, requires = "capture")]
    device: Option<String>,
    /// Write local PNGs and directional diffs outside the fixture directory.
    #[arg(long)]
    artifacts: Option<PathBuf>,
    /// Exit unsuccessfully unless every format is nonblank and pixel-identical.
    #[arg(long)]
    strict: bool,
    output: PathBuf,
}
fn create(path: PathBuf, bytes: &[u8]) -> Result<()> {
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(bytes)?;
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    if args.capture {
        let host = reqwest::Url::parse(
            args.host
                .as_deref()
                .ok_or_else(|| eyre!("--capture requires --host"))?,
        )?;
        ensure!(
            matches!(host.scheme(), "http" | "https")
                && host.username().is_empty()
                && host.password().is_none()
                && host.query().is_none()
                && host.fragment().is_none(),
            "invalid printer URL"
        );
        let device = args
            .device
            .as_deref()
            .ok_or_else(|| eyre!("--capture requires --device"))?;
        ensure!(
            !args.output.exists(),
            "capture output already exists; use a new directory"
        );
        fs::create_dir_all(&args.output)?;
        let origin = host.origin();
        let client = reqwest::Client::builder()
            .http1_title_case_headers()
            .no_proxy()
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() >= 3 || attempt.url().origin() != origin {
                    attempt.error("preview redirect rejected")
                } else {
                    attempt.follow()
                }
            }))
            .build()?;
        // Reset preview layout state separately so each case is independent of
        // its predecessor. Empty formats need not yield a fetchable image.
        // Same protocol as zpl_to_png; see docs/printer-accuracy.md.
        let reset =
            "^XA^PMN^PA0,0,0,0^FPH,0^CVN^BY2,3,100^CI27^CF0,32,0^FWN^LH0,0^LS0^LT0^PON^LRN^XZ";
        let mut manifest = json!({"schema":"zpl-printer-barcode-comparison-v1","host":host.as_str(),"device":device,"dpi":203,"width":832,"height":1218,"preview_reset_zpl":reset,"captured_unix_seconds":SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),"preview_requests":support::CASES.len(),"cases":{}});
        for (i, case) in support::CASES.iter().enumerate() {
            client
                .post(host.join("zpl")?)
                .form(&[
                    ("dev", "R"),
                    ("oname", "TEST1"),
                    ("otype", "ZPL"),
                    ("username", ""),
                    ("pw", ""),
                    ("data", reset),
                    ("prev", "Preview Label"),
                ])
                .send()
                .await?
                .error_for_status()?;
            let zpl = support::request(case);
            create(
                args.output.join(format!("{}.zpl", case.name)),
                zpl.as_bytes(),
            )?;
            let png = zebra_http_api::zpl_to_png(client.clone(), host.clone(), &zpl).await?;
            let observation = support::observation(&zpl, &png)?;
            create(args.output.join(format!("{}.png", case.name)), &png)?;
            eprintln!(
                "{}/{} {}: {}",
                i + 1,
                support::CASES.len(),
                case.name,
                observation
                    .get("comparison")
                    .unwrap_or(&observation["local_error"])
            );
            manifest["cases"][case.name] = observation;
        }
        create(
            args.output.join("manifest.json"),
            &serde_json::to_vec_pretty(&manifest)?,
        )?;
    }
    if let Some(dir) = &args.artifacts {
        ensure!(!dir.exists(), "artifact output already exists");
        fs::create_dir_all(dir)?;
    }
    let mut exact = 0;
    let mut report=String::from("# Printer barcode comparison\n\nNo registration, scaling, or cropping. Magenta: printer only; cyan: local only.\n\n| Format | Printer ink | Different pixels | Result |\n| --- | ---: | ---: | --- |\n");
    for case in support::CASES {
        let record = support::verify(&args.output, case)?;
        let matched = record["comparison"]["exact"] == true
            && record["printer"]["ink"].as_u64().unwrap_or(0) > 0;
        exact += usize::from(matched);
        let status = if record["printer"]["ink"] == 0 {
            "printer blank"
        } else if record.get("local_error").is_some() {
            "local unsupported"
        } else if matched {
            "exact"
        } else {
            "different"
        };
        let pixels = record["comparison"]["printer_only"]
            .as_u64()
            .zip(record["comparison"]["local_only"].as_u64())
            .map(|(a, b)| (a + b).to_string())
            .unwrap_or_else(|| "—".into());
        report.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            case.name, record["printer"]["ink"], pixels, status
        ));
        if let Some(dir) = &args.artifacts {
            let reference = raster_diff::Raster::decode_png(&fs::read(
                args.output.join(format!("{}.png", case.name)),
            )?)
            .map_err(|e| eyre!(e))?;
            if let Ok(local) = support::local(&support::request(case)) {
                create(
                    dir.join(format!("{}-local.png", case.name)),
                    &raster_diff::Png::encode_gray(&local, 203)?,
                )?;
                let diff = raster_diff::compare(&reference, &local, true).map_err(|e| eyre!(e))?;
                create(
                    dir.join(format!("{}-diff.png", case.name)),
                    &diff.png(1).map_err(|e| eyre!(e))?,
                )?;
            }
        }
    }
    println!(
        "{report}\n{exact}/{} nonblank exact matches.",
        support::CASES.len()
    );
    if args.strict {
        ensure!(
            exact == support::CASES.len(),
            "printer pixel parity is incomplete"
        );
    }
    Ok(())
}
