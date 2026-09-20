//! Raw native controls and provenance: fixtures/font0-common-zd621-v1/README.md.
use std::{fs, path::Path};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;
use zpl::render::profiles::ZD621_203_DPI;
#[test]
fn common_text_sizes_and_layout_pin_every_pixel() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/font0-common-zd621-v1");
    let mut count = 0;
    for row in include_str!("fixtures/font0-common-zd621-v1/manifest.tsv")
        .lines()
        .skip(1)
    {
        let c: Vec<_> = row.split('\t').collect();
        let source = fs::read(root.join(format!("{}.zpl", c[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", c[0]))).unwrap();
        assert_eq!(digest::sha256(&source), c[1]);
        assert_eq!(digest::sha256(&png), c[2]);
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let doc = zpl::render(&source, ZD621_203_DPI).unwrap();
        assert_eq!(doc.labels.len(), 1);
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare(&reference, &actual, false).unwrap();
        assert_eq!(
            (diff.reference_only, diff.candidate_only),
            (c[3].parse().unwrap(), c[4].parse().unwrap()),
            "{}",
            c[0]
        );
        assert!(reference.pixels.contains(&0));
        assert_eq!(digest::sha256(&actual.pixels), c[5]);

        count += 1;
    }
    assert_eq!(count, 77);
}

#[test]
fn geometry_is_exact_and_each_rotated_text_region_exceeds_the_floor() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/font0-common-zd621-v1");
    let source = fs::read(root.join("torture-geometry.zpl")).unwrap();
    let reference =
        raster_diff::Raster::decode_png(&fs::read(root.join("torture-geometry.png")).unwrap())
            .unwrap();
    let geometry =
        raster_diff::Raster::decode_png(&fs::read(root.join("geometry-only.png")).unwrap())
            .unwrap();
    let doc = zpl::render(&source, ZD621_203_DPI).unwrap();
    let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
    let mut text_region = vec![false; reference.pixels.len()];
    let mut geometry_source = String::from_utf8(source).unwrap();
    for n in 0..36 {
        let x = 85 + n % 6 * 132;
        let y = 100 + n / 6 * 185;
        let field = format!(
            "^FO{x},{y}^A0{},20,10^FD{n:02}^FS",
            ['N', 'R', 'I', 'B'][n % 4]
        );
        assert!(geometry_source.contains(&field));
        geometry_source = geometry_source.replace(&field, "");
        let (mut intersection, mut union) = (0usize, 0usize);
        for yy in y - 2..y + 25 {
            for xx in x - 2..x + 25 {
                let i = yy * 832 + xx;
                text_region[i] = true;
                if geometry.pixels[i] == 0 {
                    continue;
                }
                let a = reference.pixels[i] == 0;
                let b = actual.pixels[i] == 0;
                intersection += usize::from(a && b);
                union += usize::from(a || b);
            }
        }
        assert!(
            union > 0 && intersection * 100 >= union * 80,
            "text region {n}"
        );
    }
    assert_eq!(
        geometry_source.as_bytes(),
        fs::read(root.join("geometry-only.zpl")).unwrap()
    );
    for (i, in_text) in text_region.into_iter().enumerate() {
        if !in_text {
            assert_eq!(reference.pixels[i], geometry.pixels[i]);
            assert_eq!(actual.pixels[i], geometry.pixels[i]);
        }
        if geometry.pixels[i] == 0 {
            assert_eq!(reference.pixels[i], 0);
            assert_eq!(actual.pixels[i], 0);
        }
    }
}
