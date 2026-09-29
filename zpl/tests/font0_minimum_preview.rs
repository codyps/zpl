//! ZD621 preview regressions. Raw captures and provenance are in the fixture README.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
#[test]
fn minimum_font_strikes_and_origins_pin_every_pixel() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/font0-minimum-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/font0-minimum-zd621-v1/manifest.tsv")
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
        let diff = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        if !c[0].starts_with("holdout-") {
            assert!(diff.matches(), "{} must match the printer exactly", c[0]);
        } else {
            let mut covered = vec![false; reference.pixels.len()];
            let mut regions = 0;
            let regions_file =
                fs::read_to_string(root.join(format!("{}-regions.tsv", c[0]))).unwrap();
            for row in regions_file.lines().skip(1) {
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
            assert_eq!(regions, 32);
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
    assert_eq!(count, 46);
}

fn measurement(name: &str) -> raster_diff::Raster {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/font0-minimum-zd621-v1");
    let row = include_str!("fixtures/font0-minimum-zd621-v1/measurements.tsv")
        .lines()
        .skip(1)
        .find(|row| row.split('\t').next() == Some(name))
        .unwrap();
    let c: Vec<_> = row.split('\t').collect();
    let source = fs::read(root.join(format!("{name}.zpl"))).unwrap();
    let png = fs::read(root.join(format!("{name}.png"))).unwrap();
    assert_eq!(digest::sha256(&source), c[1]);
    assert_eq!(digest::sha256(&png), c[2]);
    raster_diff::Raster::decode_png(&png).unwrap()
}
#[test]
fn native_size_boundary_controls_locate_the_ten_dot_minimum() {
    for (name, columns) in [("boundary-height", 3), ("boundary-width", 1)] {
        let reference = measurement(name);
        let region = |row: usize, column: usize| {
            let mut pixels = Vec::new();
            for y in 40 + row * 140..180 + row * 140 {
                let start = y * reference.width as usize + column * 250;
                pixels.extend_from_slice(&reference.pixels[start..start + 250]);
            }
            pixels
        };
        for column in 0..columns {
            let minimum = region(4, column); // requested size 10
            assert!(minimum.contains(&0));
            for row in 0..4 {
                // requested sizes 1, 2, 7, 9
                assert_eq!(region(row, column), minimum);
            }
            for row in 5..8 {
                // requested sizes 11, 12, 15
                assert_ne!(region(row, column), minimum);
            }
        }
    }
}
#[test]
fn native_fo_ft_pairs_measure_flooring_at_every_quarter_dot_phase() {
    for first in [10, 18] {
        let reference = measurement(&format!("baseline-{first}"));
        let ink_origin = |x: usize, y: usize| {
            let (mut left, mut top) = (usize::MAX, usize::MAX);
            for yy in y - 40..y + 55 {
                for xx in x - 25..x + 50 {
                    if reference.pixels[yy * reference.width as usize + xx] == 0 {
                        left = left.min(xx);
                        top = top.min(yy);
                    }
                }
            }
            assert_ne!(left, usize::MAX);
            (left as i64 - x as i64, top as i64 - y as i64)
        };
        for i in 0..8 {
            let h = first + i;
            let baseline = (h * 3 / 4) as i64;
            for orientation in 0..4 {
                let fo = ink_origin(30 + orientation * 200, 70 + i * 140);
                let ft = ink_origin(120 + orientation * 200, 70 + i * 140);
                let delta = (fo.0 - ft.0, fo.1 - ft.1);
                match orientation {
                    0 => assert_eq!(delta, (0, baseline)),
                    1 => assert_eq!(delta, (h as i64 - 1 - baseline, 0)),
                    2 => assert_eq!(delta.1, h as i64 - 1 - baseline),
                    _ => assert_eq!(delta.0, baseline),
                }
            }
        }
    }
}
