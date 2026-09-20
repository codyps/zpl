//! ZD621 preview regressions. Raw captures and provenance are in the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn spaces_and_narrow_runs_pin_every_pixel() {
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
        if c[0] != "rotated-0" {
            assert!(diff.matches(), "{} must match the printer exactly", c[0]);
        } else {
            let mut covered = vec![false; reference.pixels.len()];
            let mut regions = 0;
            for row in include_str!("fixtures/field-block-spaces-zd621-v1/rotated-0-regions.tsv")
                .lines()
                .skip(1)
            {
                let bounds: Vec<usize> = row.split('\t').map(|v| v.parse().unwrap()).collect();
                let (mut intersection, mut union) = (0, 0);
                for y in bounds[1]..bounds[3] {
                    for x in bounds[0]..bounds[2] {
                        let i = y * reference.width as usize + x;
                        covered[i] = true;
                        let r = reference.pixels[i] == 0;
                        let a = actual.pixels[i] == 0;
                        intersection += usize::from(r && a);
                        union += usize::from(r || a);
                    }
                }
                assert!(
                    union > 0 && intersection * 100 >= union * 80,
                    "region {regions}"
                );
                regions += 1;
            }
            assert_eq!(regions, 40);
            for (i, covered) in covered.into_iter().enumerate() {
                if !covered {
                    assert_eq!(reference.pixels[i], 255);
                    assert_eq!(actual.pixels[i], 255);
                }
            }
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
    assert_eq!(count, 49);
}

#[test]
fn preserving_spaces_is_an_independent_profile_option() {
    use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI};
    for (width, value) in [(100, "  AB   CD  "), (12, "    AB"), (12, "AB    CD")] {
        let source = format!("^XA^PW832^LL300^FO80,80^AAN,9,5^FB{width},6,0,L,0^FD{value}^FS^XZ");
        let image = |options| {
            let doc = zpl::render(source.as_bytes(), options).unwrap();
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
}
