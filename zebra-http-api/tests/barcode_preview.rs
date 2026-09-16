//! Offline regression comparisons against captured real-printer previews.
mod barcode_support;
fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/barcodes-zd621-v1")
}

macro_rules! check {($($name:ident),* $(,)?)=>{$(
    #[test] fn $name(){
        let case=barcode_support::CASES.iter().find(|c|c.name==stringify!($name)).unwrap();
        barcode_support::verify(&root(),case).unwrap();
    }
)*};}
check!(
    aztec_alias,
    aztec,
    aztec_rune,
    code11,
    interleaved2of5,
    code39,
    code49,
    planet,
    pdf417,
    pdf417_truncated,
    ean8,
    upce,
    code93,
    codablock_a,
    codablock_f,
    codablock_e,
    code128,
    maxicode2,
    maxicode3,
    maxicode4,
    maxicode5,
    maxicode6,
    ean13,
    micropdf417_1,
    micropdf417_3,
    micropdf417_4,
    industrial2of5,
    standard2of5,
    codabar,
    logmars,
    msi_a,
    msi_b,
    msi_c,
    msi_d,
    plessey,
    qr,
    databar_omni,
    databar_truncated,
    databar_stacked,
    databar_stacked_omni,
    databar_limited,
    databar_expanded,
    databar_expanded_stacked,
    databar_upca,
    databar_upce,
    databar_ean13,
    databar_ean8,
    composite_a,
    composite_b,
    composite_c,
    extension2,
    extension5,
    tlc39_linear,
    tlc39_linked,
    upca,
    data_matrix,
    data_matrix_rectangular,
    postnet,
    postal_planet,
    intelligent_mail
);

#[test]
fn corpus_covers_every_barcode_command() {
    let commands: std::collections::BTreeSet<_> = barcode_support::CASES
        .iter()
        .map(|c| &c.command[..2])
        .collect();
    for name in [
        "B0", "B1", "B2", "B3", "B4", "B5", "B7", "B8", "B9", "BA", "BB", "BC", "BD", "BE", "BF",
        "BI", "BJ", "BK", "BL", "BM", "BO", "BP", "BQ", "BR", "BS", "BT", "BU", "BX", "BZ",
    ] {
        assert!(commands.contains(name), "missing {name}");
    }
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root().join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        manifest["cases"].as_object().unwrap().len(),
        barcode_support::CASES.len()
    );
}

#[test]
fn comparison_detects_pixel_and_canvas_changes() {
    use raster_diff::{Png, Raster};
    let zpl = "^XA^PW8^LL8^FO1,1^GB2,2,2^FS^XZ";
    let raster = barcode_support::local(zpl).unwrap();
    let observe =
        |r: &Raster| barcode_support::observation(zpl, &Png::encode_gray(r, 203).unwrap()).unwrap();
    let exact = observe(&raster);
    assert_eq!(exact["comparison"]["exact"], true);
    let mut changed = raster.clone();
    changed.pixels[0] ^= 255;
    let changed = observe(&changed);
    assert_eq!(changed["comparison"]["exact"], false);
    assert_ne!(exact["printer_png_sha256"], changed["printer_png_sha256"]);
    assert_ne!(
        exact["comparison"]["diff_rgb_sha256"],
        changed["comparison"]["diff_rgb_sha256"]
    );
    let mut taller = raster;
    taller.height += 1;
    taller.pixels.extend(vec![255; taller.width as usize]);
    let taller = observe(&taller);
    assert_eq!(taller["comparison"]["exact"], false);
    assert_eq!(taller["comparison"]["dimensions_match"], false);
    assert_eq!(taller["comparison"]["printer_only"], 0);
    assert_eq!(taller["comparison"]["local_only"], 0);
}

#[test]
#[ignore = "strict parity gate: known printer/local differences are not equivalence"]
fn all_formats_match_printer_pixels() {
    let mut failures = Vec::new();
    for case in barcode_support::CASES {
        let record = barcode_support::verify(&root(), case).unwrap();
        if record["comparison"]["exact"] != true || record["printer"]["ink"] == 0 {
            failures.push(format!("{}: {}", case.name, record));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
