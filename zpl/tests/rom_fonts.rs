//! ROM selection and resolver contracts. ^A@ and ^CW:
//! https://docs.zebra.com/us/en/printers/software/zpl-pg/zpl-commands/%5Ea-.html
//! Dataset: ZD621, 203 dpi, V93.21.33Z. These are local integration checks;
//! they do not establish new named-font printer parity.
use std::sync::{Arc, Mutex};
use zpl::{
    bitmap_font::{Glyph, Settings},
    fonts::{resolve_rom_font, Face, Fonts},
    render::{profiles::SPECIFICATION, render_with_fonts},
};
use zpl_bitmap_fonts::collection::{Encoding, Status};

fn image(zpl: &str, fonts: &Fonts<'_>) -> raster_diff::Raster {
    let doc = render_with_fonts(zpl.as_bytes(), SPECIFICATION, fonts).unwrap();
    zpl::output::raster::rasterize(&doc.labels[0]).unwrap()
}

#[test]
fn every_bundled_rom_name_resolves_and_renders_its_measured_pixels() {
    for font in zpl_bitmap_fonts::zd621::FONTS {
        assert!(resolve_rom_font(font.name).unwrap().is_some());
        let map = font.encoding(Encoding::Input { ci: 28 }).unwrap();
        let (code, glyph) = map
            .inputs
            .iter()
            .find_map(|&code| {
                if code <= 32 || (127..160).contains(&code) {
                    return None;
                }
                let glyph = font.encoded_glyph(Encoding::Input { ci: 28 }, code)?;
                (glyph.width > 0 && glyph.height > 0).then_some((code, glyph))
            })
            .unwrap();
        let character = char::from_u32(code).unwrap();
        let field = character
            .to_string()
            .as_bytes()
            .iter()
            .map(|b| format!("_{b:02X}"))
            .collect::<String>();
        let direct = format!(
            "^XA^CI28^PW400^LL300^FO100,100^A@N,1,1,{}^FH^FD{field}^FS^XZ",
            font.name
        );
        let alias = direct.replace(&format!("^A@N,1,1,{}", font.name), "^AXN,1,1");
        let alias = format!("^CWX,{}{}", font.name, alias);
        let actual = image(&direct, &Fonts::new());
        assert_eq!(actual, image(&alias, &Fonts::new()), "{}", font.name);
        // Construct the expected native canvas directly from the captured pixels
        // and bearings, independently of named selection and layout.
        let baseline = font.cell_metrics().map_or(0, |m| i32::from(m.baseline) - 1);
        let mut expected = vec![255; 400 * 300];
        for y in 0..glyph.height {
            for x in 0..glyph.width {
                if glyph.pixel(x, y) {
                    let px = 100 + i32::from(glyph.left) + i32::from(x);
                    let py = 100 + baseline + i32::from(glyph.top) + i32::from(y);
                    expected[py as usize * 400 + px as usize] = 0;
                }
            }
        }
        assert_eq!(actual.pixels, expected, "{}", font.name);
    }
}

#[test]
fn rom_encoding_remaps_scaling_and_orientations_match_resident_strikes() {
    for (id, name, h, w) in [('E', "Z:E8.FNT", 28, 15), ('H', "Z:H8.FNT", 21, 13)] {
        for rotation in ['N', 'R', 'I', 'B'] {
            for (ci, data) in [
                ("0", "ABC_5C"),
                ("13", "012_80"),
                ("27", "ABC"),
                ("28", "ABC"),
                ("0,65,66", "BBB"),
            ] {
                let prefix = format!("^XA^PW400^LL400^CI{ci}^FO150,150");
                let resident = format!("{prefix}^A{id}{rotation},{h},{w}^FH^FD{data}^FS^XZ");
                let named = format!("{prefix}^A@{rotation},{h},{w},{name}^FH^FD{data}^FS^XZ");
                assert!(
                    image(&resident, &Fonts::new()) == image(&named, &Fonts::new()),
                    "{named}"
                );
                let rounded =
                    named.replace(&format!(",{h},{w},"), &format!(",{},{},", h + 1, w + 1));
                assert_eq!(image(&named, &Fonts::new()), image(&rounded, &Fonts::new()));
            }
        }
    }
}

fn synthetic() -> Result<Face<'static>, String> {
    Face::bitmap(
        Settings {
            font: '0',
            width: 10,
            height: 10,
            dpi: 203,
        },
        vec![Glyph {
            codepoint: 'A' as u32,
            advance: 4,
            left: 0,
            top: -2,
            width: 1,
            height: 2,
            bitmap: vec![vec![128]; 2],
        }],
        2.,
    )
}

#[test]
fn resolver_normalizes_caches_delegates_and_is_scoped_to_each_render() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let seen = calls.clone();
    let mut fonts = Fonts::new();
    fonts.set_resolver(move |name| {
        seen.lock().unwrap().push(name.to_owned());
        if name == "R:BRAND.FNT" {
            synthetic().map(Some)
        } else {
            resolve_rom_font(name)
        }
    });
    let source = "^CWX,brand.fnt^XA^PW300^LL100^FO20,20^AXN,10,10^FDA^FS^FO30,20^A@N,10,10,r:brand.fnt^FDA^FS^FO50,20^A@N,28,15,z:e8.fnt^FDA^FS^XZ";
    let first = image(source, &fonts);
    assert_eq!(*calls.lock().unwrap(), ["R:BRAND.FNT", "Z:E8.FNT"]);
    assert_eq!(first, image(source, &fonts.clone()));
    assert_eq!(calls.lock().unwrap().len(), 4);
    let mut overridden = Fonts::new();
    overridden.set_resolver(|_| Err("resolver should not run".into()));
    // Explicit registration wins, including for a ROM name.
    overridden
        .insert_named_bitmap(
            "Z:E8.FNT",
            Settings {
                font: '0',
                width: 10,
                height: 10,
                dpi: 203,
            },
            vec![Glyph {
                codepoint: 65,
                advance: 4,
                left: 0,
                top: -2,
                width: 1,
                height: 2,
                bitmap: vec![vec![128]; 2],
            }],
            2.,
        )
        .unwrap();
    image("^XA^A@N,10,10,Z:E8.FNT^FDA^FS^XZ", &overridden);
}

#[test]
fn unknown_unmeasured_and_resolver_errors_are_explicit() {
    assert!(resolve_rom_font("Z:MISSING.FNT").unwrap().is_none());
    assert!(resolve_rom_font("R:E8.FNT").unwrap().is_none());
    assert!(resolve_rom_font("Z:../E8.FNT").is_err());
    let mut fonts = Fonts::new();
    fonts.set_resolver(|_| Err("application font lookup failed".into()));
    for command in ["^CWX,R:BRAND.FNT", "^A@N,10,10,R:BRAND.FNT"] {
        let err = render_with_fonts(format!("^XA{command}^XZ").as_bytes(), SPECIFICATION, &fonts)
            .unwrap_err();
        assert_eq!(err.offset, 3);
        assert_eq!(err.message, "application font lookup failed");
    }
    fonts.set_resolver(|_| Ok(None));
    assert!(
        render_with_fonts(b"^XA^A@N,28,15,Z:E8.FNT^FDA^FS^XZ", SPECIFICATION, &fonts)
            .unwrap_err()
            .message
            .contains("unresolved named font")
    );
    for name in ["Z:EPL6.FNT", "Z:EPL7.FNT"] {
        let err = render_with_fonts(
            format!("^XA^A@N,30,30,{name}^FD0^FS^XZ").as_bytes(),
            SPECIFICATION,
            &Fonts::new(),
        )
        .unwrap_err();
        assert!(err.message.contains("no calibrated sizing metrics"));
    }
    // The survey marked this Unicode input blank without establishing advance.
    let font = zpl_bitmap_fonts::zd621::font_by_name("Z:E12.FNT").unwrap();
    assert_eq!(
        font.encoding(Encoding::Input { ci: 28 })
            .unwrap()
            .lookup(0x400)
            .unwrap()
            .status,
        Status::BlankUnresolved
    );
    let err = render_with_fonts(
        "^XA^CI28^A@N,41,20,Z:E12.FNT^FDЀ^FS^XZ".as_bytes(),
        SPECIFICATION,
        &Fonts::new(),
    )
    .unwrap_err();
    assert!(err.message.contains("unsupported embedded font glyph"));
}

#[test]
fn resolver_can_return_a_face_borrowing_application_bytes() {
    use zpl::truetype::Hinting;
    let bytes =
        include_bytes!("fixtures/truetype-regression/font-probes-20261002/probe.ttf").to_vec();
    let mut fonts = Fonts::new();
    fonts.set_resolver(|name| {
        if name == "R:PROBE.TTF" {
            Face::truetype(&bytes, Hinting::None).map(Some)
        } else {
            resolve_rom_font(name)
        }
    });
    let mut registered = Fonts::new();
    registered
        .insert_named_truetype("R:PROBE.TTF", &bytes, Hinting::None)
        .unwrap();
    let source = "^XA^PW100^LL100^FO20,20^A@N,20,20,R:PROBE.TTF^FD!^FS^XZ";
    assert_eq!(image(source, &fonts), image(source, &registered));
}

#[test]
fn downloads_supersede_resolved_faces_but_cannot_write_rom() {
    let mut fonts = Fonts::new();
    fonts.set_resolver(|_| synthetic().map(Some));
    let before = "^XA^PW100^LL100^FO10,10^A@N,10,10,R:TEST.FNT^FDA^FS^XZ";
    let download = "~DBR:TEST.FNT,N,10,10,7,4,1,TEST,#0041.2.8.1.7.9.FF81";
    let source = format!("^CWX,R:TEST.FNT{download}^XA^PW100^LL100^FO10,10^AXN,10,10^FDA^FS^XZ");
    let expected = format!("{download}{}", before);
    assert_eq!(image(&source, &fonts), image(&expected, &Fonts::new()));
    assert_ne!(image(before, &fonts), image(&source, &fonts));
    let err = render_with_fonts(
        download.replace("R:", "Z:").as_bytes(),
        SPECIFICATION,
        &Fonts::new(),
    )
    .unwrap_err();
    assert!(err.message.contains("read-only ROM"));
}

#[test]
fn rom_aliases_replace_scalable_and_preset_ids_without_changing_geometry() {
    use zpl::render::profiles::ZD621_203_DPI;
    for profile in [SPECIFICATION, ZD621_203_DPI] {
        for id in ['0', 'P', 'H', 'X'] {
            let direct = b"^XA^PW200^LL200^FO40,40^A@R,41,20,Z:E12.FNT^FDABC^FS^XZ";
            let alias =
                format!("^CW{id},Z:E12.FNT^XA^PW200^LL200^FO40,40^A{id}R,41,20^FDABC^FS^XZ");
            let a = render_with_fonts(direct, profile, &Fonts::new()).unwrap();
            let b = render_with_fonts(alias.as_bytes(), profile, &Fonts::new()).unwrap();
            assert_eq!(a.labels, b.labels, "alias {id}");
        }
    }
}

#[test]
fn resolver_returns_format_independent_bitmap_providers() {
    use std::borrow::Cow;
    use zpl::fonts::{BitmapFont, BitmapMetrics};
    struct Strike(Glyph);
    impl BitmapFont for Strike {
        fn metrics(&self) -> BitmapMetrics {
            BitmapMetrics {
                width: 10,
                height: 10,
                baseline: 2.,
            }
        }
        fn glyph(&self, character: char) -> Result<Option<Cow<'_, Glyph>>, String> {
            Ok((character as u32 == self.0.codepoint).then_some(Cow::Borrowed(&self.0)))
        }
    }
    let strike = Arc::new(Strike(Glyph {
        codepoint: 65,
        advance: 4,
        left: 0,
        top: -2,
        width: 1,
        height: 2,
        bitmap: vec![vec![128]; 2],
    }));
    let mut registered = Fonts::new();
    registered
        .insert_named_bitmap_font("R:BRAND.FNT", strike.clone())
        .unwrap();
    let mut resolved = Fonts::new();
    resolved.set_resolver(move |path| {
        if path == "R:BRAND.FNT" {
            Face::provider(strike.clone()).map(Some)
        } else {
            resolve_rom_font(path)
        }
    });
    for selection in ["^A@N,20,30,R:BRAND.FNT", "^CWX,R:BRAND.FNT^AXN,20,30"] {
        let source = format!("^XA^PW100^LL100^FO10,10{selection}^FDAA^FS^XZ");
        assert_eq!(image(&source, &resolved), image(&source, &registered));
    }
}
