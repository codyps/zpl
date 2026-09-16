//! Fixed preview-only corpus, shared by capture tooling and offline tests.
use eyre::{ensure, eyre, Result};
use raster_diff::{compare, Raster};
use serde_json::{json, Value};
use std::path::Path;
#[path = "../../examples/font_support/mod.rs"]
pub mod digest;

pub struct Case {
    pub name: &'static str,
    pub command: &'static str,
    pub data: &'static str,
}
macro_rules! cases { ($($name:ident: $command:literal, $data:literal;)*) => {
    pub const CASES:&[Case]=&[$(Case{name:stringify!($name),command:$command,data:$data},)*];
}; }
cases! {
    aztec_alias: "B0N,4", "ABC123";
    aztec: "BON,4", "ABC123";
    aztec_rune: "BON,4,N,300", "42";
    code11: "B1N,Y,80,N,N", "123-45678";
    interleaved2of5: "B2N,80,N,N,N", "12345678";
    code39: "B3N,N,80,N,N", "CODE39";
    code49: "B4N,10,N,A", "HELLO WORLD";
    planet: "B5N,80,N,N", "12345678901";
    pdf417: "B7N,6,2,4", "Hello PDF417";
    pdf417_truncated: "B7N,6,2,4,,Y", "Hello PDF417";
    ean8: "B8N,80,N,N", "1234567";
    upce: "B9N,80,N,N", "042526";
    code93: "BAN,80,N,N", "Hello93!";
    codablock_a: "BBN,10,Y,10,2,A", "ABC123";
    codablock_f: "BBN,10,Y,8,2,F", "HELLO WORLD";
    codablock_e: "BBN,10,Y,8,2,E", "HELLO WORLD";
    code128: "BCN,80,N,N,N,N", "Code128";
    maxicode2: "BD2", "001840123450000Hello MaxiCode";
    maxicode3: "BD3", "001826ABC123Hello MaxiCode";
    maxicode4: "BD4", "Hello MaxiCode";
    maxicode5: "BD5", "Hello MaxiCode";
    maxicode6: "BD6", "Hello MaxiCode";
    ean13: "BEN,80,N,N", "590123412345";
    micropdf417_1: "BFN,4,0", "A";
    micropdf417_3: "BFN,4,13", "A";
    micropdf417_4: "BFN,4,23", "A";
    industrial2of5: "BIN,80,N,N", "12345678";
    standard2of5: "BJN,80,N,N", "12345678";
    codabar: "BKN,N,80,N,N,A,B", "123456";
    logmars: "BLN,80,N", "LOGMARS";
    msi_a: "BMN,A,80,N,N", "1234567";
    msi_b: "BMN,B,80,N,N", "1234567";
    msi_c: "BMN,C,80,N,N", "1234567";
    msi_d: "BMN,D,80,N,N", "1234567";
    plessey: "BPN,N,80,N,N", "123ABC";
    qr: "BQN,2,4,L,0", "LA,Hello QR 123";
    databar_omni: "BRN,1,2", "2001234567890";
    databar_truncated: "BRN,2,2", "2001234567890";
    databar_stacked: "BRN,3,2", "2001234567890";
    databar_stacked_omni: "BRN,4,2", "2001234567890";
    databar_limited: "BRN,5,2", "1001234567890";
    databar_expanded: "BRN,6,2,1,80,22", "0100012345678905";
    databar_expanded_stacked: "BRN,6,2,1,80,4", "01950123456789033103000123";
    databar_upca: "BRN,7,2,1,80", "03600029145";
    databar_upce: "BRN,8,2,1,80", "042526";
    databar_ean13: "BRN,9,2,1,80", "590123412345";
    databar_ean8: "BRN,10,2,1,80", "1234567";
    composite_a: "BRN,11,2,1,80", "0103212345678906|10ABC";
    composite_b: "BRN,11,2,1,80", "0103212345678906|91AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    composite_c: "BRN,12,2,1,80", "0103212345678906|10ABC";
    extension2: "BSN,80,N,N", "12";
    extension5: "BSN,80,N,N", "51234";
    tlc39_linear: "BTN,2,2,80,2,4", "123456";
    tlc39_linked: "BTN,2,2,80,2,4", "239316,12345678901234";
    upca: "BUN,80,N,N", "03600029145";
    data_matrix: "BXN,4,200", "Hello Data Matrix";
    data_matrix_rectangular: "BXN,4,200,32,8,6,_,2", "AB12";
    postnet: "BZN,80,N,N,0", "123456789";
    postal_planet: "BZN,80,N,N,1", "12345678901";
    intelligent_mail: "BZN,80,N,N,3", "0027012345620080000198765432101";
}
pub fn request(case: &Case) -> String {
    format!(
        "^XA^PW812^LL1218^LH0,0^LS0^LT0^PON^LRN^CI27^FWN^CF0,20,0^FO60,60^BY2,2,80^{}^FD{}^FS^XZ\n",
        case.command, case.data
    )
}
pub fn local(zpl: &str) -> std::result::Result<Raster, String> {
    let doc = zpl::render(zpl.as_bytes(), zpl::Options::default()).map_err(|e| e.to_string())?;
    zpl::output::raster::rasterize(&doc.labels[0]).map_err(|e| e.to_string())
}
fn raster_info(r: &Raster) -> Value {
    json!({"width":r.width,"height":r.height,"ink":r.pixels.iter().filter(|&&p|p==0).count(),"pixels_sha256":digest::sha256(&r.pixels)})
}
pub fn observation(zpl: &str, png: &[u8]) -> Result<Value> {
    let reference = Raster::decode_png(png).map_err(|e| eyre!(e))?;
    let mut record = json!({"zpl_sha256":digest::sha256(zpl.as_bytes()),"printer_png_sha256":digest::sha256(png),"printer":raster_info(&reference)});
    match local(zpl) {
        Ok(actual) => {
            let diff = compare(&reference, &actual, true).map_err(|e| eyre!(e))?;
            record["local"] = raster_info(&actual);
            record["comparison"] = json!({"exact":diff.matches(),"dimensions_match":diff.dimensions_match,"printer_only":diff.reference_only,"local_only":diff.candidate_only,"both_black":diff.both_black,"diff_rgb_sha256":digest::sha256(&diff.pixels)});
        }
        Err(error) => record["local_error"] = json!(error),
    }
    Ok(record)
}
pub fn verify(root: &Path, case: &Case) -> Result<Value> {
    let manifest: Value = serde_json::from_slice(&std::fs::read(root.join("manifest.json"))?)?;
    ensure!(
        manifest["schema"] == "zpl-printer-barcode-comparison-v1",
        "wrong manifest schema"
    );
    let expected = &manifest["cases"][case.name];
    ensure!(!expected.is_null(), "missing case {}", case.name);
    let zpl = std::fs::read_to_string(root.join(format!("{}.zpl", case.name)))?;
    ensure!(
        zpl == request(case),
        "{}: corpus changed; capture separately and review",
        case.name
    );
    let png = std::fs::read(root.join(format!("{}.png", case.name)))?;
    let current = observation(&zpl, &png)?;
    ensure!(&current==expected,"{}: printer/local comparison changed (including possible improvement). Review the diff, do not silently refresh.\nexpected: {}\nactual: {}",case.name,expected,current);
    Ok(current)
}
