use super::*;
use crate::automatic::{capture::Capture, probe::Page};
use serde_json::json;
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
            "collection-test-{}-{}",
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
fn record(id: u16) -> Record {
    Record {
        id,
        advance: 5,
        left: -1,
        top: -2,
        width: 3,
        height: 2,
        flags: 0,
        bitmap_hex: "a040".into(),
    }
}
fn font() -> Font {
    Font {
        name: "Z:TEST.FNT".into(),
        source_sha256: "a".repeat(64),
        header_hex: "00".repeat(116),
        slot_count: 302,
        absent_slots: (0..302)
            .filter(|id| ![17, 18, 300, 301].contains(id))
            .collect(),
        zero_record_slots: vec![301],
        records: vec![
            record(17),
            record(18),
            Record {
                bitmap_hex: "e080".into(),
                ..record(300)
            },
        ],
        encodings: vec![],
    }
}
#[test]
fn complete_inventory_and_hash_validation() {
    let mut c = Collection::new(vec![font()], json!({})).unwrap();
    c.fonts[0].absent_slots.pop();
    c.seal().unwrap();
    assert!(c.validate().is_err());
    let mut c = Collection::new(vec![font()], json!({})).unwrap();
    c.fonts[0].records[0].advance += 1;
    assert!(c.validate().is_err());
}
#[test]
fn raw_fnt_preserves_ids_bearings_padding_and_absent_slots() {
    let mut data = vec![0; 116 + 302 * 20 + 2];
    data[4..8].copy_from_slice(&[1, 1, 0, 3]);
    data[76..78].copy_from_slice(&116u16.to_be_bytes());
    data[92..96].copy_from_slice(&116u32.to_be_bytes());
    data[96..100].copy_from_slice(&(116u32 + 302 * 20).to_be_bytes());
    data[100..102].copy_from_slice(&302u16.to_be_bytes());
    data[116..116 + 20].fill(255);
    let p = 116 + 300 * 20;
    for (off, value) in [
        (0, 300u16),
        (2, 2),
        (4, 3),
        (6, 2),
        (8, 65535),
        (10, 2),
        (12, 5),
        (14, 7),
    ] {
        data[p + off..p + off + 2].copy_from_slice(&value.to_be_bytes());
    }
    let end = data.len();
    data[end - 2..].copy_from_slice(&[0xa1, 0x40]);
    let f = fnt::decode("Z:T.FNT", &data).unwrap();
    assert_eq!(f.records.len(), 1);
    let r = &f.records[0];
    assert_eq!((r.id, r.left, r.top, r.flags), (300, -1, -2, 7));
    assert_eq!(r.glyph().unwrap().width, 8);
    assert_eq!(r.bitmap_hex, "a140");
    assert_eq!(f.absent_slots, vec![0]);
    assert_eq!(f.zero_record_slots.len(), 300);
    assert!(fnt::decode("Z:T.FNT", &data[..end - 1]).is_err());
    data[p..p + 2].copy_from_slice(&299u16.to_be_bytes());
    assert!(fnt::decode("Z:T.FNT", &data).is_err());
}
struct Printer {
    unstable: bool,
}
impl Capture for Printer {
    async fn png(&mut self, page: &Page, repeat: bool) -> eyre::Result<Vec<u8>> {
        let mut raster = page.background()?;
        for tile in &page.tiles {
            let bytes = tile.field_bytes.as_ref().unwrap();
            let text = std::str::from_utf8(bytes)?;
            let mut pen = 0;
            for ch in text.chars() {
                // Independent response model with a non-ASCII glyph and duplicates.
                let rows = if ch == 'Λ' {
                    [0xe0, 0x80]
                } else if ch == ' ' {
                    [0, 0]
                } else {
                    [0xa0, 0x40]
                };
                for (y, row) in rows.iter().enumerate() {
                    for x in 0..3 {
                        if row & (128 >> x) != 0 {
                            let px = tile.x + tile.ox + pen + x as u32 - 1;
                            let py = tile.y + tile.oy + y as u32 - 2;
                            raster.pixels[(py * raster.width + px) as usize] = 0;
                        }
                    }
                }
                pen += 5;
            }
        }
        if repeat && self.unstable {
            raster.pixels[0] = 0;
        }
        Ok(raster_diff::Png::encode_gray(&raster, 203)?)
    }
}
#[tokio::test]
async fn unicode_mapping_ambiguity_blank_and_repeat() {
    let f = font();
    let encoding = Encoding::Input { ci: 28 };
    let pages = survey::pages(&f, encoding, &[32, 0x39b], 832, 1024).unwrap();
    assert!(pages.iter().any(|p| p.zpl().unwrap().contains("_CE_9B")));
    let map = survey::run(
        &f,
        encoding,
        &[32, 0x39b],
        832,
        1024,
        &mut Printer { unstable: false },
    )
    .await
    .unwrap();
    assert_eq!(
        map.entries
            .iter()
            .find(|e| e.input == 0x39b)
            .unwrap()
            .candidates,
        vec![300]
    );
    assert_eq!(
        map.entries
            .iter()
            .find(|e| e.input == 65)
            .unwrap()
            .candidates,
        vec![17, 18]
    );
    assert_eq!(
        map.entries.iter().find(|e| e.input == 32).unwrap().status,
        Status::BlankUnresolved
    );
    assert!(survey::run(
        &f,
        encoding,
        &[65],
        832,
        1024,
        &mut Printer { unstable: true }
    )
    .await
    .is_err());
    assert!(survey::pages(&f, Encoding::Input { ci: 29 }, &[65], 832, 1024).is_err());
}
#[test]
fn directory_validation_and_unknown_font_types() {
    assert_eq!(
        transport::directory(
            b"\"\r\n- DIR Z:*.*\r\n* Z:A.FNT 123 P A\r\n* Z:0.TTF 999 P 0\r\n\"",
            'Z'
        )
        .unwrap(),
        vec![("Z:A.FNT".into(), 123)]
    );
    assert!(transport::directory(b"* E:A.FNT 123", 'Z').is_err());
    assert!(transport::directory(b"* Z:A.FNT 123\n* Z:A.FNT 123", 'Z').is_err());
}
#[test]
fn conflicting_maps_are_rejected() {
    let mut f = font();
    let m = EncodingMap {
        encoding: Encoding::Input { ci: 28 },
        entries: vec![Entry {
            input: 65,
            status: Status::Matched,
            candidates: vec![17, 18],
        }],
        provenance: json!({}),
    };
    evidence::merge(&mut f, m.clone()).unwrap();
    let mut bad = m;
    bad.entries[0].candidates = vec![300];
    assert!(evidence::merge(&mut f, bad).is_err());
}
#[test]
fn generated_tables_compile_and_preserve_raw_ids_and_evidence() {
    let tmp = Temp::new();
    let mut f = font();
    f.encodings.push(EncodingMap {
        encoding: Encoding::Input { ci: 28 },
        entries: vec![
            Entry {
                input: 65,
                status: Status::Matched,
                candidates: vec![17, 18],
            },
            Entry {
                input: 0x39b,
                status: Status::Matched,
                candidates: vec![300],
            },
        ],
        provenance: json!({}),
    });
    f.encodings.push(EncodingMap {
        encoding: Encoding::CandidateCharacters,
        entries: vec![Entry {
            input: 66,
            status: Status::UnverifiedCandidate,
            candidates: vec![17],
        }],
        provenance: json!({"source":"test"}),
    });
    f.slot_count = 303;
    f.records.push(Record {
        id: 302,
        advance: 500,
        left: -300,
        top: -250,
        width: 257,
        height: 1,
        flags: 7,
        bitmap_hex: "ff".repeat(33),
    });
    let mut c = Collection::new(vec![f], json!({})).unwrap();
    let mut bytes = vec![0u16; 256];
    bytes[128] = 66;
    c.candidate_code_pages.push(CodePage {
        ci: 27,
        byte_to_character: bytes,
        provenance: json!({"source":"test"}),
    });
    c.seal().unwrap();
    let out = tmp.0.join("compiled");
    compile::compile(&c, &out).unwrap();
    assert!(compile::compile(&c, &out).is_err());
    let source = format!(
        r#"extern crate self as zpl_bitmap_fonts;
#[path={:?}] pub mod collection;
#[path={:?}] mod fonts;
fn main(){{let f=fonts::font_by_name("Z:TEST.FNT").unwrap();let wide=f.record(302).unwrap();assert_eq!((wide.width,wide.advance,wide.left,wide.top,wide.flags),(257,500,-300,-250,7));assert!(wide.pixel(256,0));assert!(!wide.pixel(263,0));assert!(wide.storage_pixel(263,0));assert_eq!(fonts::CANDIDATE_CODE_PAGES[0].candidate(f,128).unwrap().candidates,&[17]);assert!(f.record(301).is_none());let r=f.record(300).unwrap();assert_eq!(r.bitmap(),&[0xe0,0x80]);assert_eq!((r.left,r.top),(-1,-2));assert!(r.pixel(2,0));let m=f.encoding(collection::Encoding::Input{{ci:28}}).unwrap();assert_eq!(m.lookup(65).unwrap().candidates,&[17,18]);assert_eq!(m.lookup(0x39b).unwrap().candidates,&[300]);assert!(m.lookup(0x3a9).is_none());assert_eq!(f.encoding(collection::Encoding::CandidateCharacters).unwrap().lookup(66).unwrap().status,collection::Status::UnverifiedCandidate);}}
"#,
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../zpl-bitmap-fonts/src/collection.rs"),
        out.join("fonts.rs")
    );
    fs::write(tmp.0.join("check.rs"), source).unwrap();
    assert!(std::process::Command::new("rustc")
        .arg("--edition=2021")
        .arg(tmp.0.join("check.rs"))
        .arg("-o")
        .arg(tmp.0.join("check"))
        .status()
        .unwrap()
        .success());
    assert!(std::process::Command::new(tmp.0.join("check"))
        .status()
        .unwrap()
        .success());
}

#[test]
fn sgd_download_enforces_exact_sizes_and_read_only_commands() {
    use std::{
        io::{Read, Write},
        net::TcpListener,
    };
    fn server(payload: Vec<u8>) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut command = vec![];
            let mut byte = [0];
            while !command.ends_with(b"\r\n") {
                stream.read_exact(&mut byte).unwrap();
                command.push(byte[0]);
            }
            assert_eq!(command, b"! U1 do \"file.type\" \"Z:A.FNT.\"\r\n");
            stream.write_all(&payload).unwrap();
        });
        (addr, worker)
    }
    let (addr, thread) = server(vec![1, 2, 3]);
    assert_eq!(
        transport::exchange(&addr, "! U1 do \"file.type\" \"Z:A.FNT.\"\r\n", Some(3)).unwrap(),
        [1, 2, 3]
    );
    thread.join().unwrap();
    let (addr, thread) = server(vec![1, 2]);
    assert!(transport::exchange(&addr, "! U1 do \"file.type\" \"Z:A.FNT.\"\r\n", Some(3)).is_err());
    thread.join().unwrap();
    let (addr, thread) = server(vec![1, 2, 3, 4]);
    assert!(transport::exchange(&addr, "! U1 do \"file.type\" \"Z:A.FNT.\"\r\n", Some(3)).is_err());
    thread.join().unwrap();
}
#[test]
fn unverified_candidates_cannot_become_measured_or_lose_codepages() {
    let mut c = Collection::new(vec![font()], json!({})).unwrap();
    let report = json!({"schema":"zebra-font-mapping-candidates-v1","code_pages":[{"id":27,"byte_to_character":(0..256).collect::<Vec<_>>(),"provenance":{"label":"test page"}}],"font_candidates":{"Z:TEST.FNT":{"reference_sha256":"a".repeat(64),"provenance":{"label":"test font"},"entries":[{"raw_record_id":300,"character_codes":[923]}]}}});
    evidence::import(&mut c, &report, Path::new(".")).unwrap();
    assert_eq!(c.coverage[0].observed_equivalent_records, 0);
    assert_eq!(c.coverage[0].candidate_records, 1);
    assert_eq!(c.candidate_code_pages[0].byte_to_character.len(), 256);
    assert_eq!(
        c.candidate_code_pages[0].provenance["provenance"]["label"],
        "test page"
    );
    assert_eq!(
        c.fonts[0].encodings[0].provenance["provenance"]["label"],
        "test font"
    );
    c.fonts[0].encodings[0].entries[0].status = Status::Matched;
    c.seal().unwrap();
    assert!(c.validate().is_err());
}
#[test]
fn truncated_directory_is_not_a_complete_inventory() {
    assert!(transport::directory(b"\"\r\n- DIR Z:*.*\r\n* Z:A.FNT 123", 'Z').is_err());
}

#[test]
fn collection_io_has_its_own_bounded_budget() {
    let t = Temp::new();
    let path = t.0.join("fonts.json");
    let c = Collection::new(vec![font()], json!({})).unwrap();
    c.save(&path).unwrap();
    assert_eq!(
        Collection::load(&path).unwrap().content_sha256,
        c.content_sha256
    );
    assert!(c.save(&path).is_err());
    let oversized = t.0.join("oversized.json");
    fs::File::create(&oversized)
        .unwrap()
        .set_len(MAX_JSON_BYTES + 1)
        .unwrap();
    assert!(Collection::load(&oversized)
        .unwrap_err()
        .to_string()
        .contains("256 MiB"));
}
