use super::*;
use output::Adapter;
use std::mem::MaybeUninit;

unsafe fn take(buffer: *mut ZplBuffer) -> Vec<u8> {
    let view = unsafe { zpl_buffer_bytes(buffer) };
    let result = unsafe { bytes(view.data, view.len) }.unwrap().to_vec();
    unsafe { zpl_buffer_free(buffer) };
    result
}

#[test]
fn outputs_and_profiles_match_native_and_survive_document() {
    unsafe {
        let input = b"^XA^FO2,3^GB10,8,2^FS^XZ^XA^FO1,1^GB3,4,1^FS^XZ";
        for (profile, mut native_options) in [
            render::profiles::ZD621_203_DPI,
            render::profiles::SPECIFICATION,
            render::profiles::ZQ610_PLUS_203_DPI,
        ]
        .into_iter()
        .enumerate()
        {
            let mut options = MaybeUninit::uninit();
            assert_eq!(
                zpl_options_init(profile as u32, options.as_mut_ptr()),
                ZPL_OK
            );
            let mut options = options.assume_init();
            assert_eq!(options.native().unwrap(), native_options);
            options.width = 32;
            options.height = 24;
            native_options.width = 32;
            native_options.height = 24;
            let native = zpl::render(input, native_options).unwrap();
            let mut document = ptr::null_mut();
            assert_eq!(
                zpl_render(
                    input.as_ptr(),
                    input.len(),
                    &options,
                    ptr::null(),
                    &mut document
                ),
                ZPL_OK
            );
            assert_eq!(zpl_document_label_count(document), native.labels.len());
            assert_eq!(zpl_document_warning_count(document), native.warnings.len());
            let mut buffer = ptr::null_mut();
            assert_eq!(zpl_document_pdf(document, ptr::null(), &mut buffer), ZPL_OK);
            assert_eq!(
                take(buffer),
                output::Pdf.encode_pages(&native.labels).unwrap()
            );
            let mut scenes = Vec::new();
            for index in 0..native.labels.len() {
                let mut scene = ptr::null_mut();
                assert_eq!(zpl_document_label(document, index, &mut scene), ZPL_OK);
                scenes.push(scene);
            }
            zpl_document_free(document);
            for (scene, expected) in scenes.into_iter().zip(&native.labels) {
                let mut info = MaybeUninit::uninit();
                assert_eq!(zpl_scene_info(scene, info.as_mut_ptr()), ZPL_OK);
                let info = info.assume_init();
                assert_eq!(
                    (info.width, info.height, info.dpi),
                    (expected.width, expected.height, expected.dpi)
                );
                for (format, encoded) in [
                    output::Png.encode(expected).unwrap(),
                    output::Svg.encode(expected).unwrap(),
                    output::Pdf.encode(expected).unwrap(),
                    output::raster::rasterize(expected).unwrap().pixels,
                ]
                .into_iter()
                .enumerate()
                {
                    assert_eq!(
                        zpl_scene_encode(scene, format as u32, ptr::null(), &mut buffer),
                        ZPL_OK
                    );
                    assert_eq!(take(buffer), encoded);
                }
                zpl_scene_free(scene);
            }
        }
    }
}

#[test]
fn parse_preserves_binary_and_final_syntax() {
    unsafe {
        let mut input = b"prefix^XA^ZZunknown^FDa\0b^FS^GFB,4,4,4,^~\0\xff^FS^CC!!XZ".to_vec();
        let expected = input.clone();
        let mut parsed = ptr::null_mut();
        assert_eq!(
            zpl_parse(input.as_ptr(), input.len(), ptr::null(), &mut parsed),
            ZPL_OK
        );
        input.fill(0); // Result owns the original bytes.
        let mut rebuilt = Vec::new();
        for index in 0..zpl_parsed_count(parsed) {
            let mut element = MaybeUninit::uninit();
            assert_eq!(
                zpl_parsed_element(parsed, index, element.as_mut_ptr()),
                ZPL_OK
            );
            let element = element.assume_init();
            assert_eq!(element.offset, rebuilt.len());
            rebuilt.extend_from_slice(bytes(element.data.data, element.data.len).unwrap());
        }
        assert_eq!(rebuilt, expected);
        let mut syntax = MaybeUninit::uninit();
        assert_eq!(zpl_parsed_syntax(parsed, syntax.as_mut_ptr()), ZPL_OK);
        assert_eq!(syntax.assume_init().format_prefix, b'!');
        zpl_parsed_free(parsed);
    }
}

#[test]
fn invalid_arguments_limits_errors_and_panic_containment() {
    unsafe {
        let input = b"^XA^FO0,0^GB2,2,1^FS^XZ";
        let mut doc = ptr::null_mut();
        assert_eq!(
            zpl_render(ptr::null(), 1, ptr::null(), ptr::null(), &mut doc),
            ZPL_INVALID_ARGUMENT
        );
        assert!(doc.is_null());
        let mut options: ZplOptions = render::profiles::SPECIFICATION.into();
        options.width = 8;
        options.height = 8;
        options.compatibility.code39_normalize_input = 2;
        assert_eq!(
            zpl_render(input.as_ptr(), input.len(), &options, ptr::null(), &mut doc),
            ZPL_INVALID_ARGUMENT
        );
        options.compatibility.code39_normalize_input = 0;
        options.compatibility.preview_width_quantum.present = 2;
        assert_eq!(
            zpl_render(input.as_ptr(), input.len(), &options, ptr::null(), &mut doc),
            ZPL_INVALID_ARGUMENT
        );
        options.compatibility.preview_width_quantum.present = 0;
        let mut limits: ZplRenderLimits = render::Limits::default().into();
        limits.input_bytes = 1;
        assert_eq!(
            zpl_render(input.as_ptr(), input.len(), &options, &limits, &mut doc),
            ZPL_RENDER_ERROR
        );
        assert!(!zpl_last_error().message.data.is_null());
        limits.number_abs = f64::NAN;
        assert_eq!(
            zpl_render(input.as_ptr(), input.len(), &options, &limits, &mut doc),
            ZPL_INVALID_ARGUMENT
        );
        assert_eq!(
            zpl_render(input.as_ptr(), input.len(), &options, ptr::null(), &mut doc),
            ZPL_OK
        );
        assert_eq!(zpl_last_error().status, ZPL_OK);
        let mut scene = ptr::null_mut();
        assert_eq!(zpl_document_label(doc, 2, &mut scene), ZPL_INVALID_ARGUMENT);
        assert!(scene.is_null());
        assert_eq!(zpl_document_label(doc, 0, &mut scene), ZPL_OK);
        let mut out = ptr::null_mut();
        let mut limits: ZplOutputLimits = output::Limits::default().into();
        limits.pixels = 1;
        assert_eq!(
            zpl_scene_encode(scene, 0, &limits, &mut out),
            ZPL_OUTPUT_ERROR
        );
        assert!(out.is_null());
        assert_eq!(
            zpl_scene_encode(scene, 99, ptr::null(), &mut out),
            ZPL_INVALID_ARGUMENT
        );
        zpl_scene_free(scene);
        zpl_document_free(doc);
        let mut parsed = ptr::null_mut();
        let bad = b"^XA^";
        assert_eq!(
            zpl_parse(bad.as_ptr(), bad.len(), ptr::null(), &mut parsed),
            ZPL_PARSE_ERROR
        );
        assert_eq!(zpl_last_error().offset, 3);
        assert!(parsed.is_null());
        assert_eq!(zpl_parse(ptr::null(), 0, ptr::null(), &mut parsed), ZPL_OK);
        assert_eq!(zpl_parsed_count(parsed), 0);
        zpl_parsed_free(parsed);
        assert_eq!(boundary(|| panic!("test unwind")), ZPL_PANIC);
        assert_eq!(zpl_last_error().status, ZPL_PANIC);
        std::thread::spawn(|| {
            assert_eq!(zpl_last_error().status, ZPL_OK);
            assert_eq!(zpl_options_init(100, ptr::null_mut()), ZPL_INVALID_ARGUMENT);
        })
        .join()
        .unwrap();
        assert_eq!(zpl_last_error().status, ZPL_PANIC);
    }
}

#[test]
fn warnings_preserve_native_text() {
    unsafe {
        let input = b"^XA^FO1,1^A0N,24,24^FDHello^FS^XZ";
        let native = zpl::render(input, render::profiles::SPECIFICATION).unwrap();
        assert!(!native.warnings.is_empty());
        let options: ZplOptions = render::profiles::SPECIFICATION.into();
        let mut document = ptr::null_mut();
        assert_eq!(
            zpl_render(
                input.as_ptr(),
                input.len(),
                &options,
                ptr::null(),
                &mut document
            ),
            ZPL_OK
        );
        assert_eq!(zpl_document_warning_count(document), native.warnings.len());
        for (index, expected) in native.warnings.iter().enumerate() {
            let mut warning = MaybeUninit::uninit();
            assert_eq!(
                zpl_document_warning(document, index, warning.as_mut_ptr()),
                ZPL_OK
            );
            let warning = warning.assume_init();
            assert_eq!(
                bytes(warning.data, warning.len).unwrap(),
                expected.as_bytes()
            );
        }
        zpl_document_free(document);
    }
}
