//! Native evidence and source references in fixtures/retail-font-zd621-v1/README.md.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;

fn pixels(body: &[u8], options: zpl::Options) -> raster_diff::Raster {
    let mut source = b"^XA^PW832^LL400^FO40,40^A0N,32,0".to_vec();
    source.extend_from_slice(body);
    source.extend_from_slice(b"^FS^XZ");
    let doc = zpl::render(&source, options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
}

#[test]
fn legacy_decoding_precedes_block_layout() {
    // ^TB ignores soft hyphens (Programming Guide p. 356); CP850 encodes
    // U+00AD at F0, not AD. ^FB's generated hyphens are already Unicode.
    // https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
    // https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/PC/CP850.TXT
    for options in [
        zpl::render::profiles::SPECIFICATION,
        zpl::render::profiles::ZD621_203_DPI,
    ] {
        assert_eq!(
            pixels(b"^CI13^TBN,200,100^FH^FDAB_F0CD", options),
            pixels(b"^CI13^TBN,200,100^FDABCD", options)
        );
        assert_eq!(
            pixels(b"^CI13^FB80,4,0,L^FDABCDEFGHIJKL", options),
            pixels(b"^CI28^FB80,4,0,L^FDABCDEFGHIJKL", options)
        );
    }
}

#[test]
fn legacy_remapping_uses_original_image_indices() {
    // ^CI remaps source image to destination byte (Programming Guide p. 158).
    // CP850 B6 is U+00C2, retained in the native retail supplements.
    for options in [
        zpl::render::profiles::SPECIFICATION,
        zpl::render::profiles::ZD621_203_DPI,
    ] {
        assert_eq!(
            pixels(b"^CI13,182,65^FDA", options),
            pixels("^CI28^FDÂ".as_bytes(), options)
        );
        assert_eq!(
            pixels(b"^CI13,65,182^FH^FD_B6", options),
            pixels(b"^CI28^FDA", options)
        );
    }
}

#[test]
fn native_retail_encoding_controls() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/retail-font-zd621-v1/controls");
    let provenance: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("provenance.json")).unwrap()).unwrap();
    for name in ["legacy", "cp1252", "utf8", "rotations", "utf8-repeat"] {
        let source = fs::read(root.join(format!("{name}.zpl"))).unwrap();
        let png = fs::read(root.join(format!("{name}.png"))).unwrap();
        let row = provenance["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["name"] == name)
            .unwrap();
        assert_eq!(digest::sha256(&source), row["zpl_sha256"].as_str().unwrap());
        assert_eq!(digest::sha256(&png), row["png_sha256"].as_str().unwrap());
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&source, zpl::render::profiles::ZD621_203_DPI).unwrap();
        assert_eq!(doc.labels.len(), 1);
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert_eq!((diff.reference_only, diff.candidate_only), (0, 0), "{name}");
    }
}
