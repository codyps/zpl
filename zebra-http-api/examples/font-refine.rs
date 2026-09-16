//! Bounded preview campaign. Defaults to an offline dry-run manifest.
use clap::Parser;
use eyre::{ensure, eyre, Result};
use image_diff::Raster;
use serde_json::json;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
mod font_refine;
mod font_support;
#[derive(Parser)]
struct Args {
    #[arg(long)]
    host: String,
    /// Four adaptive spacing and missing-font control previews.
    #[arg(long,conflicts_with_all=["capture","analyze","install_calibration","dropout_calibration","cleanup_calibration"])]
    diagnostic: bool,
    /// Delete only this campaign's installed calibration RAM objects and verify absence.
    #[arg(long,conflicts_with_all=["capture","offline","analyze","install_calibration","dropout_calibration"])]
    cleanup_calibration: bool,
    /// Install five dedicated RAM fonts and capture 30 dropout-control probes.
    #[arg(long, conflicts_with_all=["capture","analyze","install_calibration"])]
    dropout_calibration: bool,
    /// Put scan-control instructions in every glyph as well as prep.
    #[arg(long, requires = "dropout_calibration")]
    glyph_controls: bool,
    /// Analyze the complete cached development campaign. No acceptance pixels are read.
    #[arg(long, conflicts_with_all=["capture","offline","install_calibration"])]
    analyze: bool,
    /// Fetch missing previews sequentially. Default writes requests only.
    #[arg(long)]
    capture: bool,
    /// Install the two generated fonts into otherwise unused RAM objects via TCP 9100.
    #[arg(long, conflicts_with_all=["capture","offline"])]
    install_calibration: bool,
    /// Analyze cached development captures without network access.
    #[arg(long, conflicts_with = "capture")]
    offline: bool,
    #[arg(long, default_value_t = 200)]
    request_cap: usize,
    /// Optional exact group filter for capture; the manifest always contains all groups.
    #[arg(long)]
    group: Option<String>,
    #[arg(long, default_value = "ZD621 203 dpi; firmware not recorded")]
    device_note: String,
    output: PathBuf,
}
fn read(path: &Path) -> Result<Vec<u8>> {
    let mut b = vec![];
    fs::File::open(path)?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut b)?;
    ensure!(b.len() <= 16 * 1024 * 1024, "file too large");
    Ok(b)
}
fn immutable(path: &Path, data: &[u8]) -> Result<()> {
    if path.exists() {
        ensure!(
            read(path)? == data,
            "saved artifact differs: {}",
            path.display()
        );
    } else {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?
            .write_all(data)?;
    }
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    let a = Args::parse();
    let host = reqwest::Url::parse(&a.host)?;
    ensure!(
        matches!(host.scheme(), "http" | "https")
            && host.host_str().is_some()
            && host.username().is_empty()
            && host.password().is_none()
            && host.query().is_none()
            && host.fragment().is_none(),
        "invalid host URL"
    );
    let _lock = if a.capture
        || a.install_calibration
        || a.cleanup_calibration
        || ((a.dropout_calibration || a.diagnostic) && !a.offline)
    {
        Some(font_refine::transport::Lock::acquire(&a.output)?)
    } else {
        None
    };
    if a.cleanup_calibration {
        font_refine::transport::cleanup(&host, &a.output).await?;
        return Ok(());
    }
    if a.diagnostic {
        font_refine::diagnostic::run(&host, &a.output, a.offline, a.request_cap).await?;
        return Ok(());
    }
    if a.dropout_calibration {
        font_refine::dropout::run(&host, &a.output, a.glyph_controls, a.offline, a.request_cap)
            .await?;
        return Ok(());
    }
    if a.analyze {
        let report = font_refine::analyze::run(&a.output)?;
        fs::write(
            a.output.join("analysis.json"),
            serde_json::to_vec_pretty(&report)?,
        )?;
        println!("Wrote development analysis; sealed captures remain unread");
        return Ok(());
    }
    let pages = font_refine::campaign::plan();
    ensure!(
        pages.len() <= a.request_cap && a.request_cap <= 200,
        "{} planned requests exceed cap (maximum 200)",
        pages.len()
    );
    if let Some(g) = &a.group {
        ensure!(pages.iter().any(|p| &p.group == g), "unknown group");
    }
    ensure!(
        !a.offline || a.output.exists(),
        "offline capture directory is missing"
    );
    fs::create_dir_all(&a.output)?;
    immutable(
        &a.output.join("manifest.json"),
        &serde_json::to_vec_pretty(
            &json!({"schema":"font-refine-v1","host":host.as_str(),"device":a.device_note,"dpi":203,"requests":pages.len(),"sealed_policy":"pixels excluded from development report and model selection","pages":pages.iter().map(|p|p.metadata()).collect::<Vec<_>>()}),
        )?,
    )?;
    for shift in [false, true] {
        immutable(
            &a.output.join(if shift {
                "calibration-shift.ttf"
            } else {
                "calibration-plain.ttf"
            }),
            &font_refine::sfnt::font(shift),
        )?;
    }
    println!(
        "{} planned preview requests; {} glyph/text observations",
        pages.len(),
        pages.iter().map(|p| p.probes.len()).sum::<usize>()
    );
    let client = reqwest::Client::builder()
        .http1_title_case_headers()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    if a.install_calibration {
        use tokio::io::AsyncWriteExt;
        let directory = client
            .get(host.join("dir?dev=R&otype=TTF")?)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        ensure!(
            directory.contains("Directory of: R:*.TTF"),
            "invalid RAM directory response"
        );
        ensure!(
            !directory.contains("R:ZRFN.TTF") && !directory.contains("R:ZRFH.TTF"),
            "calibration RAM names already exist; refusing to overwrite"
        );
        let mut stream = tokio::time::timeout(
            Duration::from_secs(10),
            tokio::net::TcpStream::connect((host.host_str().unwrap(), 9100)),
        )
        .await??;
        let mut hashes = vec![];
        for shift in [false, true] {
            let name = if shift { "R:ZRFH.TTF" } else { "R:ZRFN.TTF" };
            let bytes = font_refine::sfnt::font(shift);
            let header = format!("~DY{name},B,T,{},,", bytes.len());
            tokio::time::timeout(Duration::from_secs(10), stream.write_all(header.as_bytes()))
                .await??;
            tokio::time::timeout(Duration::from_secs(10), stream.write_all(&bytes)).await??;
            hashes.push(
                json!({"object":name,"bytes":bytes.len(),"sha256":font_support::sha256(&bytes)}),
            );
        }
        stream.shutdown().await?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        let directory = client
            .get(host.join("dir?dev=R&otype=TTF")?)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        ensure!(
            directory.contains("R:ZRFN.TTF") && directory.contains("R:ZRFH.TTF"),
            "download not confirmed in RAM directory"
        );
        immutable(
            &a.output.join("calibration-install.json"),
            &serde_json::to_vec_pretty(
                &json!({"transport":"TCP 9100 binary ~DY; no label format or print command", "fonts":hashes}),
            )?,
        )?;
        println!("Installed calibration fonts in RAM; directory confirms both objects");
        return Ok(());
    }
    let user = std::env::var("ZPL_USERNAME").unwrap_or_default();
    let password = std::env::var("ZPL_PASSWORD").unwrap_or_default();
    if a.capture
        && pages.iter().any(|p| {
            p.group == "calibration"
                && a.group.as_ref().is_none_or(|g| g == "calibration")
                && !a
                    .output
                    .join("development")
                    .join(format!("{}.png", p.name))
                    .exists()
        })
    {
        ensure!(
            a.output.join("calibration-install.json").exists(),
            "install calibration fonts with --install-calibration before capturing"
        );
        let listing = client
            .get(host.join("dir?dev=R&otype=TTF")?)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        ensure!(
            listing.contains("R:ZRFN.TTF") && listing.contains("R:ZRFH.TTF"),
            "calibration fonts are missing from RAM; reinstall them before capture"
        );
    }
    let mut records = vec![];
    for p in &pages {
        let dir = a.output.join(if p.group.starts_with("sealed-") {
            "sealed"
        } else {
            "development"
        });
        fs::create_dir_all(&dir)?;
        let z = p.zpl();
        immutable(&dir.join(format!("{}.zpl", p.name)), z.as_bytes())?;
        if a.group.as_ref().is_some_and(|g| g != &p.group) {
            continue;
        }
        let path = dir.join(format!("{}.png", p.name));
        if !path.exists() && a.capture {
            tokio::time::sleep(Duration::from_millis(500)).await;
            font_refine::transport::reserve(&a.output, &p.name, a.request_cap)?;
            let data = zebra_http_api::zpl_to_png_with_credentials(
                client.clone(),
                host.clone(),
                &z,
                &user,
                &password,
            )
            .await?;
            let r = Raster::decode_png_with_threshold(&data, None).map_err(|e| eyre!(e))?;
            font_refine::campaign::validate(&r, p)?;
            immutable(&path, &data)?;
            println!("captured {}", p.name);
        }
        if a.offline {
            ensure!(path.exists(), "missing offline capture {}", p.name);
        }
        if path.exists() {
            let data = read(&path)?;
            let r = Raster::decode_png_with_threshold(&data, None).map_err(|e| eyre!(e))?;
            font_refine::campaign::validate(&r, p)?;
            immutable(
                &dir.join(format!("{}.sha256", p.name)),
                font_support::sha256(&data).as_bytes(),
            )?;
            if !p.group.starts_with("sealed-") {
                records.push(json!({"page":p.name,"png_sha256":font_support::sha256(&data),"measurements":p.probes.iter().map(|probe|font_refine::campaign::measurement(&font_refine::campaign::normalize(&r,probe))).collect::<Vec<_>>()}));
            }
        }
    }
    if a.capture || a.offline {
        let report = serde_json::to_vec_pretty(
            &json!({"schema":"font-refine-measurements-v1","selected_group":a.group,"pages":records}),
        )?;
        fs::write(a.output.join("development-measurements.json"), report)?;
    }
    Ok(())
}
