//! Synthetic provider parity, not printer evidence. Contracts are defined by
//! zpl::fonts::BitmapFont and zpl::fonts::Fonts; no independent font engine.
use super::*;
use std::{
    borrow::Cow,
    ffi::c_void,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};
use zpl::fonts::{BitmapFont, BitmapMetrics, Fonts, Glyph};

struct Strike {
    calls: AtomicUsize,
    mode: u8,
}
impl BitmapFont for Strike {
    fn metrics(&self) -> BitmapMetrics {
        BitmapMetrics {
            width: 8,
            height: 12,
            baseline: 9.0,
        }
    }
    fn glyph(&self, c: char) -> std::result::Result<Option<Cow<'_, Glyph>>, String> {
        Ok((c == 'A').then(|| {
            Cow::Owned(Glyph {
                codepoint: c as u32,
                advance: 8,
                left: 0,
                top: -3,
                width: 3,
                height: 3,
                bitmap: vec![vec![0x40], vec![0xa0], vec![0xe0]],
            })
        }))
    }
}
unsafe extern "C" fn lookup(
    context: *mut c_void,
    c: u32,
    out: *mut ZplGlyph,
    error: *mut ZplBytes,
) -> i32 {
    let strike = unsafe { &*context.cast::<Strike>() };
    strike.calls.fetch_add(1, Ordering::Relaxed);
    if strike.mode == 1 {
        unsafe { *error = ZplBytes::new(b"synthetic provider failure") };
        return 2;
    }
    if c != u32::from('A') {
        return 1;
    }
    unsafe {
        *out = ZplGlyph {
            advance: 8,
            left: 0,
            top: -3,
            width: 3,
            height: 3,
            bitmap: ZplBytes::new(if strike.mode == 2 {
                &[0x40]
            } else {
                &[0x40, 0xa0, 0xe0]
            }),
        }
    };
    0
}
fn provider(strike: &Strike) -> ZplBitmapProvider {
    ZplBitmapProvider {
        metrics: ZplBitmapMetrics {
            width: 8,
            height: 12,
            baseline: 9.,
        },
        user_data: (strike as *const Strike).cast_mut().cast(),
        glyph: Some(lookup),
    }
}
unsafe fn render_c(input: &[u8], options: &ZplOptions, fonts: *const ZplFonts) -> *mut ZplDocument {
    let mut doc = ptr::null_mut();
    assert_eq!(
        unsafe {
            zpl_render_with_fonts(
                input.as_ptr(),
                input.len(),
                options,
                fonts,
                ptr::null(),
                &mut doc,
            )
        },
        ZPL_OK
    );
    doc
}
#[test]
fn bitmap_callback_matches_native_across_profiles_and_selection() {
    unsafe {
        let strike = Strike {
            calls: AtomicUsize::new(0),
            mode: 0,
        };
        let provider = provider(&strike);
        let mut collection = ptr::null_mut();
        assert_eq!(zpl_fonts_new(&mut collection), ZPL_OK);
        assert_eq!(
            zpl_fonts_insert_bitmap(collection, 'Z' as u32, &provider),
            ZPL_OK
        );
        let name = b"R:PROBE.FNT";
        assert_eq!(
            zpl_fonts_insert_named_bitmap(collection, name.as_ptr(), name.len(), &provider),
            ZPL_OK
        );
        let mut native = Fonts::new();
        let reference = Arc::new(Strike {
            calls: AtomicUsize::new(0),
            mode: 0,
        });
        native.insert_bitmap_font('Z', reference.clone()).unwrap();
        native
            .insert_named_bitmap_font("R:PROBE.FNT", reference)
            .unwrap();
        for profile in [
            render::profiles::SPECIFICATION,
            render::profiles::ZD621_203_DPI,
            render::profiles::ZQ610_PLUS_203_DPI,
        ] {
            let options = zpl::Options {
                width: 64,
                height: 48,
                ..profile
            };
            for source in [
                "^XA^FO2,3^AZN,12,8^FDA^FS^XZ",
                "^XA^FO2,3^AZR,24,16^FDAA^FS^XZ",
                "^XA^FO2,3^A@N,12,8,R:PROBE.FNT^FDA^FS^XZ",
                "^CWY,R:PROBE.FNT^XA^FO2,3^AYN,12,8^FDA^FS^XZ",
            ] {
                let expected =
                    render::render_with_fonts(source.as_bytes(), options, &native).unwrap();
                let actual = render_c(source.as_bytes(), &options.into(), collection);
                assert_eq!((*actual).0.labels, expected.labels);
                assert_eq!((*actual).0.warnings, expected.warnings);
                zpl_document_free(actual);
            }
        }
        assert!(strike.calls.load(Ordering::Relaxed) > 0);
        let shared = &*collection;
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(move || {
                    let doc = render_c(
                        b"^XA^AZN,12,8^FDA^FS^XZ",
                        &render::profiles::SPECIFICATION.into(),
                        shared,
                    );
                    zpl_document_free(doc);
                });
            }
        });
        let doc = render_c(
            b"^XA^AZN,12,8^FDA^FS^XZ",
            &render::profiles::SPECIFICATION.into(),
            collection,
        );
        zpl_fonts_free(collection);
        let before = strike.calls.load(Ordering::Relaxed);
        let mut buffer = ptr::null_mut();
        assert_eq!(zpl_document_pdf(doc, ptr::null(), &mut buffer), ZPL_OK);
        assert_eq!(strike.calls.load(Ordering::Relaxed), before);
        zpl_buffer_free(buffer);
        zpl_document_free(doc);
    }
}
#[test]
fn callback_failures_and_registration_are_transactional() {
    unsafe {
        let strike = Strike {
            calls: AtomicUsize::new(0),
            mode: 0,
        };
        let mut p = provider(&strike);
        let mut fonts = ptr::null_mut();
        assert_eq!(zpl_fonts_new(&mut fonts), ZPL_OK);
        assert_eq!(zpl_fonts_insert_bitmap(fonts, 'Z' as u32, &p), ZPL_OK);
        p.metrics.baseline = f64::NAN;
        assert_eq!(
            zpl_fonts_insert_bitmap(fonts, 'Z' as u32, &p),
            ZPL_FONT_ERROR
        );
        let input = b"^XA^AZN,12,8^FDA^FS^XZ";
        let options: ZplOptions = render::profiles::SPECIFICATION.into();
        let doc = render_c(input, &options, fonts);
        zpl_document_free(doc);
        p = provider(&strike);
        p.glyph = None;
        assert_eq!(
            zpl_fonts_insert_bitmap(fonts, 'Z' as u32, &p),
            ZPL_INVALID_ARGUMENT
        );
        assert_eq!(
            zpl_fonts_insert_bitmap(fonts, 'a' as u32, &provider(&strike)),
            ZPL_FONT_ERROR
        );
        assert_eq!(
            zpl_fonts_insert_bitmap(fonts, 0xd800, &provider(&strike)),
            ZPL_INVALID_ARGUMENT
        );
        let missing = b"^XA^AZN,12,8^FDB^FS^XZ";
        let mut doc = ptr::null_mut();
        assert_eq!(
            zpl_render_with_fonts(
                missing.as_ptr(),
                missing.len(),
                &options,
                fonts,
                ptr::null(),
                &mut doc
            ),
            ZPL_RENDER_ERROR
        );
        assert!(doc.is_null());
        for mode in [1, 2] {
            let bad = Strike {
                calls: AtomicUsize::new(0),
                mode,
            };
            assert_eq!(
                zpl_fonts_insert_bitmap(fonts, 'Z' as u32, &provider(&bad)),
                ZPL_OK
            );
            assert_eq!(
                zpl_render_with_fonts(
                    input.as_ptr(),
                    input.len(),
                    &options,
                    fonts,
                    ptr::null(),
                    &mut doc
                ),
                ZPL_RENDER_ERROR
            );
            let error = zpl_last_error();
            assert!(error.offset < input.len());
            let message =
                String::from_utf8_lossy(bytes(error.message.data, error.message.len).unwrap());
            assert!(message.contains(if mode == 1 {
                "synthetic provider failure"
            } else {
                "invalid glyph bitmap length"
            }));
            assert!(doc.is_null());
            // Replace before dropping borrowed callback context.
            assert_eq!(
                zpl_fonts_insert_bitmap(fonts, 'Z' as u32, &provider(&strike)),
                ZPL_OK
            );
        }
        zpl_fonts_free(fonts);
    }
}
#[test]
fn copied_truetype_and_cross_type_named_replacement_match_native() {
    const TTF: &[u8] = include_bytes!(
        "../../zpl/tests/fixtures/truetype-regression/font-probes-20261002/probe.ttf"
    );
    unsafe {
        for (hint, native_hint) in [
            (0, zpl::truetype::Hinting::None),
            (1, zpl::truetype::Hinting::Native),
        ] {
            let mut fonts = ptr::null_mut();
            assert_eq!(zpl_fonts_new(&mut fonts), ZPL_OK);
            let name = b"  probe.ttf  ";
            let strike = Strike {
                calls: AtomicUsize::new(0),
                mode: 0,
            };
            assert_eq!(
                zpl_fonts_insert_named_bitmap(fonts, name.as_ptr(), name.len(), &provider(&strike)),
                ZPL_OK
            );
            let mut data = TTF.to_vec();
            assert_eq!(
                zpl_fonts_insert_truetype(fonts, 'Z' as u32, data.as_ptr(), data.len(), hint),
                ZPL_OK
            );
            let canonical = b"R:PROBE.TTF";
            assert_eq!(
                zpl_fonts_insert_named_truetype(
                    fonts,
                    canonical.as_ptr(),
                    canonical.len(),
                    data.as_ptr(),
                    data.len(),
                    hint
                ),
                ZPL_OK
            );
            data.fill(0); // C caller's TrueType bytes no longer retained.
            assert_eq!(
                zpl_fonts_insert_truetype(fonts, 'Z' as u32, data.as_ptr(), data.len(), hint),
                ZPL_FONT_ERROR
            );
            let mut native = Fonts::new();
            native.insert_truetype('Z', TTF, native_hint).unwrap();
            native
                .insert_named_truetype("probe.ttf", TTF, native_hint)
                .unwrap();
            let options = zpl::Options {
                width: 64,
                height: 48,
                ..render::profiles::SPECIFICATION
            };
            for source in [
                "^XA^FO2,3^AZN,24,20^FD!^FS^XZ",
                "^XA^FO2,3^A@N,24,20,probe.ttf^FD!^FS^XZ",
            ] {
                let expected =
                    render::render_with_fonts(source.as_bytes(), options, &native).unwrap();
                let actual = render_c(source.as_bytes(), &options.into(), fonts);
                assert_eq!((*actual).0.labels, expected.labels);
                zpl_document_free(actual);
            }
            assert_eq!(strike.calls.load(Ordering::Relaxed), 0);
            assert_eq!(
                zpl_fonts_insert_named_bitmap(fonts, name.as_ptr(), name.len(), &provider(&strike)),
                ZPL_OK
            );
            let doc = render_c(b"^XA^A@N,12,8,probe.ttf^FDA^FS^XZ", &options.into(), fonts);
            zpl_document_free(doc);
            assert!(strike.calls.load(Ordering::Relaxed) > 0);
            zpl_fonts_free(fonts);
        }
    }
}
