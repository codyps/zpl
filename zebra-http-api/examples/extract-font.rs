use clap::Parser;
use eyre::{bail, ensure, eyre, Result};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use zpl::{
    font_extract::{self, Settings},
    output::Raster,
};
mod font_support;

#[derive(Parser)]
#[command(about = "Extract resident bitmap fonts through Zebra's preview API. No print requests.")]
struct Args {
    #[arg(long)]
    host: String,
    #[arg(long, default_value = "0")]
    font: char,
    #[arg(long, default_value_t = 32)]
    height: u32,
    #[arg(long, default_value_t = 0)]
    width: u32,
    #[arg(long, default_value_t = 203)]
    dpi: u32,
    /// Printable ASCII subset; default is all 95 characters.
    #[arg(long)]
    characters: Option<String>,
    #[arg(long, default_value_t = 8)]
    batch_size: usize,
    #[arg(long, default_value_t = 30.0)]
    timeout: f64,
    #[arg(long, default_value_t = 0.5)]
    delay: f64,
    /// Compare this text's composed glyphs against a separate printer preview.
    #[arg(long)]
    verify_text: Option<String>,
    /// Reuse matching saved pages and download missing pages.
    #[arg(long)]
    resume: bool,
    /// Only read saved pages, without making any network requests.
    #[arg(long)]
    offline: bool,
    output: PathBuf,
}
const LIMIT: usize = 16 * 1024 * 1024;
fn read(path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(LIMIT as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= LIMIT, "saved file exceeds 16 MiB");
    Ok(bytes)
}
fn json_write(path: &Path, value: &Value) -> Result<()> {
    write_atomic(
        path,
        format!("{}\n", serde_json::to_string_pretty(value)?).as_bytes(),
    )
}
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().unwrap_or_default().to_string_lossy()
    ));
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    let result = (|| {
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result?;
    Ok(())
}
fn check_config(args: &Args, config: &Value) -> Result<()> {
    if args.output.exists() {
        ensure!(
            args.resume || args.offline,
            "output exists; choose another directory or use --resume"
        );
        let saved: Value = serde_json::from_slice(&read(&args.output.join("capture.json"))?)?;
        ensure!(
            &saved == config,
            "saved capture configuration differs from these arguments"
        );
    } else {
        ensure!(!args.offline, "offline capture directory does not exist");
        fs::create_dir_all(&args.output)?;
        json_write(&args.output.join("capture.json"), config)?;
    }
    Ok(())
}
struct Capture<'a> {
    args: &'a Args,
    client: reqwest::Client,
    host: reqwest::Url,
    username: String,
    password: String,
}
impl Capture<'_> {
    async fn page(&self, name: &str, zpl: &str) -> Result<(Vec<u8>, Raster)> {
        let request = self.args.output.join(format!("{name}.zpl"));
        let png = self.args.output.join(format!("{name}.png"));
        if request.exists() {
            ensure!(
                read(&request)? == zpl.as_bytes(),
                "saved {name} ZPL differs from this request"
            );
        }
        let data = if png.exists() {
            ensure!(request.exists(), "cached PNG has no matching ZPL");
            read(&png)?
        } else {
            ensure!(!self.args.offline, "missing offline page {name}");
            write_atomic(&request, zpl.as_bytes())?;
            if name != "page-000" {
                tokio::time::sleep(Duration::try_from_secs_f64(self.args.delay)?).await;
            }
            let bytes = zebra_http_api::zpl_to_png_with_credentials(
                self.client.clone(),
                self.host.clone(),
                zpl,
                &self.username,
                &self.password,
            )
            .await?;
            Raster::decode_png(&bytes).map_err(|e| eyre!(e))?;
            write_atomic(&png, &bytes)?;
            bytes
        };
        let image = Raster::decode_png(&data).map_err(|e| eyre!(e))?;
        Ok((data, image))
    }
}
fn capture_config(args: &Args, codes: &[u8]) -> Value {
    json!({"schema":"zpl-preview-bitmap-font-v1","font":args.font.to_string(),"requested_height":args.height,"requested_width":args.width,"dpi":args.dpi,"codepoints":codes,"batch_size":args.batch_size,"source":args.host,"coordinates":"printer dots; left/top relative to FT baseline; rows top-to-bottom, MSB-first"})
}
async fn run(args: Args) -> Result<()> {
    let settings = Settings {
        font: args.font,
        height: args.height,
        width: args.width,
        dpi: args.dpi,
    };
    settings.validate().map_err(|e| eyre!(e))?;
    ensure!(
        (1..=16).contains(&args.batch_size)
            && args.timeout.is_finite()
            && args.timeout > 0.
            && args.timeout <= 3600.
            && args.delay.is_finite()
            && (0. ..=3600.).contains(&args.delay),
        "invalid batch size, timeout or delay"
    );
    let mut host = reqwest::Url::parse(&args.host)?;
    ensure!(
        matches!(host.scheme(), "http" | "https")
            && host.host_str().is_some()
            && host.username().is_empty()
            && host.password().is_none()
            && host.query().is_none()
            && host.fragment().is_none(),
        "host must be HTTP(S) without embedded credentials, query or fragment"
    );
    if !host.path().ends_with('/') {
        host.set_path(&format!("{}/", host.path()));
    }
    let mut codes: Vec<u8> = args
        .characters
        .as_ref()
        .map(|s| s.as_bytes().to_vec())
        .unwrap_or_else(|| (32..=126).collect());
    codes.sort_unstable();
    codes.dedup();
    ensure!(
        !codes.is_empty() && codes.iter().all(|c| (32..=126).contains(c)),
        "characters must be nonempty printable ASCII"
    );
    if let Some(text) = &args.verify_text {
        ensure!(
            !text.is_empty() && text.bytes().all(|c| codes.contains(&c)),
            "verification text must use captured characters"
        );
    }
    let config = capture_config(&args, &codes);
    check_config(&args, &config)?;
    let client = reqwest::Client::builder()
        .http1_title_case_headers()
        .timeout(Duration::try_from_secs_f64(args.timeout)?)
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 10 {
                attempt.error("too many preview redirects")
            } else if attempt
                .previous()
                .first()
                .is_some_and(|u| u.origin() != attempt.url().origin())
                || !attempt.url().username().is_empty()
                || attempt.url().password().is_some()
            {
                attempt.error("cross-origin preview redirect rejected")
            } else {
                attempt.follow()
            }
        }))
        .build()?;
    let capture = Capture {
        args: &args,
        client,
        host,
        username: std::env::var("ZPL_USERNAME").unwrap_or_default(),
        password: std::env::var("ZPL_PASSWORD").unwrap_or_default(),
    };
    let (mut glyphs, mut captures) = (Vec::new(), Vec::new());
    for (page, batch) in codes.chunks(args.batch_size).enumerate() {
        let plan = font_extract::page_plan(batch, settings).map_err(|e| eyre!(e))?;
        let (data, image) = capture.page(&format!("page-{page:03}"), &plan.zpl).await?;
        glyphs.extend(font_extract::extract_page(&image, &plan).map_err(|e| eyre!(e))?);
        captures.push(json!({"page":page,"sha256":font_support::sha256(&data)}));
        eprintln!("page {}: {}/{} glyphs", page + 1, glyphs.len(), codes.len());
    }
    let mut document = config;
    document["captures"] = json!(captures);
    document["glyphs"]=json!(glyphs.iter().map(|g|json!({"codepoint":g.codepoint,"character":char::from(g.codepoint).to_string(),"advance":g.advance,"left":g.left,"top":g.top,"width":g.width,"height":g.height,"bitmap":g.bitmap.iter().map(|r|font_extract::hex(r)).collect::<Vec<_>>()})).collect::<Vec<_>>());
    if let Some(text) = &args.verify_text {
        let (zpl, expected) =
            font_extract::verification_plan(&glyphs, settings, text).map_err(|e| eyre!(e))?;
        let (data, actual) = capture.page("verification", &zpl).await?;
        let diff = image_diff::compare(&expected, &actual, false).map_err(|e| eyre!(e))?;
        let report = json!({"text":text,"different_pixels":diff.different_pixels(),"sha256":font_support::sha256(&data)});
        json_write(&args.output.join("verification.json"), &report)?;
        // The same directional diff used by png-diff also diagnoses font verification.
        write_atomic(
            &args.output.join("verification-diff.png"),
            &diff.png(1).map_err(|e| eyre!(e))?,
        )?;
        if !diff.matches() {
            bail!(
                "verification differs by {} pixels; see verification-diff.png",
                diff.different_pixels()
            )
        }
        document["verification"] = report;
        eprintln!("verification: exact pixel match");
    }
    let packed = font_extract::pack(&glyphs, settings).map_err(|e| eyre!(e))?;
    let bdf = font_extract::bdf(&glyphs, settings).map_err(|e| eyre!(e))?;
    json_write(&args.output.join("font.json"), &document)?;
    write_atomic(&args.output.join("font.bdf"), bdf.as_bytes())?;
    write_atomic(&args.output.join("font.zbf"), &packed)?;
    println!(
        "Extracted {} glyphs to {}",
        glyphs.len(),
        args.output.display()
    );
    Ok(())
}
#[tokio::main]
async fn main() -> std::process::ExitCode {
    match run(Args::parse()).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("font extraction failed: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!(
                "zpl-font-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            )))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn args(path: &Path) -> Args {
        Args::parse_from([
            "extract-font",
            "--host",
            "http://127.0.0.1:9/",
            "--height",
            "8",
            "--characters",
            " A",
            "--verify-text",
            "A A",
            "--offline",
            path.to_str().unwrap(),
        ])
    }
    fn encode(r: &Raster) -> Vec<u8> {
        let rgb: Vec<u8> = r.pixels.iter().flat_map(|&p| [p, p, p]).collect();
        zpl::output::Png::encode_rgb(r.width, r.height, &rgb).unwrap()
    }
    fn saved_pages(path: &Path) {
        fs::create_dir(path).unwrap();
        let a = args(path);
        json_write(&path.join("capture.json"), &capture_config(&a, b" A")).unwrap();
        let s = Settings {
            font: '0',
            height: 8,
            width: 0,
            dpi: 203,
        };
        let plan = font_extract::page_plan(b" A", s).unwrap();
        let mut image = Raster {
            width: plan.width,
            height: plan.height,
            pixels: vec![255; (plan.width * plan.height) as usize],
        };
        // Synthetic font: pipe = 1x5 at bearing 1; A = 3x4 at bearing 1.
        let draw = |image: &mut Raster, text: &[u8], mut x: u32, base: u32| {
            for &c in text {
                let (advance, top, rows): (u32, i32, &[&str]) = match c {
                    b'|' => (3, -5, &["1", "1", "1", "1", "1"]),
                    b'A' => (5, -4, &["010", "101", "111", "101"]),
                    b' ' => (4, 0, &[]),
                    _ => panic!(),
                };
                for (y, row) in rows.iter().enumerate() {
                    for (dx, p) in row.bytes().enumerate() {
                        if p == b'1' {
                            image.pixels[((base as i32 + top + y as i32) as u32 * image.width
                                + x
                                + 1
                                + dx as u32) as usize] = 0;
                        }
                    }
                }
                x += advance;
            }
        };
        for (i, code) in [None, Some(b' '), Some(b'A')].into_iter().enumerate() {
            let x = i as u32 % 2 * 80 + 16;
            let base = i as u32 / 2 * 88 + 32;
            let text = code.map(|c| vec![c]).unwrap_or_default();
            draw(&mut image, &text, x, base);
            let mut probe = vec![b'|'];
            probe.extend(text);
            probe.push(b'|');
            draw(&mut image, &probe, x, base + 32);
        }
        fs::write(path.join("page-000.zpl"), &plan.zpl).unwrap();
        fs::write(path.join("page-000.png"), encode(&image)).unwrap();
        let glyphs = font_extract::extract_page(&image, &plan).unwrap();
        let (zpl, r) = font_extract::verification_plan(&glyphs, s, "A A").unwrap();
        fs::write(path.join("verification.zpl"), zpl).unwrap();
        fs::write(path.join("verification.png"), encode(&r)).unwrap();
    }
    #[tokio::test]
    async fn offline_resume_and_failed_verification_preserve_exports() {
        let temp = Temp::new();
        saved_pages(&temp.0);
        run(args(&temp.0)).await.unwrap();
        let original = fs::read(temp.0.join("font.json")).unwrap();
        let report: Value = serde_json::from_slice(&original).unwrap();
        assert_eq!(report["glyphs"][0]["advance"], 4);
        let (settings, packed_glyphs) =
            font_extract::unpack(&fs::read(temp.0.join("font.zbf")).unwrap()).unwrap();
        assert_eq!((settings.font, settings.height), ('0', 8));
        assert_eq!(
            packed_glyphs.iter().map(|g| g.advance).collect::<Vec<_>>(),
            [4, 5]
        );
        assert_eq!(report["verification"]["different_pixels"], 0);
        run(args(&temp.0)).await.unwrap();
        assert_eq!(original, fs::read(temp.0.join("font.json")).unwrap());
        let mut r =
            Raster::decode_png(&fs::read(temp.0.join("verification.png")).unwrap()).unwrap();
        r.pixels[0] = 0;
        fs::write(temp.0.join("verification.png"), encode(&r)).unwrap();
        assert!(run(args(&temp.0))
            .await
            .unwrap_err()
            .to_string()
            .contains("differs by 1"));
        assert_eq!(original, fs::read(temp.0.join("font.json")).unwrap());
        assert!(temp.0.join("verification-diff.png").exists());
    }
    #[tokio::test]
    async fn changed_config_and_missing_cache_fail_offline() {
        let temp = Temp::new();
        saved_pages(&temp.0);
        let mut a = args(&temp.0);
        a.font = 'A';
        assert!(run(a)
            .await
            .unwrap_err()
            .to_string()
            .contains("configuration differs"));
        fs::remove_file(temp.0.join("page-000.png")).unwrap();
        assert!(run(args(&temp.0))
            .await
            .unwrap_err()
            .to_string()
            .contains("missing offline"));
    }
}
