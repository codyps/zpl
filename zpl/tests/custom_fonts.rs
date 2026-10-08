//! API regressions for supplied fonts. ZPL font selection and field placement:
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
//! ^A, ^CF, ^FB, ^FO, ^FT. These synthetic fonts assert local behavior, not printer parity.
#[path = "support/compact_font.rs"]
mod compact_font;
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
fn captured_bitmap_and_bounded_text_use_custom_metrics() {
    let mut captured = Fonts::new();
    let (settings, glyphs) = compact_font::decoded("font0-32.zbf").unwrap();
    captured.insert_bitmap('0', settings, glyphs, 24.).unwrap();
    let source = b"^XA^FO30,30^A0N,32,32^FDABC^FS^XZ";
    let resident = zpl::render(source, SPECIFICATION).unwrap();
    let custom = render_with_fonts(source, SPECIFICATION, &captured).unwrap();
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
fn symbol_registration_is_not_a_named_font() {
    let custom = fonts('@', 4);
    assert!(render_with_fonts(
        b"^XA^A@N,10,10,R:FONT.TTF^FDA^FS^XZ",
        SPECIFICATION,
        &custom
    )
    .is_err());
    assert!(render_with_fonts(b"^XA^CF@,10,10^FDA^FS^XZ", SPECIFICATION, &custom).is_err());
}

fn named_fonts() -> Fonts<'static> {
    let mut custom = fonts('Z', 4);
    let (settings, glyphs) = bitmap(4);
    custom
        .insert_named_bitmap("R:brand.fnt", settings, glyphs, 7.)
        .unwrap();
    let (settings, glyphs) = bitmap(8);
    custom
        .insert_named_bitmap("E:brand.fnt", settings, glyphs, 7.)
        .unwrap();
    custom
}

#[test]
fn named_bitmap_alias_and_direct_selection_match_registered_ids() {
    // ^CW p.168 and ^A@ p.62: aliases and device-qualified names select a face.
    let custom = named_fonts();
    let inline = "^XA^PW100^LL100^FO40,40^AZN,10,10^FDAA^FS^XZ";
    let expected = image(inline, &custom);
    for selection in [
        "^A@N,10,10,R:BRAND.FNT",
        "^A@N,10,10,brand.fnt",
        "^CWQ,R:BRAND.FNT^AQN,10,10",
        "^CWQ,R:BRAND.FNT^CFQ,10,10",
    ] {
        assert!(
            expected == image(&inline.replace("^AZN,10,10", selection), &custom),
            "{selection}"
        );
    }
    for rotation in ['N', 'R', 'I', 'B'] {
        let selected = inline.replace("AZN", &format!("AZ{rotation}"));
        let direct = selected.replace(
            &format!("^AZ{rotation},10,10"),
            &format!("^A@{rotation},10,10,R:BRAND.FNT"),
        );
        assert!(
            image(&selected, &custom) == image(&direct, &custom),
            "{rotation}"
        );
    }
    let disk = inline.replace("^AZN,10,10", "^A@N,10,10,E:BRAND.FNT");
    assert!(image(&disk, &custom) == image(inline, &fonts('Z', 8)));
    assert!(image(&disk, &custom) != expected);
}

#[test]
fn named_selections_persist_within_jobs_and_leave_the_caller_unchanged() {
    let custom = named_fonts();
    let one = "^XA^PW100^LL100^FO10,10^AZN,10,10^FDAA^FS^XZ";
    let input = "^CWQ,R:BRAND.FNT^XA^PW100^LL100^FO10,10^AQN,10,10^FDAA^FS^XZ^XA^FO10,10^AQN,10,10^FDAA^FS^XZ";
    let doc = render_with_fonts(input.as_bytes(), SPECIFICATION, &custom).unwrap();
    assert_eq!(doc.labels.len(), 2);
    for label in &doc.labels {
        assert!(zpl::output::raster::rasterize(label).unwrap() == image(one, &custom));
    }
    // Rebinding an alias is request-local and cannot rewrite direct ^A@ selection.
    let input = "^XA^PW100^LL100^FO10,10^A@N,10,10,R:BRAND.FNT^FDAA^FS^CWQ,E:BRAND.FNT^FO10,20^A@N,10,10^FDAA^FS^XZ";
    let expected = "^XA^PW100^LL100^FO10,10^AZN,10,10^FDAA^FS^FO10,20^AZN,10,10^FDAA^FS^XZ";
    assert!(image(input, &custom) == image(expected, &custom));
    let aliased = "^CWZ,E:BRAND.FNT^XA^PW100^LL100^FO10,10^AZN,10,10^FDAA^FS^XZ";
    assert!(image(aliased, &custom) == image(one, &fonts('Z', 8)));
    assert!(image(one, &custom) == image(one, &fonts('Z', 4)));
    // ^A@ without any previous name uses ^CF, not another call's selected face.
    let unnamed = "^XA^CFZ,10,10^PW100^LL100^FO10,10^A@N,10,10^FDAA^FS^XZ";
    assert!(image(unnamed, &custom) == image(one, &custom));
}

#[test]
fn named_truetype_and_bitmap_registration_feed_the_existing_font_engines() {
    let mut custom = Fonts::new();
    custom.insert_truetype('Z', TTF, Hinting::Native).unwrap();
    custom
        .insert_named_truetype("probe.ttf", TTF, Hinting::Native)
        .unwrap();
    let source = "^XA^PW150^LL150^FO50,50^AZN,24,31^FD!!^FS^XZ";
    assert!(
        image(source, &custom)
            == image(
                &source.replace("^AZN,24,31", "^A@N,24,31,PROBE.TTF"),
                &custom
            )
    );
    let (settings, glyphs) = compact_font::decoded("font0-32.zbf").unwrap();
    custom
        .insert_named_bitmap("CAPTURE.FNT", settings, glyphs.clone(), 24.)
        .unwrap();
    custom.insert_bitmap('Z', settings, glyphs, 24.).unwrap();
    let source = "^XA^PW150^LL150^FO20,20^AZN,32,32^FDABC^FS^XZ";
    assert!(
        image(source, &custom)
            == image(
                &source.replace("^AZN,32,32", "^CWZ,CAPTURE.FNT^AZN,32,32"),
                &custom
            )
    );
}

#[test]
fn named_fonts_work_in_stored_formats_captions_and_changed_syntax() {
    let custom = named_fonts();
    let inline = "^XA^PW100^LL100^FO10,10^AZN,10,10^FDAA^FS^XZ";
    let stored = "^XA^DFR:LABEL.ZPL^FO10,10^A@N,10,10,R:BRAND.FNT^FDAA^FS^XZ^XA^PW100^LL100^XFR:LABEL.ZPL^XZ";
    assert!(image(inline, &custom) == image(stored, &custom));
    let changed = "^CD;^CWQ;R:BRAND.FNT^CC!!XA!PW100!LL100!FO10;10!AQN;10;10!FDAA!FS!XZ";
    assert!(image(inline, &custom) == image(changed, &custom));
    let changed = "^CD;^CC!!XA!PW100!LL100!FO10;10!A@N;10;10;R:BRAND.FNT!FDAA!FS!XZ";
    assert!(image(inline, &custom) == image(changed, &custom));
    let caption = "^XA^PW200^LL100^FO10,10^AZN,10,10^BCN,30,Y,N,N^FDAA^FS^XZ";
    assert!(
        image(caption, &custom)
            == image(
                &caption.replace("^AZN,10,10", "^A@N,10,10,R:BRAND.FNT"),
                &custom
            )
    );
}

#[test]
fn invalid_and_unresolved_names_keep_offsets_and_do_not_touch_registration() {
    let mut custom = named_fonts();
    for name in [
        "",
        "../font.ttf",
        "/tmp/font.ttf",
        "C:FONT.TTF",
        "R:FONT",
        "R:FONT.WOFF",
        "R:FO/NT.TTF",
    ] {
        assert!(
            custom
                .insert_named_truetype(name, TTF, Hinting::None)
                .is_err(),
            "{name}"
        );
    }
    assert!(custom
        .insert_named_truetype("R:BRAND.FNT", b"bad", Hinting::None)
        .is_err());
    for (command, expected) in [
        ("^CWQ,R:MISSING.TTF", "unresolved named font"),
        ("^A@N,10,10,R:MISSING.TTF", "unresolved named font"),
        ("^CW@,R:BRAND.FNT", "font ID"),
        ("^CWZZ,R:BRAND.FNT", "single font ID"),
        ("^CWQ", "requires a font filename"),
        ("^CWQ,R:BRAND.FNT,extra", "unexpected command parameters"),
        (
            "^A@N,10,10,R:BRAND.FNT,extra",
            "unexpected command parameters",
        ),
    ] {
        let input = format!("^XA{command}^XZ");
        let error = render_with_fonts(input.as_bytes(), SPECIFICATION, &custom).unwrap_err();
        assert_eq!(error.offset, 3, "{command}");
        assert!(error.message.contains(expected), "{error}");
    }
    let before = "^XA^PW100^LL100^FO10,10^AZN,10,10^FDAA^FS^XZ";
    let named = before.replace("^AZN,10,10", "^A@N,10,10,R:BRAND.FNT");
    assert!(image(before, &custom) == image(&named, &custom));
}

#[test]
fn overridden_resident_ids_remap_to_custom_unicode_glyphs() {
    // ^CI image 66 (B) -> source 65 (A). Compact resident source-slot tags
    // must not reach the Unicode glyph lookup of a caller-supplied face.
    let source = "^XA^PW100^LL100^CI0,65,66^FO10,10^AAN,10,10^FDB^FS^XZ";
    let expected = "^XA^PW100^LL100^FO10,10^AZN,10,10^FDA^FS^XZ";
    assert_eq!(
        image(source, &fonts('A', 4)),
        image(expected, &fonts('Z', 4))
    );
}

// A caller-owned strike with no ZBF settings, resident tag, or DPI metadata.
struct SuppliedBitmap<'a> {
    metrics: zpl::fonts::BitmapMetrics,
    glyphs: &'a [Glyph],
    owned: bool,
}

impl zpl::fonts::BitmapFont for SuppliedBitmap<'_> {
    fn metrics(&self) -> zpl::fonts::BitmapMetrics {
        self.metrics
    }

    fn glyph(&self, c: char) -> Result<Option<std::borrow::Cow<'_, Glyph>>, String> {
        Ok(self
            .glyphs
            .iter()
            .find(|g| g.codepoint == c as u32)
            .map(|g| {
                if self.owned {
                    std::borrow::Cow::Owned(g.clone())
                } else {
                    std::borrow::Cow::Borrowed(g)
                }
            }))
    }
}

fn supplied(glyphs: &[Glyph], owned: bool) -> SuppliedBitmap<'_> {
    SuppliedBitmap {
        metrics: zpl::fonts::BitmapMetrics {
            width: 10,
            height: 10,
            baseline: 7.,
        },
        glyphs,
        owned,
    }
}

#[test]
fn bitmap_provider_matches_decoded_strike_for_layout_and_named_selection() {
    let (_, glyphs) = bitmap(4);
    let legacy = fonts('Z', 4);
    for owned in [false, true] {
        let provider = std::sync::Arc::new(supplied(&glyphs, owned));
        let mut custom = Fonts::new();
        custom.insert_bitmap_font('Z', provider.clone()).unwrap();
        custom
            .insert_named_bitmap_font("brand.fnt", provider)
            .unwrap();
        let cloned = custom.clone();
        drop(custom);
        for orientation in ['N', 'R', 'I', 'B'] {
            for dimensions in ["0,0", "20,0", "0,30", "20,30"] {
                for origin in ["^FO40,40", "^FT40,40"] {
                    for block in ["", "^FB40,3,0,L,0"] {
                        let source = format!(
                            "^XA^PW100^LL100{origin}^AZ{orientation},{dimensions}{block}^FDA A^FS^XZ"
                        );
                        let expected = image(&source, &legacy);
                        assert_eq!(image(&source, &cloned), expected);
                        assert_eq!(
                            image(&format!("^CWZ,R:BRAND.FNT{source}"), &cloned),
                            expected
                        );
                        assert_eq!(
                            image(
                                &source.replace(
                                    &format!("^AZ{orientation},{dimensions}"),
                                    &format!("^A@{orientation},{dimensions},R:BRAND.FNT")
                                ),
                                &cloned
                            ),
                            expected
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn bitmap_provider_validates_metrics_before_replacing_a_face() {
    let (_, glyphs) = bitmap(4);
    let source = "^XA^PW100^LL100^FO10,10^AZN,10,10^FDA^FS^XZ";
    let mut custom = fonts('Z', 4);
    let expected = image(source, &custom);
    for (width, height, baseline) in [
        (0, 10, 7.),
        (10, 0, 0.),
        (4097, 10, 7.),
        (10, 4097, 7.),
        (10, 10, -1.),
        (10, 10, 11.),
        (10, 10, f64::NAN),
        (10, 10, f64::INFINITY),
    ] {
        let mut provider = supplied(&glyphs, false);
        provider.metrics = zpl::fonts::BitmapMetrics {
            width,
            height,
            baseline,
        };
        assert!(custom
            .insert_bitmap_font('Z', std::sync::Arc::new(provider))
            .is_err());
        assert_eq!(image(source, &custom), expected);
    }
    // Provider cell dimensions are not subject to the legacy capture limit of 128.
    let mut provider = supplied(&glyphs, false);
    provider.metrics.width = 200;
    provider.metrics.height = 200;
    custom
        .insert_bitmap_font('Z', std::sync::Arc::new(provider))
        .unwrap();
    assert_eq!(
        image(&source.replace("10,10^FD", "200,200^FD"), &custom),
        expected
    );
}

#[test]
fn bitmap_provider_rejects_bad_glyphs_and_propagates_lookup_errors() {
    struct Provider(Option<Glyph>);
    impl zpl::fonts::BitmapFont for Provider {
        fn metrics(&self) -> zpl::fonts::BitmapMetrics {
            supplied(&[], false).metrics
        }
        fn glyph(&self, c: char) -> Result<Option<std::borrow::Cow<'_, Glyph>>, String> {
            if c == 'B' {
                return Err("caller bitmap decoder failed".into());
            }
            Ok(self.0.as_ref().map(std::borrow::Cow::Borrowed))
        }
    }
    let source = b"^XA^AZN,10,10^FDA^FS^XZ";
    let (_, glyphs) = bitmap(4);
    let mut malformed = glyphs[0].clone();
    malformed.bitmap.pop();
    let mut wrong_codepoint = glyphs[0].clone();
    wrong_codepoint.codepoint = 'C' as u32;
    for (glyph, expected) in [
        (None, "unsupported custom font glyph 'A'"),
        (Some(malformed), "invalid glyph metrics or bitmap"),
        (
            Some(wrong_codepoint),
            "bitmap font returned a different codepoint",
        ),
    ] {
        let mut custom = Fonts::new();
        custom
            .insert_bitmap_font('Z', std::sync::Arc::new(Provider(glyph)))
            .unwrap();
        let error = render_with_fonts(source, SPECIFICATION, &custom).unwrap_err();
        assert!(error.message.contains(expected), "{error:?}");
        let error =
            render_with_fonts(b"^XA^AZN,10,10^FDB^FS^XZ", SPECIFICATION, &custom).unwrap_err();
        assert!(
            error.message.contains("caller bitmap decoder failed"),
            "{error:?}"
        );
    }
}
