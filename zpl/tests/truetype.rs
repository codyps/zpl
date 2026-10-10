//! Known-font and native-origin checks for the original TrueType engine.
//! Reference outlines were produced by FreeType 2.13.2 with native monochrome
//! hinting and pedantic error checks, using only our original constructed font.
//! https://freetype.org/freetype2/docs/reference/ft2-glyph_retrieval.html
//! https://learn.microsoft.com/en-us/typography/opentype/spec/tt_instructions
use serde_json::Value;
use std::collections::BTreeSet;
use zpl::{
    output::raster::truetype::{rasterize, ScanMode},
    truetype::{Environment, Font, Hinting, Size},
};

const DATA: &[u8] = include_bytes!("fixtures/truetype-regression/font-probes-20261002/probe.ttf");
const MANIFEST: &str =
    include_str!("fixtures/truetype-regression/font-probes-20261002/manifest.json");

#[test]
fn native_hint_outlines_and_advances_match_independent_reference() {
    check_independent_outlines(
        DATA,
        include_str!("fixtures/truetype-regression/font-probes-20261002/freetype-outlines.json"),
        59,
        464,
    );
}

#[test]
fn diagonal_freedom_vectors_match_independent_reference() {
    check_independent_outlines(
        include_bytes!("fixtures/truetype-regression/line-vector-20261005/probe.ttf"),
        include_str!("fixtures/truetype-regression/line-vector-20261005/freetype-outlines.json"),
        6,
        18,
    );
}

#[test]
fn shared_stem_counter_allocation_matches_independent_reference() {
    // Original connected-stem programs: equal CVT widths and integer counter
    // remainder allocation using GC, MUL, DIV, ROUND and SCFS. FreeType
    // references include odd counter totals and floor/ceil allocation policies.
    for (font, reference) in [
        (
            include_bytes!("fixtures/truetype-regression/balanced-hint-programs-20261005/grid.ttf")
                .as_slice(),
            include_str!(
                "fixtures/truetype-regression/balanced-hint-programs-20261005/grid-freetype.json"
            ),
        ),
        (
            include_bytes!(
                "fixtures/truetype-regression/balanced-hint-programs-20261005/floor.ttf"
            )
            .as_slice(),
            include_str!(
                "fixtures/truetype-regression/balanced-hint-programs-20261005/floor-freetype.json"
            ),
        ),
        (
            include_bytes!("fixtures/truetype-regression/balanced-hint-programs-20261005/ceil.ttf")
                .as_slice(),
            include_str!(
                "fixtures/truetype-regression/balanced-hint-programs-20261005/ceil-freetype.json"
            ),
        ),
    ] {
        check_independent_outlines(font, reference, 42, 5);
    }
}

#[test]
fn curve_first_grid_hints_preserve_independent_interpolation_residuals() {
    let mut reference: Value = serde_json::from_str(include_str!(
        "fixtures/truetype-regression/curve-first-experiment-20261005/freetype-outlines.json"
    ))
    .unwrap();
    // 62 of 64 nonempty glyph cases match FreeType 2.13.2 exactly. Pin the two
    // measured +1/64-pixel IUP Y residuals rather than claiming exact parity
    // or applying a blanket coordinate tolerance. Advances and all other
    // points, including fractional quadratic controls, must remain exact.
    let mut residuals = 0;
    for case in reference["cases"].as_array_mut().unwrap() {
        let query = (
            case["x"].as_u64(),
            case["y"].as_u64(),
            case["codepoint"].as_u64(),
        );
        let point = match query {
            (Some(11), Some(11), Some(83)) => Some(36),
            (Some(320), Some(448), Some(103)) => Some(27),
            _ => None,
        };
        if let Some(point) = point {
            let y = case["contours"][0][point][1].as_i64().unwrap();
            case["contours"][0][point][1] = (y + 1).into();
            residuals += 1;
        }
    }
    assert_eq!(residuals, 2);
    check_independent_outlines(
        include_bytes!("fixtures/truetype-regression/curve-grid-hints-20261005/curve.ttf"),
        &reference.to_string(),
        85,
        64,
    );
}

fn check_independent_outlines(data: &[u8], reference: &str, glyphs: u16, count: usize) {
    let reference: Value = serde_json::from_str(reference).unwrap();
    use sha2::{Digest, Sha256};
    assert_eq!(
        Sha256::digest(data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>(),
        reference["font_sha256"]
    );
    let font = Font::parse(data).unwrap();
    assert_eq!(font.glyph_count(), glyphs);
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), count);
    for case in cases {
        let size = Size::new(
            case["x"].as_u64().unwrap() as u16,
            case["y"].as_u64().unwrap() as u16,
        )
        .unwrap();
        let ch = char::from_u32(case["codepoint"].as_u64().unwrap() as u32).unwrap();
        let outline = font
            .instance(size, Hinting::Native, Environment::Standard)
            .unwrap()
            .glyph(ch)
            .unwrap();
        let actual = outline
            .contours
            .iter()
            .map(|c| {
                c.iter()
                    .map(|p| [p.x, p.y, i32::from(p.on_curve)])
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let expected: Vec<Vec<[i32; 3]>> =
            serde_json::from_value(case["contours"].clone()).unwrap();
        assert_eq!(actual, expected, "{size:?} {ch:?}");
        assert_eq!(
            outline.advance,
            case["advance"].as_i64().unwrap() as i32,
            "{size:?} {ch:?}"
        );
    }
}

#[test]
fn native_printer_probe_canvases_preserve_measured_residuals() {
    check_native_printer_probe_canvases(false);
}

#[test]
fn calibrated_printer_curve_and_rotation_residuals_are_pinned() {
    check_native_printer_probe_canvases(true);
}

fn check_native_printer_probe_canvases(calibrated: bool) {
    let manifest: Value = serde_json::from_str(MANIFEST).unwrap();
    let font = Font::parse(DATA).unwrap();
    let capture: Value = serde_json::from_str(include_str!(
        "fixtures/truetype-regression/font-probes-20261002/zd621/capture.json"
    ))
    .unwrap();
    // ZD621, 203 DPI, V93.21.33Z: full native canvases, shared experimental
    // scan policy. These residuals are not a font-parity acceptance threshold.
    let expected = [
        (0, 0, 164),
        (0, 0, 508),
        (0, 0, 1736),
        (0, 0, 432),
        (0, 0, 756),
        (0, 0, 416),
        (0, 0, 432),
        (0, 0, 416),
        (152, 68, 2444),
        (252, 134, 2515),
        (181, 97, 2506),
        (139, 89, 2439),
        (58, 19, 743),
        (134, 49, 863),
        (128, 128, 2784),
        (160, 144, 1654),
    ];
    let expected = if calibrated {
        [
            (0, 0, 164),
            (0, 0, 508),
            (0, 0, 1736),
            (0, 0, 432),
            (0, 0, 756),
            (0, 0, 416),
            (0, 0, 432),
            (0, 0, 416),
            (24, 0, 2376),
            (4, 0, 2381),
            (27, 0, 2409),
            (4, 2, 2352),
            (0, 0, 724),
            (0, 0, 814),
            (0, 0, 2656),
            (16, 1, 1511),
        ]
    } else {
        expected
    };
    for (index, page) in manifest["pages"]
        .as_array()
        .unwrap()
        .iter()
        .take(16)
        .enumerate()
    {
        let name = page["name"].as_str().unwrap();
        let filename = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/truetype-regression/font-probes-20261002/zd621")
            .join(format!("{name}.png"));
        let bytes = std::fs::read(filename).unwrap();
        use sha2::{Digest, Sha256};
        let record = capture["pages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == name)
            .unwrap();
        assert_eq!(
            Sha256::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
            record["png_sha256"]
        );
        let image = raster_diff::Raster::decode_png_with_threshold(&bytes, None).unwrap();
        assert_eq!(image.width, page["canvas"][0].as_u64().unwrap() as u32);
        assert_eq!(image.height, page["canvas"][1].as_u64().unwrap() as u32);
        let reference = image
            .pixels
            .iter()
            .enumerate()
            .filter(|(_, v)| **v == 0)
            .map(|(i, _)| {
                (
                    (i as u32 % image.width) as i32,
                    (i as u32 / image.width) as i32,
                )
            })
            .collect::<BTreeSet<_>>();
        let mut candidate = BTreeSet::new();
        for p in page["probes"].as_array().unwrap() {
            let h = p["height"].as_u64().unwrap() as u16;
            let w = p["width"].as_u64().unwrap() as u16;
            let w = if w == 0 { h } else { w };
            let turns = "NRIB".find(p["orientation"].as_str().unwrap()).unwrap() as u8;
            let ch = p["text"].as_str().unwrap().chars().next().unwrap();
            let instance = font
                .instance(
                    Size::new(w, h).unwrap(),
                    Hinting::Native,
                    if calibrated {
                        Environment::Zd621V93 {
                            quarter_turns: turns,
                        }
                    } else {
                        Environment::Zebra203 {
                            quarter_turns: turns,
                        }
                    },
                )
                .unwrap();
            let outline = instance.glyph(ch).unwrap();
            let glyph = rasterize(
                &outline,
                ch as u32,
                (outline.advance / 64) as u32,
                turns,
                if calibrated {
                    ScanMode::Zd621V93
                } else {
                    ScanMode::ZebraExperimental
                },
            )
            .unwrap();
            let x = p["tile"][0].as_i64().unwrap() as i32
                + p["anchor"][0].as_i64().unwrap() as i32
                + glyph.left;
            let y = p["tile"][1].as_i64().unwrap() as i32
                + p["anchor"][1].as_i64().unwrap() as i32
                + glyph.top;
            for (row, bits) in glyph.bitmap.iter().enumerate() {
                for col in 0..glyph.width as usize {
                    if bits[col / 8] & (128 >> (col % 8)) != 0 {
                        let point = (x + col as i32, y + row as i32);
                        assert!(
                            point.0 >= 0
                                && point.0 < image.width as i32
                                && point.1 >= 0
                                && point.1 < image.height as i32,
                            "no clipping permitted"
                        );
                        candidate.insert(point);
                    }
                }
            }
        }
        assert_eq!(
            (
                reference.difference(&candidate).count(),
                candidate.difference(&reference).count(),
                reference.union(&candidate).count()
            ),
            expected[index],
            "{name}"
        );
    }
}

#[test]
fn malformed_fonts_and_invalid_sizes_fail_without_panicking() {
    let tables = u16::from_be_bytes([DATA[4], DATA[5]]) as usize;
    let required = (0..tables)
        .map(|i| {
            let p = 12 + 16 * i;
            u32::from_be_bytes(DATA[p + 8..p + 12].try_into().unwrap()) as usize
                + u32::from_be_bytes(DATA[p + 12..p + 16].try_into().unwrap()) as usize
        })
        .max()
        .unwrap();
    for length in 0..required {
        assert!(
            Font::parse(&DATA[..length]).is_err(),
            "truncation at {length}"
        );
    }
    for size in [(0, 1), (1, 0), (4097, 32), (32, 65535)] {
        assert!(Size::new(size.0, size.1).is_err());
    }
    let font = Font::parse(DATA).unwrap();
    assert!(font.glyph_index('\u{10ffff}').is_none());
    assert!(font
        .instance(
            Size::new(32, 32).unwrap(),
            Hinting::Native,
            Environment::Standard
        )
        .unwrap()
        .outline(65535)
        .is_err());
}

// Replace the original font's empty space with a small, original composite.
// SFNT checksums are not interpreted by the reader; table offsets/lengths are.
fn font_with_composite(components: &[(u16, u16, i16, i16)]) -> Vec<u8> {
    use std::collections::BTreeMap;
    let count = u16::from_be_bytes(DATA[4..6].try_into().unwrap()) as usize;
    let mut tables = BTreeMap::new();
    for i in 0..count {
        let p = 12 + i * 16;
        let tag: [u8; 4] = DATA[p..p + 4].try_into().unwrap();
        let offset = u32::from_be_bytes(DATA[p + 8..p + 12].try_into().unwrap()) as usize;
        let len = u32::from_be_bytes(DATA[p + 12..p + 16].try_into().unwrap()) as usize;
        tables.insert(tag, DATA[offset..offset + len].to_vec());
    }
    let mut glyph = Vec::new();
    for value in [-1_i16, 397, 385, 525, 1409] {
        glyph.extend(value.to_be_bytes());
    }
    for (i, &(flags, id, x, y)) in components.iter().enumerate() {
        let flags = flags | if i + 1 < components.len() { 32 } else { 0 };
        for value in [flags, id, x as u16, y as u16] {
            glyph.extend(value.to_be_bytes());
        }
    }
    glyph.resize(glyph.len().next_multiple_of(4), 0);
    let shift = glyph.len() as u32;
    glyph.extend(tables[b"glyf"].iter());
    tables.insert(*b"glyf", glyph);
    for offset in tables.get_mut(b"loca").unwrap()[8..]
        .as_chunks_mut::<4>()
        .0
        .iter_mut()
    {
        let value = u32::from_be_bytes(*offset) + shift;
        offset.copy_from_slice(&value.to_be_bytes());
    }
    tables.get_mut(b"hmtx").unwrap()[6..8].copy_from_slice(&397_i16.to_be_bytes());
    let mut data = DATA[..12 + count * 16].to_vec();
    for (i, (tag, bytes)) in tables.into_iter().enumerate() {
        let p = 12 + i * 16;
        data[p..p + 4].copy_from_slice(&tag);
        let offset = data.len() as u32;
        data[p + 8..p + 12].copy_from_slice(&offset.to_be_bytes());
        data[p + 12..p + 16].copy_from_slice(&(bytes.len() as u32).to_be_bytes());
        data.extend(bytes);
        data.resize(data.len().next_multiple_of(4), 0);
    }
    data
}

#[test]
fn composite_offsets_round_both_axes_only_when_requested() {
    // OpenType glyf, "Component Glyph flags" and offset-vector paragraph:
    // https://learn.microsoft.com/en-us/typography/opentype/spec/glyf
    // At 16 ppem / 2048 UPEM these offsets are 70.5 and 192.5 in 26.6.
    // Native ROUND_XY_TO_GRID rounds both to grid lines, not just Y.
    for (flags, hinting, x, y) in [
        (3, Hinting::Native, 199, 193),
        (7, Hinting::Native, 192, 192),
        (7, Hinting::None, 199, 193),
        (515, Hinting::Native, 199, 193),
    ] {
        let data = font_with_composite(&[(flags, 3, 141, 385)]);
        let font = Font::parse(&data).unwrap();
        let instance = font
            .instance(Size::new(16, 16).unwrap(), hinting, Environment::Standard)
            .unwrap();
        let outline = instance.glyph(' ').unwrap();
        assert_eq!((outline.contours[0][0].x, outline.contours[0][0].y), (x, y));
        assert_eq!(outline.advance, if flags & 512 == 0 { 256 } else { 512 });
        assert_eq!(outline.linear_advance, outline.advance);
    }
}

#[test]
fn composite_cycles_and_expansion_fail_with_diagnostics() {
    for (parts, message) in [
        (vec![(3, 1, 0, 0)], "recursive"),
        (vec![(3, 0, 0, 0); 128], "expansion limit"),
        (vec![(3, 65535, 0, 0)], "glyph index"),
    ] {
        let data = font_with_composite(&parts);
        let font = Font::parse(&data).unwrap();
        let instance = font
            .instance(
                Size::new(16, 16).unwrap(),
                Hinting::Native,
                Environment::Standard,
            )
            .unwrap();
        assert!(instance.glyph(' ').unwrap_err().0.contains(message));
    }
}

#[test]
fn component_phantom_origin_is_applied_once_at_the_parent() {
    // glyf / hmtx: the component placement operates on outline coordinates;
    // horizontal phantom points determine the final baseline origin. USE_MY_METRICS
    // inherits the component's metrics, including its left phantom point.
    for (flags, expected_x) in [(3, 397), (515, 525)] {
        let mut data = font_with_composite(&[(flags, 3, 141, 385)]);
        let count = u16::from_be_bytes(data[4..6].try_into().unwrap()) as usize;
        for i in 0..count {
            let p = 12 + 16 * i;
            let offset = u32::from_be_bytes(data[p + 8..p + 12].try_into().unwrap()) as usize;
            if &data[p..p + 4] == b"hmtx" {
                data[offset + 14..offset + 16].copy_from_slice(&384_i16.to_be_bytes());
            } else if &data[p..p + 4] == b"head" {
                data[offset + 17] &= !2; // xMin no longer equals every lsb.
            }
        }
        let font = Font::parse(&data).unwrap();
        let instance = font
            .instance(
                Size::new(32, 32).unwrap(),
                Hinting::Native,
                Environment::Standard,
            )
            .unwrap();
        assert_eq!(instance.glyph('"').unwrap().contours[0][0].x, 384);
        assert_eq!(instance.glyph(' ').unwrap().contours[0][0].x, expected_x);
    }
}

#[test]
fn zd621_point_quantization_rounding_and_spacing_match_native_canvases() {
    use sha2::{Digest, Sha256};
    let hash = |bytes: &[u8]| {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    };
    // Constructed witnesses independently expose RCVT/GC fractional scale,
    // ROUND thresholds and hmtx spacing. 65..128 was reserved until the shared
    // rule was frozen from 1..64. No resampling, registration or ignored pixels.
    // ZD621_203_DPI provenance; this low-level API selects the font environment
    // directly rather than going through ZPL's render::Options profile.
    let mut pages = 0;
    let mut fields = 0;
    for directory in [
        "rounding-font-20261003",
        "scaling-font-20261003",
        "scaling-font-validation-20261003",
        "cvt-axis-20261003",
        "cvt-axis-validation-v2-20261003",
        "font0-hints-20261003",
        "reconstruction-replay-20261003",
        "joint-replay-20261003",
        "expanded-replay-20261004",
        "accuracy-feature-replay-20261004",
        "structured-baseline-replay-20261004",
        "structured-candidate-replay-20261004",
        "ascii-cmap-20261004",
        "target-training-replay-20261004",
        "ascii-baseline-replay-20261004",
        "ascii-candidate-replay-20261004",
        "line-vector-20261005",
        "repair-baseline-replay-20261005",
        "repair-targeted-replay-20261005",
        "curve-first-experiment-20261005/baseline-replay",
        "curve-first-experiment-20261005/curve-replay",
    ] {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/truetype-regression")
            .join(directory);
        let data = std::fs::read(root.join("probe.ttf")).unwrap();
        let manifest_bytes = std::fs::read(root.join("manifest.json")).unwrap();
        let manifest: Value = serde_json::from_slice(&manifest_bytes).unwrap();
        let capture: Value =
            serde_json::from_slice(&std::fs::read(root.join("zd621/capture.json")).unwrap())
                .unwrap();
        assert_eq!(capture["status"], "complete");
        assert_eq!(capture["resident_repeat_exact"], true);
        assert_eq!(capture["cleanup"]["confirmed_absent"], true);
        assert_eq!(capture["printer"]["model"], "ZD621");
        assert_eq!(capture["printer"]["firmware"], "V93.21.33Z");
        assert_eq!(
            capture["printer"]["dpi"],
            zpl::render::profiles::ZD621_203_DPI.dpi
        );
        assert_eq!(hash(&data), manifest["font_sha256"]);
        assert_eq!(hash(&manifest_bytes), capture["manifest_sha256"]);
        let font = Font::parse(&data).unwrap();
        for (page_index, page) in manifest["pages"].as_array().unwrap().iter().enumerate() {
            pages += 1;
            let name = page["name"].as_str().unwrap();
            let record = capture["pages"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["name"] == name)
                .unwrap();
            let bytes = std::fs::read(root.join("zd621").join(format!("{name}.png"))).unwrap();
            assert_eq!(hash(&bytes), record["png_sha256"]);
            let request = std::fs::read(root.join("zd621").join(format!("{name}.zpl"))).unwrap();
            assert_eq!(hash(&request), page["zpl_sha256"]);
            assert_eq!(page["zpl_sha256"], record["zpl_sha256"]);
            let image = raster_diff::Raster::decode_png_with_threshold(&bytes, None).unwrap();
            assert_eq!(image.width as u64, page["canvas"][0].as_u64().unwrap());
            assert_eq!(image.height as u64, page["canvas"][1].as_u64().unwrap());
            let reference = image
                .pixels
                .iter()
                .enumerate()
                .filter(|(_, p)| **p == 0)
                .map(|(i, _)| {
                    (
                        (i as u32 % image.width) as i32,
                        (i as u32 / image.width) as i32,
                    )
                })
                .collect::<BTreeSet<_>>();
            let mut candidate = BTreeSet::new();
            for p in page["probes"].as_array().unwrap() {
                fields += 1;
                let turns = "NRIB".find(p["orientation"].as_str().unwrap()).unwrap() as u8;
                let size = Size::new(
                    p["width"].as_u64().unwrap() as u16,
                    p["height"].as_u64().unwrap() as u16,
                )
                .unwrap();
                let instance = font
                    .instance(
                        size,
                        Hinting::Native,
                        Environment::Zd621V93 {
                            quarter_turns: turns,
                        },
                    )
                    .unwrap();
                let tx = p["tile"][0].as_i64().unwrap() as i32;
                let ty = p["tile"][1].as_i64().unwrap() as i32;
                let tw = p["tile"][2].as_i64().unwrap() as i32;
                let th = p["tile"][3].as_i64().unwrap() as i32;
                let x = tx + p["anchor"][0].as_i64().unwrap() as i32;
                let y = ty + p["anchor"][1].as_i64().unwrap() as i32;
                let mut pen = 0;
                for ch in p["text"].as_str().unwrap().chars() {
                    let advance = instance
                        .layout_advance(font.glyph_index(ch).unwrap())
                        .unwrap();
                    let glyph = rasterize(
                        &instance.glyph(ch).unwrap(),
                        ch as u32,
                        advance,
                        turns,
                        ScanMode::Zd621V93,
                    )
                    .unwrap();
                    for (row, bits) in glyph.bitmap.iter().enumerate() {
                        for col in 0..glyph.width as usize {
                            if bits[col / 8] & (128 >> (col % 8)) != 0 {
                                let (dx, dy) =
                                    [(pen, 0), (0, pen), (-pen, 0), (0, -pen)][turns as usize];
                                let point = (
                                    x + dx + glyph.left + col as i32,
                                    y + dy + glyph.top + row as i32,
                                );
                                assert!(
                                    point.0 >= tx
                                        && point.0 < tx + tw
                                        && point.1 >= ty
                                        && point.1 < ty + th,
                                    "{directory}/{name}: clipping"
                                );
                                candidate.insert(point);
                            }
                        }
                    }
                    pen += advance as i32;
                }
            }
            let actual = (
                reference.difference(&candidate).count(),
                candidate.difference(&reference).count(),
                reference.union(&candidate).count(),
            );
            if directory == "font0-hints-20261003" {
                // Same generated font, local versus printer. Keep residuals
                // distinct from the larger error against resident Font 0.
                let expected = [
                    (0, 0, 52),
                    (5, 0, 4930),
                    (3, 0, 2552),
                    (0, 0, 11786),
                    (0, 0, 3901),
                ];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else if directory == "reconstruction-replay-20261003" {
                // Automatically fitted quadratic geometry and derived hints;
                // same generated TTF on both sides, native unaligned canvases.
                let expected = [(0, 0, 52), (1, 0, 20591), (1, 0, 974)];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else if directory == "joint-replay-20261003" {
                // Frozen joint-search proposal, retained despite rejection by
                // reconstruction acceptance. This checks execution of that TTF,
                // not its resemblance to resident Font 0 or production fitness.
                assert_eq!(manifest["variant"], "proposal");
                let expected = [(0, 0, 52), (12, 9, 18658), (0, 0, 1953)];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else if directory == "expanded-replay-20261004" {
                // Same generated font on 294 independently reserved square,
                // stretched and rotated cases after expanding development data.
                assert_eq!(manifest["variant"], "proposal");
                let expected = [
                    (0, 0, 52),
                    (43, 42, 8273),
                    (64, 58, 31630),
                    (16, 16, 6353),
                    (0, 0, 43314),
                    (66, 73, 33905),
                    (23, 23, 28001),
                    (0, 0, 41177),
                    (0, 0, 47900),
                    (0, 0, 20264),
                ];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else if directory == "accuracy-feature-replay-20261004" {
                // Additive local-curve feature experiment, frozen before these
                // 96 Font 0 targets were captured. This measures execution of
                // the same TTF, separately from its reconstruction accuracy.
                let expected = [(0, 0, 52), (0, 1, 6772), (4, 6, 22165)];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else if directory == "structured-baseline-replay-20261004" {
                // Paired baseline on 240 fresh dimensions/rotations near the
                // coarse hint branches. Keep every native pixel accounted for.
                let expected = [
                    (0, 0, 52),
                    (53, 56, 9550),
                    (34, 38, 11406),
                    (43, 40, 14707),
                    (36, 47, 6622),
                ];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else if directory == "structured-candidate-replay-20261004" {
                // Relative counter links, IP anchor interpolation and MPPEM
                // branches execute on the printer; Font 0 fit is scored apart.
                let expected = [
                    (0, 0, 52),
                    (52, 55, 9574),
                    (34, 38, 11416),
                    (42, 42, 14721),
                    (36, 41, 6635),
                ];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else if directory == "ascii-cmap-20261004" {
                // Distinct shapes for every visible ASCII codepoint, actual
                // space advances, and private-use identity/instruction markers.
                // Native printer canvases; this is same-font execution evidence.
                let expected = [(0, 0, 52), (0, 0, 3476), (0, 0, 1254)];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else if directory == "target-training-replay-20261004" {
                // Fractional placement, optical programs, and disconnected
                // feature controls on known training cases. This diagnoses
                // execution; it is not an independent reconstruction holdout.
                let expected = [(0, 0, 52), (16, 8, 3437), (15, 11, 6014), (24, 1, 4632)];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else if directory == "ascii-baseline-replay-20261004" {
                // Full ASCII, frozen before the 1,504 small/transformed
                // resident holdouts. Same-font residuals remain independent
                // of Font 0 fitting scores and preserve every native pixel.
                let expected = [
                    (0, 0, 52),
                    (7, 3, 22140),
                    (4, 4, 16340),
                    (8, 0, 14687),
                    (2, 4, 7879),
                    (9, 2, 10295),
                    (5, 3, 14319),
                    (12, 13, 17357),
                    (7, 4, 14112),
                    (6, 3, 14364),
                    (17, 13, 15882),
                    (21, 17, 16533),
                    (4, 3, 16175),
                    (3, 0, 16891),
                    (5, 1, 8699),
                    (10, 3, 9492),
                    (21, 14, 21135),
                    (14, 12, 12009),
                    (8, 2, 8067),
                    (7, 6, 5483),
                    (17, 9, 8626),
                    (5, 2, 8798),
                    (8, 6, 13775),
                    (9, 9, 10822),
                    (1, 0, 4619),
                ];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else if directory == "ascii-candidate-replay-20261004" {
                // Full ASCII, frozen before the 1,504 small/transformed
                // resident holdouts. Same-font residuals remain independent
                // of Font 0 fitting scores and preserve every native pixel.
                let expected = [
                    (0, 0, 52),
                    (42, 67, 22240),
                    (77, 86, 16497),
                    (74, 32, 14844),
                    (43, 22, 7890),
                    (61, 51, 10333),
                    (47, 41, 14388),
                    (49, 53, 17454),
                    (33, 54, 14268),
                    (38, 62, 14512),
                    (33, 24, 15989),
                    (58, 82, 16618),
                    (76, 89, 16303),
                    (35, 22, 16949),
                    (70, 33, 8826),
                    (36, 38, 9561),
                    (32, 47, 21343),
                    (30, 36, 12063),
                    (69, 45, 8096),
                    (29, 14, 5503),
                    (49, 25, 8670),
                    (73, 45, 8924),
                    (37, 16, 13821),
                    (40, 32, 10916),
                    (65, 30, 4734),
                ];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else if directory == "line-vector-20261005" {
                let expected = [
                    (0, 0, 52),
                    (0, 0, 45),
                    (0, 0, 114),
                    (0, 0, 217),
                    (0, 0, 483),
                    (0, 0, 166),
                    (0, 0, 162),
                    (0, 0, 166),
                    (0, 0, 166),
                    (0, 0, 166),
                ];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else if directory == "repair-baseline-replay-20261005" {
                let expected = [
                    (0, 0, 52),
                    (12, 8, 4252),
                    (16, 11, 10908),
                    (9, 4, 8847),
                    (57, 42, 10183),
                    (60, 41, 7603),
                    (13, 16, 3224),
                ];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else if directory == "repair-targeted-replay-20261005" {
                let expected = [
                    (0, 0, 52),
                    (16, 10, 4243),
                    (24, 13, 10929),
                    (9, 4, 8850),
                    (57, 43, 10175),
                    (62, 42, 7610),
                    (15, 17, 3226),
                ];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else if directory == "curve-first-experiment-20261005/baseline-replay" {
                let expected = [
                    (0, 0, 52),
                    (0, 0, 23357),
                    (0, 0, 17691),
                    (0, 0, 27687),
                    (0, 0, 24936),
                    (0, 0, 70409),
                    (0, 0, 53090),
                    (0, 0, 82907),
                    (0, 0, 74754),
                    (0, 0, 95534),
                    (0, 0, 72399),
                    (0, 0, 113085),
                    (0, 0, 101717),
                    (0, 0, 57116),
                    (0, 0, 43627),
                    (0, 0, 68453),
                    (0, 0, 61320),
                    (0, 0, 57613),
                    (0, 0, 43517),
                    (0, 0, 68019),
                    (0, 0, 61249),
                    (16, 12, 5257),
                ];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else if directory == "curve-first-experiment-20261005/curve-replay" {
                // Compact curves and integer stem/counter chains. These are
                // same-font interpreter/raster residuals, not Font 0 IoUs.
                let expected = [
                    (0, 0, 52),
                    (0, 0, 23286),
                    (0, 0, 17632),
                    (0, 0, 27855),
                    (0, 0, 24863),
                    (0, 0, 69230),
                    (0, 0, 53320),
                    (0, 0, 82725),
                    (0, 0, 74809),
                    (0, 0, 94623),
                    (0, 0, 72250),
                    (0, 0, 113056),
                    (0, 0, 101699),
                    (10, 4, 57144),
                    (15, 15, 43381),
                    (34, 8, 67915),
                    (9, 33, 61447),
                    (1, 2, 56805),
                    (8, 15, 43614),
                    (7, 14, 67986),
                    (13, 15, 61301),
                    (5, 2, 5376),
                ];
                assert_eq!(actual, expected[page_index], "{directory}/{name}");
            } else {
                assert_eq!((actual.0, actual.1), (0, 0), "{directory}/{name}");
            }
        }
    }
    assert_eq!(pages, 187);
    assert_eq!(fields, 7818);
}
