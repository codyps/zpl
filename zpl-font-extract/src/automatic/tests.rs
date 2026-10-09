use super::*;
use raster_diff::Png;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "zpl-auto-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Independent tiny printer interprets emitted ZPL, not extraction plan geometry.
// Deliberately unfamiliar nominal dimensions (7x11), signed bearings, variable
// advances, overlapping glyphs, and ink bounds smaller than the nominal cell.
fn fixture(key: u8) -> Glyph {
    if key == 32 {
        return Glyph {
            key,
            advance: 4,
            ..Glyph::default()
        };
    }
    if key == 0 {
        return Glyph {
            key,
            ..Glyph::default()
        };
    }
    Glyph {
        key,
        advance: if key == 67 { 2 } else { 6 + u16::from(key % 3) },
        left: if key == 67 { -1 } else { 1 },
        top: -7,
        width: 5,
        height: 5,
        bitmap: vec![
            "f8".into(),
            format!("{:02x}", 0x88 | ((key & 7) << 4)),
            "a8".into(),
            "88".into(),
            "f8".into(),
        ],
    }
}
fn printer_png(zpl: &str) -> Result<Vec<u8>> {
    let (mut width, mut height) = (0, 0);
    let (mut x, mut y) = (0i32, 0i32);
    let mut fo = false;
    let (mut sx, mut sy) = (1i32, 1i32);
    let mut pixels = Vec::new();
    let mut map = (0u8..=255).collect::<Vec<_>>();
    let mut encoding = 27;
    for command in zpl.split('^').skip(1) {
        let (cmd, args) = command.split_at(2);
        match cmd {
            "PW" => width = args.parse::<u32>()?,
            "LL" => {
                height = args.parse::<u32>()?;
                pixels = vec![255; (width * height) as usize];
            }
            "CI" => {
                let ns = args
                    .split(',')
                    .map(str::parse::<u16>)
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                encoding = ns[0];
                for pair in ns[1..].as_chunks::<2>().0 {
                    map[usize::from(pair[1])] = pair[0] as u8;
                }
            }
            "FO" | "FT" => {
                let ns = args
                    .split(',')
                    .map(str::parse::<i32>)
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                x = ns[0];
                y = ns[1];
                fo = cmd == "FO";
            }
            "GB" => {
                let ns = args
                    .split(',')
                    .take(2)
                    .map(str::parse::<i32>)
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                for dy in 0..ns[1] {
                    for dx in 0..ns[0] {
                        put(&mut pixels, width, height, x + dx, y + dy)?;
                    }
                }
            }
            "GS" | "AA" | "AB" | "AC" | "AD" | "AE" | "AF" | "AG" | "AH" | "A@" => {
                let ns = args
                    .split(',')
                    .skip(1)
                    .take(2)
                    .map(str::parse::<f64>)
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                sy = (ns[0] / 11.0).round().clamp(1.0, 10.0) as i32;
                sx = (ns[1] / 7.0).round().clamp(1.0, 10.0) as i32;
            }
            "FD" => {
                let bytes = args
                    .split('_')
                    .skip(1)
                    .map(|s| u8::from_str_radix(s, 16))
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                let keys = if encoding == 28 {
                    String::from_utf8(bytes)?
                        .chars()
                        .map(|c| c as u8)
                        .collect::<Vec<_>>()
                } else {
                    bytes
                };
                for key in keys {
                    let g = fixture(if encoding == 0 {
                        map[usize::from(key)]
                    } else {
                        key
                    });
                    // Render directly from hex rows (independent of Glyph::points).
                    for (row, bits) in g.bitmap.iter().enumerate() {
                        let b = u8::from_str_radix(bits, 16)?;
                        for column in 0..g.width {
                            if b & (128 >> column) != 0 {
                                for dy in 0..sy {
                                    for dx in 0..sx {
                                        put(
                                            &mut pixels,
                                            width,
                                            height,
                                            x + (i32::from(g.left) + i32::from(column)) * sx + dx,
                                            y + (i32::from(g.top)
                                                + row as i32
                                                + if fo { 8 } else { 0 })
                                                * sy
                                                + dy,
                                        )?;
                                    }
                                }
                            }
                        }
                    }
                    x += i32::from(g.advance) * sx;
                }
            }
            _ => {}
        }
    }
    Ok(Png::encode_gray(
        &Raster {
            width,
            height,
            pixels,
        },
        203,
    )?)
}
fn put(p: &mut [u8], w: u32, h: u32, x: i32, y: i32) -> Result<()> {
    ensure!(
        x >= 0 && y >= 0 && x < w as i32 && y < h as i32,
        "fixture clipped"
    );
    p[y as usize * w as usize + x as usize] = 0;
    Ok(())
}
#[derive(Default)]
struct Fake {
    calls: usize,
    corrupt: Option<&'static str>,
}
impl Capture for Fake {
    async fn png(&mut self, p: &Page, repeat: bool) -> Result<Vec<u8>> {
        self.calls += 1;
        let png = printer_png(&p.zpl()?)?;
        if self.corrupt == Some("verification") && p.stage == "verification"
            || self.corrupt == Some("repeat") && repeat
            || self.corrupt == Some("identity")
        {
            let mut r = decode(&png)?;
            if self.corrupt == Some("verification") {
                let t = &p.tiles[0];
                let i = ((t.y + t.oy - 3) * r.width + t.x + t.ox + 2) as usize;
                r.pixels[i] ^= 255;
            } else {
                r.pixels[0] = 0;
            }
            return Ok(Png::encode_gray(&r, 203)?);
        }
        Ok(png)
    }
}
fn config() -> Config {
    Config {
        fonts: vec!["A".into()],
        codes: vec![32, 65, 66, 67, 200],
        bound: 24,
        width: 384,
        height: 768,
        ..Config::default()
    }
}
#[tokio::test]
async fn automatic_metrics_variable_advances_and_fresh_holdout() {
    let mut printer = Fake::default();
    let d = calibrate(&config(), &mut printer).await.unwrap();
    assert!(printer.calls > 5);
    let face = &d.fonts[0];
    assert_eq!(
        face.metrics,
        Metrics {
            cell_height: 11,
            cell_width: 7,
            baseline: 9,
            space_advance: 4
        }
    );
    for g in &face.glyphs {
        assert_eq!(*g, fixture(g.key));
    }
    d.validate().unwrap();
    assert!(d.provenance["lineage"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v["stage"] == "calibration-1"));
}
#[tokio::test]
async fn utf8_and_source_keys_do_not_become_unicode_by_accident() {
    for source in [false, true] {
        let mut c = config();
        c.source = source;
        c.encoding = if source { 0 } else { 28 };
        c.codes.extend([94, 95, 126]);
        if source {
            c.codes.extend([0, 9]);
        }
        let d = calibrate(&c, &mut Fake::default()).await.unwrap();
        for g in &d.fonts[0].glyphs {
            assert_eq!(*g, fixture(g.key));
        }
        if source {
            assert_eq!(d.provenance["omitted_format_inputs"], json!([9]));
            assert!(d.provenance["excluded_inputs"]["A"]["0"].is_string());
        }
    }
}
#[tokio::test]
async fn rejects_changed_holdouts_unstable_repeats_and_wrong_registration() {
    for corrupt in ["verification", "repeat", "identity"] {
        let error = calibrate(
            &config(),
            &mut Fake {
                corrupt: Some(corrupt),
                ..Fake::default()
            },
        )
        .await
        .unwrap_err();
        assert!(format!("{error:?}").contains(if corrupt == "repeat" {
            "unstable"
        } else if corrupt == "verification" {
            "JSON does not reproduce"
        } else {
            "identity/registration"
        }));
    }
}
#[test]
fn all_native_dimensions_can_be_distinguished() {
    for native in 1..=256 {
        let mut cs = (1..=256).collect::<Vec<_>>();
        while cs.len() > 1 {
            let q = distinguish(&cs, 256).unwrap();
            let scale = zoom(q, native);
            cs.retain(|n| zoom(q, *n) == scale);
        }
        assert_eq!(cs, [native]);
    }
}
#[test]
fn bitmap_model_rejects_non_replication() {
    let g = fixture(65);
    let mut bad = g.clone();
    bad.bitmap[2] = "f8".into();
    assert!(stretch(&g, &bad, false).is_err());
    assert!(Config {
        fonts: vec!["A^XZ".into()],
        ..config()
    }
    .validate()
    .is_err());
    assert!(Config {
        width: 1,
        ..config()
    }
    .validate()
    .is_err());
}
#[tokio::test]
async fn json_tampering_and_compact_export() {
    let mut d = calibrate(&config(), &mut Fake::default()).await.unwrap();
    let t = Temp::new();
    compile::compile(
        &crate::collection::Collection::from_verified(&d).unwrap(),
        &t.0.join("rust"),
    )
    .unwrap();
    assert!(compile::compile(
        &crate::collection::Collection::from_verified(&d).unwrap(),
        &t.0.join("rust")
    )
    .is_err());
    let module = fs::read_to_string(t.0.join("rust/fonts.rs")).unwrap();
    assert!(module.contains("zpl_bitmap_fonts"));
    d.fonts[0].glyphs[0].advance += 1;
    assert!(d.validate().is_err());
    assert!(crate::collection::Collection::from_verified(&d).is_err());
    assert!(!t.0.join("bad").exists());
}

struct Server {
    host: String,
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    task: Option<std::thread::JoinHandle<usize>>,
}
impl Server {
    fn new() -> Self {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            sync::{atomic::AtomicBool, Arc},
            thread,
            time::Duration,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let host = format!("http://{}/", listener.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let task = thread::spawn(move || {
            let mut png = vec![];
            let mut calls = 0;
            let mut object_name: Option<String> = None;
            while !done.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(s) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(2));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                // Accepted sockets can inherit the listener's nonblocking mode on macOS.
                // Poll only accept; read each HTTP request with the timeout below.
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = vec![];
                let end;
                loop {
                    let mut b = [0; 4096];
                    let n = stream.read(&mut b).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&b[..n]);
                    if let Some(e) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
                        let headers = String::from_utf8_lossy(&bytes[..e]);
                        let len = headers
                            .lines()
                            .find_map(|s| {
                                let (k, v) = s.split_once(':')?;
                                if k.eq_ignore_ascii_case("content-length") {
                                    Some(v.trim().parse::<usize>().unwrap())
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0);
                        if bytes.len() >= e + 4 + len {
                            end = e + 4;
                            break;
                        }
                    }
                }
                let body = if bytes.starts_with(b"POST /zpl ") {
                    let form = std::str::from_utf8(&bytes[end..]).unwrap();
                    let url = reqwest::Url::parse(&format!("http://localhost/?{form}")).unwrap();
                    let fields = url.query_pairs().collect::<BTreeMap<_, _>>();
                    assert_eq!(fields["prev"], "Preview Label");
                    let name = fields["oname"].as_ref();
                    assert_eq!(name.len(), 8);
                    assert_ne!(name, "TEST1");
                    if let Some(previous) = &object_name {
                        assert_eq!(previous, name);
                    } else {
                        object_name = Some(name.to_owned());
                    }
                    png = printer_png(&fields["data"]).unwrap();
                    calls += 1;
                    b"<IMG SRC=\"/preview.png\">".to_vec()
                } else {
                    assert!(bytes.starts_with(b"GET /preview.png "));
                    png.clone()
                };
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )
                .unwrap();
                stream.write_all(&body).unwrap();
            }
            calls
        });
        Self {
            host,
            stop,
            task: Some(task),
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.task.take().unwrap().join().unwrap();
    }
}
#[tokio::test]
async fn http_end_to_end_cache_replay_and_integrity() {
    let server = Server::new();
    let t = Temp::new();
    let mut printer = capture::Printer::new(&server.host, &t.0, false, 0.0, 5).unwrap();
    assert!(capture::Printer::new(&server.host, &t.0, false, 0.0, 5).is_err());
    let online = calibrate(&config(), &mut printer).await.unwrap();
    drop(printer);
    let host = server.host.clone();
    drop(server);
    let mut offline = capture::Printer::new(&host, &t.0, true, 0.0, 5).unwrap();
    let replay = calibrate(&config(), &mut offline).await.unwrap();
    assert_eq!(
        online.content_hash().unwrap(),
        replay.content_hash().unwrap()
    );
    for folder in fs::read_dir(&t.0).unwrap() {
        let path = folder.unwrap().path().join("preview.png");
        if path.exists() {
            fs::write(path, b"corrupt").unwrap();
        }
    }
    let error = calibrate(&config(), &mut offline).await.unwrap_err();
    assert!(format!("{error:?}").contains("integrity mismatch"));
}

#[tokio::test]
async fn incomplete_offline_cache_never_falls_back_to_network() {
    let t = Temp::new();
    let mut printer = capture::Printer::new("http://127.0.0.1:1/", &t.0, true, 0.0, 1).unwrap();
    let error = calibrate(&config(), &mut printer).await.unwrap_err();
    assert!(format!("{error:?}").contains("missing cached preview"));
}

#[tokio::test]
async fn compact_full_key_span_and_metric_limits() {
    let mut d = calibrate(&config(), &mut Fake::default()).await.unwrap();
    d.mapping = Mapping {
        kind: "ci0-source".into(),
        encoding: 0,
    };
    d.fonts[0].glyphs = vec![
        Glyph {
            key: 0,
            advance: 1,
            ..Glyph::default()
        },
        fixture(255),
    ];
    d.verification["content_sha256"] = json!(d.content_hash().unwrap());
    let t = Temp::new();
    compile::compile(
        &crate::collection::Collection::from_verified(&d).unwrap(),
        &t.0.join("all-keys"),
    )
    .unwrap();
    let catalog: Value =
        serde_json::from_slice(&fs::read(t.0.join("all-keys/catalog.json")).unwrap()).unwrap();
    assert_eq!(catalog["unique_records"], 2);
    d.fonts[0].glyphs[0].advance = 256;
    d.verification["content_sha256"] = json!(d.content_hash().unwrap());
    compile::compile(
        &crate::collection::Collection::from_verified(&d).unwrap(),
        &t.0.join("wide-metrics"),
    )
    .unwrap();
}

#[tokio::test]
async fn cached_capture_is_scoped_to_printer_identity() {
    let server = Server::new();
    let temp = Temp::new();
    let identity = json!({"device.unique_id":"serial","appl.name":"firmware-1"});
    let mut printer = capture::Printer::new(&server.host, &temp.0, false, 0.0, 5)
        .unwrap()
        .with_identity(identity.clone());
    calibrate(&config(), &mut printer).await.unwrap();
    drop(printer);
    let host = server.host.clone();
    drop(server);
    let mut wrong = capture::Printer::new(&host, &temp.0, true, 0.0, 5)
        .unwrap()
        .with_identity(json!({"device.unique_id":"serial","appl.name":"firmware-2"}));
    let error = calibrate(&config(), &mut wrong).await.unwrap_err();
    assert!(format!("{error:?}").contains("missing cached preview"));
    drop(wrong);
    let mut right = capture::Printer::new(&host, &temp.0, true, 0.0, 5)
        .unwrap()
        .with_identity(identity);
    calibrate(&config(), &mut right).await.unwrap();
}

#[tokio::test]
async fn unified_recovery_keeps_calibration_and_multiple_encodings() {
    use crate::collection::Encoding;
    let options = RecoveryConfig {
        inspect_new_fonts: false,
        missing_only: false,
        probes: config(),
        encodings: vec![Encoding::Input { ci: 27 }, Encoding::Input { ci: 28 }],
        codes: vec![32, 65, 66, 67, 200],
        seed: None,
    };
    let c = recover(&options, &mut Fake::default()).await.unwrap();
    c.validate().unwrap();
    assert_eq!(c.fonts[0].metrics, Some([7, 11, 9, 4]));
    assert_eq!(c.fonts[0].encodings.len(), 2);
    let t = Temp::new();
    compile::compile(&c, &t.0.join("rust")).unwrap();
}

#[tokio::test]
async fn unified_http_recovery_replays_all_encodings_offline() {
    let server = Server::new();
    let t = Temp::new();
    let options = RecoveryConfig {
        inspect_new_fonts: false,
        missing_only: false,
        probes: config(),
        encodings: vec![
            crate::collection::Encoding::Input { ci: 27 },
            crate::collection::Encoding::Input { ci: 28 },
        ],
        codes: vec![32, 65, 66, 67, 200],
        seed: None,
    };
    let mut printer = capture::Printer::new(&server.host, &t.0, false, 0.0, 5).unwrap();
    let online = recover(&options, &mut printer).await.unwrap();
    drop(printer);
    let mut offline = capture::Printer::new(&server.host, &t.0, true, 0.0, 5).unwrap();
    let replay = recover(&options, &mut offline).await.unwrap();
    assert_eq!(
        serde_json::to_vec(&online).unwrap(),
        serde_json::to_vec(&replay).unwrap()
    );
}

#[tokio::test]
async fn automatic_restart_does_not_retry_http_status_errors() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let host = format!("http://{}/", listener.local_addr().unwrap());
    let task = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        let mut request = [0; 8192];
        assert!(socket.read(&mut request).unwrap() > 0);
        socket
            .write_all(
                b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            )
            .unwrap();
    });
    let temp = Temp::new();
    let mut printer = capture::Printer::new(&host, &temp.0, false, 0.0, 1)
        .unwrap()
        .with_restart(1, 10)
        .unwrap()
        .with_identity(serde_json::json!({"device.unique_id":"test","appl.name":"test"}));
    let error = calibrate(&config(), &mut printer).await.unwrap_err();
    assert!(format!("{error:?}").contains("401"));
    assert!(!temp.0.join("last-reset.json").exists());
    let requests = std::fs::read_dir(&temp.0)
        .unwrap()
        .filter_map(|p| {
            let p = p.unwrap().path();
            p.join("request.zpl").exists().then_some(p)
        })
        .collect::<Vec<_>>();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].join("plan.json").exists());
    assert!(!requests[0].join("capture.json").exists());
    task.join().unwrap();
}

#[tokio::test]
async fn checkpoints_are_atomic_and_replaceable() {
    let temp = Temp::new();
    let mut printer = capture::Printer::new("http://localhost/", &temp.0, true, 0.0, 1).unwrap();
    let mut c = crate::collection::Collection::new(
        vec![crate::collection::Font {
            name: "A".into(),
            source_sha256: "a".repeat(64),
            metrics: None,
            records: vec![],
            encodings: vec![],
        }],
        serde_json::json!({}),
    )
    .unwrap();
    use capture::Capture;
    printer.checkpoint(&c).unwrap();
    c.provenance = serde_json::json!({"stage":2});
    printer.checkpoint(&c).unwrap();
    let saved = crate::collection::Collection::load(&temp.0.join("recovery.json")).unwrap();
    assert_eq!(saved.provenance["stage"], 2);
}

#[tokio::test]
async fn discovered_named_bitmaps_are_calibrated_from_previews() {
    let (face, report) = discover_native(&config(), "Z:NEW.FNT", &mut Fake::default())
        .await
        .unwrap();
    let face = face.unwrap();
    assert_eq!(face.name, "Z:NEW.FNT");
    assert_eq!(face.metrics.unwrap()[..2], [7, 11]);
    assert_eq!(report["status"], "calibrated-native-bitmap");
    assert!(!face.records.is_empty());
}

#[tokio::test]
async fn directory_names_do_not_authorize_default_font_fallback_as_data() {
    struct Fallback;
    impl Capture for Fallback {
        async fn png(&mut self, page: &Page, _repeat: bool) -> Result<Vec<u8>> {
            let mut raster = decode(&printer_png(&page.zpl()?)?)?;
            for tile in &page.tiles {
                if tile.key.starts_with("fallback-") && tile.key.ends_with("-B") {
                    let index =
                        ((tile.y + tile.oy - 7) * raster.width + tile.x + tile.ox + 1) as usize;
                    raster.pixels[index] ^= 255;
                }
            }
            Ok(Png::encode_gray(&raster, 203)?)
        }
    }
    let (face, report) = discover_native(&config(), "Z:UNSELECTABLE.FNT", &mut Fallback)
        .await
        .unwrap();
    assert!(face.is_none());
    assert_eq!(report["reason"], "filename follows the default font");
    assert!(report["lineage"].as_array().unwrap().len() >= 2);
}
