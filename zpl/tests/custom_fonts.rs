//! API regressions for supplied fonts. ZPL font selection and field placement:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
//! ^A, ^CF, ^FB, ^FO, ^FT. These synthetic fonts assert local behavior, not printer parity.
use zpl::{
    bitmap_font::{Glyph, Settings},
    render::{fonts::Fonts, profiles::SPECIFICATION, render_with_fonts},
    truetype::Hinting,
};

fn bitmap(advance: u32) -> (Settings, Vec<Glyph>) {
    (
        Settings {
            font: '0',
            width: 10,
            height: 10,
            dpi: 203,
        },
        vec![
            Glyph {
                codepoint: 'A' as u32,
                advance,
                left: 0,
                top: -7,
                width: 2,
                height: 3,
                bitmap: vec![vec![0xc0]; 3],
            },
            Glyph {
                codepoint: ' ' as u32,
                advance,
                left: 0,
                top: 0,
                width: 0,
                height: 0,
                bitmap: vec![],
            },
        ],
    )
}
fn fonts(id: char, advance: u32) -> Fonts<'static> {
    let mut fonts = Fonts::new();
    let (s, glyphs) = bitmap(advance);
    fonts.insert_bitmap(id, s, glyphs, 7.).unwrap();
    fonts
}
fn image(source: &str, fonts: &Fonts<'_>) -> raster_diff::Raster {
    let document = render_with_fonts(source.as_bytes(), SPECIFICATION, fonts).unwrap();
    zpl::output::raster::rasterize(&document.labels[0]).unwrap()
}

#[test]
fn bitmap_selection_scaling_and_render_scope() {
    let source = "^XA^PW100^LL100^FO10,10^A0N,10,10^FDAA^FS^XZ";
    let normal = zpl::render(source.as_bytes(), SPECIFICATION).unwrap();
    let empty = render_with_fonts(source.as_bytes(), SPECIFICATION, &Fonts::new()).unwrap();
    assert_eq!(normal.labels, empty.labels);
    let custom = fonts('0', 4);
    let rendered = image(source, &custom);
    assert_ne!(
        rendered,
        zpl::output::raster::rasterize(&normal.labels[0]).unwrap()
    );
    // A synthetic two-dot-wide glyph at 10 dots with a four-dot advance.
    let black: Vec<_> = rendered
        .pixels
        .iter()
        .enumerate()
        .filter(|(_, v)| **v == 0)
        .map(|(i, _)| (i % 100, i / 100))
        .collect();
    assert_eq!(
        black,
        vec![
            (10, 10),
            (11, 10),
            (14, 10),
            (15, 10),
            (10, 11),
            (11, 11),
            (14, 11),
            (15, 11),
            (10, 12),
            (11, 12),
            (14, 12),
            (15, 12)
        ]
    );
    assert_eq!(
        rendered,
        image(&source.replace("^A0N,10,10", "^CFZ,10,10"), &fonts('Z', 4))
    );
    assert_eq!(
        rendered,
        image(&source.replace("^A0N", "^AZN"), &fonts('Z', 4))
    );
    assert_ne!(rendered, image(source, &fonts('0', 7)));
    assert_eq!(
        normal.labels,
        zpl::render(source.as_bytes(), SPECIFICATION)
            .unwrap()
            .labels
    );
    for rotation in ['N', 'R', 'I', 'B'] {
        let source = format!("^XA^PW100^LL100^FO50,50^AZ{rotation},20,30^FDAA^FS^XZ");
        assert_eq!(
            image(&source, &fonts('Z', 4)),
            image(&source.replace("AZ", "A0"), &custom)
        );
    }
}

#[test]
fn bitmap_metrics_drive_baseline_and_wrapping() {
    let custom = fonts('Z', 4);
    let fo = "^XA^PW100^LL100^FO10,10^AZN,10,10^FDA^FS^XZ";
    assert_eq!(
        image(fo, &custom),
        image(&fo.replace("^FO10,10", "^FT10,17"), &custom)
    );
    let block = "^XA^PW100^LL100^FO10,10^AZN,10,10^FB12,2,0,L^FDAA AA^FS^XZ";
    let manual = "^XA^PW100^LL100^FO10,10^AZN,10,10^FDAA^FS^FO10,20^AZN,10,10^FDAA^FS^XZ";
    assert_eq!(image(block, &custom), image(manual, &custom));
}

#[test]
fn invalid_fonts_and_missing_glyphs_report_errors() {
    let mut custom = fonts('Z', 4);
    assert!(custom
        .insert_truetype('Z', b"invalid", Hinting::None)
        .is_err());
    let (s, mut glyphs) = bitmap(4);
    glyphs.push(glyphs[0].clone());
    assert!(custom.insert_bitmap('Z', s, glyphs, 7.).is_err());
    let error = render_with_fonts(b"^XA^AZN,10,10^FDB^FS^XZ", SPECIFICATION, &custom).unwrap_err();
    assert!(
        error.message.contains("unsupported custom font glyph 'B'"),
        "{error}"
    );
    // Failed assignments leave the prior face intact.
    image("^XA^AZN,10,10^FDA^FS^XZ", &custom);
}

const TTF: &[u8] = include_bytes!("fixtures/truetype-regression/font-probes-20261002/probe.ttf");
#[test]
fn truetype_uses_existing_engine_at_requested_sizes() {
    use zpl::{
        output::raster::truetype::{rasterize, ScanMode},
        truetype::{Environment, Font, Size},
    };
    let face = Font::parse(TTF).unwrap();
    let c = '!'; // Constructed identity glyph: nonempty vertical stems.
    for hinting in [Hinting::None, Hinting::Native] {
        let mut custom = Fonts::new();
        custom.insert_truetype('Z', TTF, hinting).unwrap();
        for (w, h) in [(20, 20), (31, 24)] {
            let instance = face
                .instance(Size::new(w, h).unwrap(), hinting, Environment::Standard)
                .unwrap();
            let index = face.glyph_index(c).unwrap();
            let glyph = rasterize(
                &instance.outline(index).unwrap(),
                c as u32,
                instance.layout_advance(index).unwrap(),
                0,
                ScanMode::Center,
            )
            .unwrap();
            assert!(glyph.bitmap.iter().flatten().any(|&byte| byte != 0));
            let ascent = i16::from_be_bytes(face.table(b"hhea").unwrap()[4..6].try_into().unwrap());
            let baseline = f64::from(ascent) * f64::from(h) / f64::from(face.units_per_em());
            // Encode engine output as a bitmap face to check the complete render integration.
            let mut reference = Fonts::new();
            reference
                .insert_bitmap(
                    'Z',
                    Settings {
                        font: '0',
                        width: w as u32,
                        height: h as u32,
                        dpi: 203,
                    },
                    vec![glyph],
                    baseline,
                )
                .unwrap();
            for rotation in ['N', 'R', 'I', 'B'] {
                let source = format!("^XA^PW150^LL150^FO70,70^AZ{rotation},{h},{w}^FD{c}{c}^FS^XZ");
                assert_eq!(image(&source, &custom), image(&source, &reference));
            }
        }
    }
}

#[test]
fn captions_stored_formats_and_output_adapters_use_supplied_faces() {
    use zpl::output::{Adapter, Pdf, Png, Svg};
    let custom = fonts('Z', 4);
    let caption = "^XA^PW200^LL100^FO10,10^AZN,10,10^BCN,30,Y,N,N^FDAA^FS^XZ";
    let changed = fonts('Z', 8);
    assert_ne!(image(caption, &custom), image(caption, &changed));
    let hidden = caption.replace(",Y,N,N", ",N,N,N");
    assert_eq!(image(&hidden, &custom), image(&hidden, &changed));
    let inline = "^XA^PW100^LL100^FO10,10^AZN,10,10^FDAA^FS^XZ";
    let stored = "^XA^DFR:CUSTOM.ZPL^FO10,10^AZN,10,10^FDAA^FS^XZ^XA^PW100^LL100^XFR:CUSTOM.ZPL^XZ";
    assert_eq!(image(inline, &custom), image(stored, &custom));
    let doc = render_with_fonts(inline.as_bytes(), SPECIFICATION, &custom).unwrap();
    assert!(doc.warnings.is_empty());
    let scene = &doc.labels[0];
    assert!(Png.encode(scene).unwrap().starts_with(b"\x89PNG"));
    assert!(Svg.encode(scene).unwrap().starts_with(b"<svg"));
    assert!(Pdf.encode(scene).unwrap().starts_with(b"%PDF"));
    let limits = zpl::render::Limits {
        field_bytes: 1,
        ..Default::default()
    };
    assert!(zpl::render::render_with_fonts_and_limits(
        inline.as_bytes(),
        SPECIFICATION,
        &custom,
        limits
    )
    .unwrap_err()
    .message
    .contains("field data exceeds"));
}

#[test]
fn packed_bitmap_and_bounded_text_use_custom_metrics() {
    let mut packed = Fonts::new();
    packed
        .insert_zbf('0', include_bytes!("../assets/font0-32.zbf"), 24.)
        .unwrap();
    let source = b"^XA^FO30,30^A0N,32,32^FDABC^FS^XZ";
    let resident = zpl::render(source, SPECIFICATION).unwrap();
    let custom = render_with_fonts(source, SPECIFICATION, &packed).unwrap();
    assert_eq!(
        zpl::output::raster::rasterize(&resident.labels[0]).unwrap(),
        zpl::output::raster::rasterize(&custom.labels[0]).unwrap()
    );
    let custom = fonts('Z', 4);
    let bounded = "^XA^PW100^LL100^FO10,10^AZN,10,10^TBN,12,10^FDAA AA^FS^XZ";
    let single = "^XA^PW100^LL100^FO10,10^AZN,10,10^FDAA^FS^XZ";
    assert_eq!(image(bounded, &custom), image(single, &custom));
}

#[test]
fn true_type_sizes_and_assignment_ids_are_checked() {
    let mut custom = Fonts::new();
    assert!(custom.insert_truetype('a', TTF, Hinting::None).is_err());
    custom.insert_truetype('Z', TTF, Hinting::None).unwrap();
    let input = b"^XA^AZN,4097,20^FD!^FS^XZ";
    let error = render_with_fonts(input, SPECIFICATION, &custom).unwrap_err();
    assert_eq!(&input[error.offset..error.offset + 3], b"^AZ");
    assert!(error.message.contains("1..=4096"));
    let (s, mut glyphs) = bitmap(4);
    glyphs[0].bitmap.clear();
    assert!(custom.insert_bitmap('Z', s, glyphs, 7.).is_err());
    let (s, glyphs) = bitmap(4);
    assert!(custom.insert_bitmap('Z', s, glyphs, f64::NAN).is_err());
}

#[test]
fn symbol_registration_does_not_enable_filename_font_lookup() {
    let custom = fonts('@', 4);
    assert!(render_with_fonts(
        b"^XA^A@N,10,10,R:FONT.TTF^FDA^FS^XZ",
        SPECIFICATION,
        &custom
    )
    .is_err());
    assert!(render_with_fonts(b"^XA^CF@,10,10^FDA^FS^XZ", SPECIFICATION, &custom).is_err());
}
