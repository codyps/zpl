use zpl::render::profiles::SPECIFICATION;
use zpl::{
    output::raster::Raster,
    output::{Adapter, Draw, Paint, Path, Png, Point, Scene, Segment, Svg},
    render, Options,
};
fn scene(z: &[u8]) -> Scene {
    render(
        z,
        Options {
            width: 100,
            height: 100,
            dpi: 203,
            ..SPECIFICATION
        },
    )
    .unwrap()
    .labels
    .remove(0)
}
#[test]
fn shape_pixels_and_reverse() {
    let s = scene(b"^XA^FO2,3^GB10,8,2^FS^FO4,5^GB2,2,2^FR^FS^XZ");
    let r = zpl::output::raster::rasterize(&s).unwrap();
    assert_eq!(r.pixels.iter().filter(|&&p| p == 0).count(), 60);
    assert_eq!(r.pixels[3 * 100 + 2], 0);
    assert_eq!(r.pixels[5 * 100 + 6], 255);
    assert_eq!(r.pixels[5 * 100 + 4], 0);
}
#[test]
fn geometry_shared_by_adapters() {
    let s = scene(b"^XA^FO10,10^GE30,20,3^FS^XZ");
    assert!(s.draws[0]
        .path
        .segments
        .iter()
        .any(|s| matches!(s, Segment::Cubic(..))));
    let svg = String::from_utf8(Svg.encode(&s).unwrap()).unwrap();
    assert!(svg.contains("fill-rule=\"evenodd\""));
    assert!(!svg.contains("<text"));
    assert!(Png.encode(&s).unwrap().starts_with(b"\x89PNG\r\n\x1a\n"));
    let r = zpl::output::raster::rasterize(&s).unwrap();
    assert_eq!(r.pixels[20 * 100 + 25], 255);
    assert_eq!(r.pixels[10 * 100 + 25], 0);
}
#[test]
fn inversion_and_clipping() {
    let mut s = Scene::new(4, 4, 203).unwrap();
    let mut p = Path::default();
    p.rect(-2., -2., 5., 5.);
    s.draws.push(Draw {
        path: p.clone(),
        paint: Paint::Black,
    });
    s.draws.push(Draw {
        path: p,
        paint: Paint::Invert,
    });
    assert!(zpl::output::raster::rasterize(&s)
        .unwrap()
        .pixels
        .iter()
        .all(|&v| v == 255));
}
#[test]
fn errors_are_explicit() {
    for z in [
        "^XA^BQN,3^FDLA,X^FS^XZ",
        "^XA^A@N,20,20,FONT^FDX^FS^XZ",
        "^XA^FO1,2^FDa^XZ",
        "^XA^GB2,2^GB2,2^FS^XZ",
        "^XA^BCN,20,N,N,N,Z^FD123^FS^XZ",
        "^XA^PW-1^XZ",
        "^XA^GB2,2,1,B,9^FS^XZ",
        "^XA^FDé^FS^XZ",
        "^XA^FT,2^FDX^FS^XZ",
        "^XA~FO1,2^XZ",
    ] {
        assert!(render(z.as_bytes(), SPECIFICATION).is_err(), "{z}")
    }
    let e = render(b"^XA^BQN,2,4,L,0,extra", SPECIFICATION).unwrap_err();
    assert_eq!(e.offset, 3);
    assert!(e.message.contains("BQ"));
}
#[test]
fn syntax_and_multiple_labels() {
    let doc = render(
        b"^CC!!XA!CD;!PW20!LL30!FO2;3!GB4;5;4!FS!XZ!XA!XZ",
        SPECIFICATION,
    )
    .unwrap();
    assert_eq!(doc.labels.len(), 2);
    assert_eq!((doc.labels[1].width, doc.labels[1].height), (20, 30));
    assert_eq!(
        zpl::output::raster::rasterize(&doc.labels[0])
            .unwrap()
            .pixels
            .iter()
            .filter(|&&v| v == 0)
            .count(),
        20
    );
}
#[test]
fn graphics_binary_ascii_and_rle() {
    let a = scene(b"^XA^GFA,2,2,1,FF81^FS^XZ");
    let b = scene(b"^XA^GFB,2,2,1,\xff\x81^FS^XZ");
    let c = scene(b"~DGR:LOGO.GRF,2,1,HF81^XA^XGR:LOGO.GRF,1,1^FS^XZ");
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_eq!(
        zpl::output::raster::rasterize(&a)
            .unwrap()
            .pixels
            .iter()
            .filter(|&&v| v == 0)
            .count(),
        10
    );
}
#[test]
fn field_blocks_hex_and_font_warning() {
    let d = render(
        b"^XA^CF0,32,32^FO20,20^FB60,2,1,L^FH^FDA_20B\\&C^FS^XZ",
        SPECIFICATION,
    )
    .unwrap();
    assert!(d.warnings.is_empty());
    let expected = render(
        b"^XA^CF0,32,32^FO20,20^FDA B^FS^FO20,53^FDC^FS^XZ",
        SPECIFICATION,
    )
    .unwrap();
    assert_eq!(
        zpl::output::raster::rasterize(&d.labels[0]).unwrap(),
        zpl::output::raster::rasterize(&expected.labels[0]).unwrap()
    );
    let scaled = render(b"^XA^CF0,16^FDA^FS^XZ", SPECIFICATION).unwrap();
    assert_eq!(scaled.warnings.len(), 1);
}

#[test]
fn rotations_keep_fo_positive() {
    for orientation in ["N", "R", "I", "B"] {
        let z = format!("^XA^FW{orientation}^FO20,20^GB10,5,5^FS^XZ");
        let r = zpl::output::raster::rasterize(&scene(z.as_bytes())).unwrap();
        assert_eq!(r.pixels.iter().filter(|&&v| v == 0).count(), 50);
        assert_eq!(r.pixels[20 * 100 + 20], 0);
        assert_eq!(r.pixels[19 * 100 + 20], 255)
    }
}
#[test]
fn code128_known_symbol_widths() {
    let s = scene(b"^XA^BY1^BCN,10,N^FDA^FS^XZ");
    let r = zpl::output::raster::rasterize(&s).unwrap();
    let row = &r.pixels[..100];
    let expected = "1101001000010100011000100010110001100011101011";
    assert_eq!(expected.len(), 46);
    for (i, b) in expected.bytes().enumerate() {
        assert_eq!(row[i], if b == b'1' { 0 } else { 255 }, "module {i}")
    }
}
#[test]
fn invalid_output_rejected_by_both_adapters() {
    let mut s = Scene::new(10, 10, 203).unwrap();
    s.draws.push(Draw {
        path: Path {
            segments: vec![Segment::Line(Point::new(1., 1.))],
        },
        paint: Paint::Black,
    });
    assert!(Svg.encode(&s).is_err());
    assert!(Png.encode(&s).is_err());
    assert!(Scene::new(u32::MAX, u32::MAX, 203).is_err());
}

#[test]
fn compressed_graphics_and_crc() {
    let expected = scene(b"^XA^GFA,2,2,1,FF81^FS^XZ");
    assert_eq!(scene(b"^XA^GFA,2,2,1,:B64:/4E=:EF02^FS^XZ"), expected);
    assert!(render(b"^XA^GFA,2,2,1,:B64:/4E=:0000^FS^XZ", SPECIFICATION).is_err());
    assert_eq!(
        scene(b"^XA^GFA,2,2,1,:Z64:eJz73wgAAoEBgQ==:12CE^FS^XZ"),
        expected
    );
    assert!(render(
        b"^XA^GFA,2,2,1,:Z64:eJz73wgAAoEBgQ==:0000^FS^XZ",
        SPECIFICATION
    )
    .is_err());
}
#[test]
fn practical_label_fixture() {
    let d = render(
        include_bytes!("../../docs/examples/local-label.zpl"),
        SPECIFICATION,
    )
    .unwrap();
    assert_eq!((d.labels[0].width, d.labels[0].height), (600, 400));
    assert_eq!(d.labels[0].draws.len(), 9);
    assert!(Png.encode(&d.labels[0]).is_ok());
}

#[test]
fn font_override_is_field_local_and_zero_width_is_inferred() {
    let a = scene(b"^XA^CF0,7,6^FO0,0^A0N,14,0^FDA^FS^FO20,0^FDA^FS^XZ");
    let b = scene(b"^XA^CF0,7,6^FO0,0^A0N,14,14^FDA^FS^FO20,0^A0N,7,6^FDA^FS^XZ");
    assert_eq!(a, b);
    for z in [b"^FDLOST^XA^XZ".as_slice(), b"^GFB,1,1,1,\xff^XA^XZ"] {
        assert!(render(z, SPECIFICATION).is_err())
    }
}
#[test]
fn malformed_streams_never_panic() {
    let mut state = 42u64;
    for len in 0..256 {
        let mut data = b"^XA".to_vec();
        for _ in 0..len {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            data.push((state >> 32) as u8)
        }
        let _ = render(&data, SPECIFICATION);
    }
}

#[test]
fn captured_font_matches_every_printer_atlas_and_held_out_text() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/font0-32");
    let names = (0..12)
        .map(|i| format!("page-{i:03}"))
        .chain(std::iter::once("verification".into()));
    for name in names {
        let zpl = std::fs::read(root.join(format!("{name}.zpl"))).unwrap();
        let reference = Raster::decode_png_with_threshold(
            &std::fs::read(root.join(format!("{name}.png"))).unwrap(),
            None,
        )
        .unwrap();
        let doc = render(&zpl, zpl::render::profiles::ZD621_203_DPI).unwrap();
        assert!(doc.warnings.is_empty(), "{name}");
        let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
        let diff = raster_diff::compare(&reference, &actual, false).unwrap();
        assert!(
            diff.matches(),
            "{name}: {} differing pixels",
            diff.different_pixels()
        );
    }
}
#[test]
fn proportional_blocks_align_and_wrap_by_advance() {
    // WW advances 52 dots; ii advances 16 dots, despite both containing two characters.
    let a = scene(b"^XA^CF0,32,32^FO10,10^FB55,2,0,R^FDWW ii^FS^XZ");
    let b = scene(b"^XA^CF0,32,32^FO13,10^FDWW^FS^FO49,42^FDii^FS^XZ");
    assert_eq!(
        zpl::output::raster::rasterize(&a).unwrap(),
        zpl::output::raster::rasterize(&b).unwrap()
    );
    let c = scene(b"^XA^CF0,32,32^FO10,10^FB56,1,0,C^FDii^FS^XZ");
    let d = scene(b"^XA^CF0,32,32^FO30,10^FDii^FS^XZ");
    assert_eq!(
        zpl::output::raster::rasterize(&c).unwrap(),
        zpl::output::raster::rasterize(&d).unwrap()
    );
    assert!(render(b"^XA^AZN,32,32^FDA^FS^XZ", SPECIFICATION).is_err());
}

#[test]
fn captured_font_layout_baselines_blocks_and_rotations() {
    let input = include_bytes!("fixtures/font0-32/layout.zpl");
    let reference = Raster::decode_png(include_bytes!("fixtures/font0-32/layout.png")).unwrap();
    let doc = render(input, zpl::render::profiles::ZD621_203_DPI).unwrap();
    let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
    assert!(raster_diff::compare(&reference, &actual, false)
        .unwrap()
        .matches());
    let input = include_bytes!("fixtures/font0-32/rotations.zpl");
    let reference = Raster::decode_png(include_bytes!("fixtures/font0-32/rotations.png")).unwrap();
    let doc = render(input, zpl::render::profiles::ZD621_203_DPI).unwrap();
    assert_eq!(doc.warnings.len(), 1);
    let actual = zpl::output::raster::rasterize(&doc.labels[0]).unwrap();
    // Zebra rasterizes rotated outlines slightly differently: three edge pixels
    // in this sample cannot be reproduced by rotating the captured normal strike.
    let d = raster_diff::compare(&reference, &actual, false).unwrap();
    assert!(d.different_pixels() <= 3);
}
#[test]
fn unsampled_font_sizes_scale_the_fallback_strike() {
    let normal = scene(b"^XA^CF0,32,32^FT10,40^FDaWj|^FS^XZ");
    let enlarged = scene(b"^XA^PW400^LL400^CF0,128,128^FT40,160^FDaWj|^FS^XZ");
    let a = zpl::output::raster::rasterize(&normal).unwrap();
    let b = zpl::output::raster::rasterize(&enlarged).unwrap();
    for y in 0..400 {
        for x in 0..400 {
            assert_eq!(b.pixels[y * 400 + x], a.pixels[y / 4 * 100 + x / 4]);
        }
    }
}

#[test]
fn field_block_overshoots_union_with_descenders() {
    let block = scene(b"^XA^CF0,32^FO20,20^FB60,2,0,L^FD_\\&O^FS^XZ");
    let separate = scene(b"^XA^CF0,32^FO20,20^FD_^FS^FO20,52^FDO^FS^XZ");
    assert_eq!(
        zpl::output::raster::rasterize(&block).unwrap(),
        zpl::output::raster::rasterize(&separate).unwrap()
    );
}

#[test]
fn field_block_overflow_unions_ink_on_the_last_row() {
    // ^FB p. 186 specifies overprinting the final row. Real-printer controls:
    // field-block-overflow-zd621-v1, including max-lines 1/2 and every rotation.
    for max_lines in [1, 2, 3] {
        let block = scene(
            format!("^XA^AAN,9,5^FO20,20^FB60,{max_lines},4,L^FDAB\\&CD\\&EF\\&GH^FS^XZ")
                .as_bytes(),
        );
        let mut separate = String::from("^XA");
        for (i, text) in ["AB", "CD", "EF", "GH"].iter().enumerate() {
            let y = 20 + i.min(max_lines - 1) * 13;
            separate.push_str(&format!("^AAN,9,5^FO20,{y}^FD{text}^FS"));
        }
        separate.push_str("^XZ");
        assert_eq!(
            zpl::output::raster::rasterize(&block).unwrap(),
            zpl::output::raster::rasterize(&scene(separate.as_bytes())).unwrap()
        );
    }
}
