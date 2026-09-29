//! Unmodified ZQ610 Plus 203-DPI V100.21.21Z HTTP preview captures.
//! See fixtures/zq610-plus-v1/README.md for acquisition, scope and limitations.
use std::{fs, path::Path};
use zpl::render::profiles::{SPECIFICATION, ZD621_203_DPI, ZQ610_PLUS_203_DPI};
#[path = "../../zebra-http-api/examples/font_support/mod.rs"]
mod digest;

#[test]
fn zq610_native_captures_pin_full_canvas_pixels_at_original_origin() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/zq610-plus-v1");
    let manifest = fs::read_to_string(root.join("manifest.tsv")).unwrap();
    let mut count = 0;
    for row in manifest.lines().skip(1) {
        let columns: Vec<_> = row.split('\t').collect();
        let source = fs::read(root.join(format!("{}.zpl", columns[0]))).unwrap();
        let png = fs::read(root.join(format!("{}.png", columns[0]))).unwrap();
        assert_eq!(digest::sha256(&source), columns[1], "{} source", columns[0]);
        assert_eq!(
            digest::sha256(&png),
            columns[2],
            "{} native PNG",
            columns[0]
        );
        let reference = raster_diff::Raster::decode_png(&png).unwrap();
        let document = zpl::render(&source, ZQ610_PLUS_203_DPI).unwrap();
        let actual = zpl::output::raster::rasterize(&document.labels[0]).unwrap();
        let comparison = raster_diff::compare_stats(&reference, &actual, false).unwrap();
        if matches!(columns[0], "text-0-B" | "text-0-I" | "text-0-R") {
            // Each image contains exactly one text field; foreground IoU is
            // independent of the large white canvas. Both physical printers
            // show the same residual rotated Font 0 strike differences.
            let union =
                comparison.both_black + comparison.reference_only + comparison.candidate_only;
            assert!(union > 0 && comparison.both_black * 1000 >= union * 985);
        } else {
            assert!(comparison.matches(), "{} full native raster", columns[0]);
        }
        assert_eq!(comparison.both_black, columns[6].parse::<usize>().unwrap());
        if comparison.both_black == 0 {
            assert_eq!(
                columns[0], "width-late-grow",
                "only the explicit clipped diagnostic may be blank"
            );
        }
        assert_eq!(
            comparison.reference_only,
            columns[3].parse::<usize>().unwrap()
        );
        assert_eq!(
            comparison.candidate_only,
            columns[4].parse::<usize>().unwrap()
        );
        assert_eq!(digest::sha256(&actual.pixels), columns[5]);
        count += 1;
    }
    assert_eq!(count, 120);
}

#[test]
fn preview_height_policy_is_independent_and_caller_configurable() {
    let source = b"^XA^PW384^LL100^FO20,150^GB30,20,3^FS^XZ";
    let native = zpl::render(source, ZQ610_PLUS_203_DPI).unwrap();
    assert_eq!(native.labels[0].height, 2030);
    for options in [SPECIFICATION, ZD621_203_DPI] {
        assert_eq!(zpl::render(source, options).unwrap().labels[0].height, 100);
    }
    let mut options = ZQ610_PLUS_203_DPI;
    options.height = 600;
    assert_eq!(zpl::render(source, options).unwrap().labels[0].height, 600);
    options.compatibility.preview_ignores_label_length = false;
    assert_eq!(zpl::render(source, options).unwrap().labels[0].height, 100);
    assert_eq!(zpl::Options::default(), ZD621_203_DPI);
}

#[test]
fn preview_width_rounding_cap_and_latching_are_independent() {
    let input = b"^XA^PW65^FO0,20^GB85,10,10^FS^XZ";
    let render = |options| zpl::render(input, options).unwrap().labels.remove(0);
    let native = render(ZQ610_PLUS_203_DPI);
    assert_eq!(native.width, 128);
    let raster = zpl::output::raster::rasterize(&native).unwrap();
    assert_eq!(raster.pixels[20 * 128 + 30], 255);
    assert_eq!(raster.pixels[20 * 128 + 31], 0);
    assert_eq!(raster.pixels[20 * 128 + 115], 0);
    assert_eq!(raster.pixels[20 * 128 + 116], 255);
    let mut options = ZQ610_PLUS_203_DPI;
    options.compatibility.preview_width_quantum = None;
    assert_eq!(render(options).width, 65);
    let wide = b"^XA^PW832^XZ";
    assert_eq!(zpl::render(wide, options).unwrap().labels[0].width, 384);
    options.compatibility.preview_max_width = None;
    assert_eq!(zpl::render(wide, options).unwrap().labels[0].width, 832);
    let late = b"^XA^PW120^FO100,20^GB40,20,3^FS^PW384^XZ";
    assert_eq!(
        zpl::render(late, ZQ610_PLUS_203_DPI).unwrap().labels[0].width,
        128
    );
    options = ZQ610_PLUS_203_DPI;
    options.compatibility.preview_width_latched_at_first_draw = false;
    assert_eq!(zpl::render(late, options).unwrap().labels[0].width, 384);
    options.compatibility.preview_width_quantum = Some(0);
    assert!(zpl::render(input, options).is_err());
}
