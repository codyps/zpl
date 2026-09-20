//! ZD621 preview regressions. Raw captures and provenance are in the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn spaces_pin_every_painted_pixel_and_known_narrow_differences() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/field-block-spaces-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/field-block-spaces-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let input = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&input), c[1], "{} input", c[0]);
        assert_eq!(digest::sha256(&png), c[2], "{} printer capture", c[0]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&input, zpl::render::profiles::ZD621_203_DPI).unwrap();
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare(&reference, &actual, false).unwrap();
        if c[6] == "exact" {
            assert!(diff.matches(), "{} must match the printer exactly", c[0]);
        }
        assert_eq!(
            diff.reference_only,
            c[3].parse::<usize>().unwrap(),
            "{} underpaint",
            c[0]
        );
        assert_eq!(
            diff.candidate_only,
            c[4].parse::<usize>().unwrap(),
            "{} overpaint",
            c[0]
        );
        assert_eq!(
            digest::sha256(&actual.pixels),
            c[5],
            "{} local pixels",
            c[0]
        );
        assert!(
            diff.both_black > 0,
            "{}: positive control must contain ink",
            c[0]
        );
        count += 1;
    }
    assert_eq!(count, 36);
}

#[test]
fn preserving_spaces_is_an_independent_profile_option() {
    use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
    let source = b"^XA^PW832^LL300^FO80,80^AAN,9,5^FB100,3,0,L,0^FD  AB   CD  ^FS^XZ";
    let image = |options| {
        let doc = zpl::render(source, options).unwrap();
        zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
    };
    let native = image(ZD621_203_DPI);
    let normalized = image(SPECIFICATION);
    assert_ne!(native, normalized);
    let mut specification = SPECIFICATION;
    specification.compatibility.block_preserves_extra_spaces = true;
    assert_eq!(image(specification), native);
    let mut printer = ZD621_203_DPI;
    printer.compatibility.block_preserves_extra_spaces = false;
    assert_eq!(image(printer), normalized);
}
