//! Unmodified ZD621 character remapping controls; see the fixture README.
use std::{fs, path::Path};
#[path = "support/digest.rs"]
mod digest;
#[test]
fn raw_remap_frames_are_pixel_exact() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/character-remap-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/character-remap-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let source = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&source), c[1]);
        assert_eq!(digest::sha256(&png), c[2]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&source, zpl::render::profiles::ZD621_203_DPI).unwrap();
        assert_eq!(doc.labels.len(), 1);
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (c[3].parse().unwrap(), c[4].parse().unwrap()),
            "{}",
            c[0]
        );
        assert_eq!(&c[3..5], &["0", "0"]);
        assert_eq!(digest::sha256(&actual.pixels), c[5], "{}", c[0]);
        assert!(reference.pixels.contains(&0));
        count += 1;
    }
    assert_eq!(count, 4);
}

fn pixels(field: &str, options: zpl::render::Options) -> Vec<u8> {
    let source = format!("^XA^PW832^LL200^PA0,0,0,0^FO40,40^A0N,40,24{field}^FS^XZ");
    let doc = zpl::render(source.as_bytes(), options).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0])
        .unwrap()
        .pixels
}

#[test]
fn space_remapping_is_an_independent_printer_departure() {
    use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
    let mut selected = SPECIFICATION;
    selected.compatibility.remap_space = true;
    let input = "^CI0,65,32^FDW W";
    assert!(pixels(input, SPECIFICATION) == pixels("^CI0^FDW W", SPECIFICATION));
    assert!(pixels(input, selected) == pixels("^CI0^FDWAW", selected));
    assert!(pixels(input, selected) == pixels(input, ZD621_203_DPI));
}

#[test]
fn mappings_are_per_encoding_persistent_and_nonrecursive() {
    use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
    for options in [SPECIFICATION, ZD621_203_DPI] {
        for (input, expected) in [
            ("^CI0,65,66,66,67^FDABBC", "^CI0^FDAAAB"),
            ("^CI0,65,66^CI13^CI0^FDABBC", "^CI0^FDAAAC"),
            ("^CI0,65,66^CI13^FDABBC", "^CI13^FDABBC"),
            ("^CI0,65,66,67,66^FDABBC", "^CI0^FDACCC"),
            ("^CI28,65,66^FDABBC", "^CI28^FDABBC"),
            ("^CI27,65,66^FDABBC", "^CI27^FDABBC"),
            ("^CI0,21,36^FD$", "^CI28^FD€"),
        ] {
            assert!(
                pixels(input, options) == pixels(expected, options),
                "{input}"
            );
        }
        assert!(
            pixels("^CI0,65,66^BQN,2,3^FDLA,ABBC", options)
                == pixels("^CI0^BQN,2,3^FDLA,ABBC", options)
        );
    }
}

#[test]
fn invalid_remapping_operands_fail_before_glyph_lookup() {
    for command in [
        "^CI0,65",
        "^CI0,256,65",
        "^CI0,65,256",
        "^CI0,-1,65",
        "^CI0,,65",
    ] {
        let source = format!("^XA{command}^FO20,20^FDABC^FS^XZ");
        assert!(zpl::render(source.as_bytes(), zpl::render::profiles::SPECIFICATION).is_err());
    }
}
