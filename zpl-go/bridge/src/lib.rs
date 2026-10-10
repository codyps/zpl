//! Experimental scalar ABI shared by native Go FFI and a freestanding Wasm guest.
//! C ABI: https://doc.rust-lang.org/reference/items/external-blocks.html#abi
//! Wasm host pointers are offsets in linear memory, never native host addresses:
//! https://webassembly.github.io/spec/core/exec/runtime.html#memory-instances
#![deny(unsafe_op_in_unsafe_fn)]
use std::{panic::catch_unwind, slice};
use zpl::{
    output::{self, Adapter, Pdf, Png, Svg},
    render,
};

const MAX_INPUT: usize = 1024 * 1024;

// Wire v1: eight little-endian u32s (version, status, offset, width, height,
// label count, warning byte count, body byte count), then warnings and body.
// Error bodies contain UTF-8 diagnostics; u32::MAX means no input offset.
fn packet(
    status: u32,
    offset: u32,
    dimensions: (u32, u32),
    labels: u32,
    warnings: &[u8],
    body: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + warnings.len() + body.len());
    for n in [
        1,
        status,
        offset,
        dimensions.0,
        dimensions.1,
        labels,
        warnings.len() as u32,
        body.len() as u32,
    ] {
        out.extend_from_slice(&n.to_le_bytes());
    }
    out.extend_from_slice(warnings);
    out.extend_from_slice(body);
    out
}
fn error(status: u32, offset: u32, message: &str) -> Vec<u8> {
    packet(status, offset, (0, 0), 0, &[], message.as_bytes())
}

/// Prototype contract: bounded input, first label for PNG/SVG/gray, all for PDF.
/// Profiles are native constants; output adapters retain their bounded defaults.
pub fn render_packet(input: &[u8], format: u32, profile: u32, width: u32, height: u32) -> Vec<u8> {
    if input.len() > MAX_INPUT {
        return error(1, u32::MAX, "prototype input exceeds 1 MiB");
    }
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return error(1, u32::MAX, "prototype dimensions must be 1..4096");
    }
    if format > 3 {
        return error(1, u32::MAX, "unknown output format");
    }
    let mut options = match profile {
        0 => render::profiles::SPECIFICATION,
        1 => render::profiles::ZD621_203_DPI,
        2 => render::profiles::ZQ610_PLUS_203_DPI,
        _ => return error(1, u32::MAX, "unknown profile"),
    };
    options.width = width;
    options.height = height;
    let document = match render::render_with_limits(input, options, render::Limits::default()) {
        Ok(d) => d,
        Err(e) => return error(2, e.offset as u32, &e.to_string()),
    };
    let Some(scene) = document.labels.first() else {
        return error(2, u32::MAX, "no labels");
    };
    let encoded = match format {
        0 => Png.encode(scene),
        1 => Svg.encode(scene),
        2 => Pdf.encode_pages(&document.labels),
        3 => output::raster::rasterize(scene).map(|r| r.pixels),
        _ => unreachable!(),
    };
    match encoded {
        Ok(body) => packet(
            0,
            u32::MAX,
            (scene.width, scene.height),
            document.labels.len() as u32,
            document.warnings.join("\n").as_bytes(),
            &body,
        ),
        Err(e) => error(3, u32::MAX, &e.to_string()),
    }
}

#[no_mangle]
pub extern "C" fn zp_abi() -> u32 {
    1
}

/// Allocate input storage. A null result means the size exceeds the input budget.
#[no_mangle]
pub extern "C" fn zp_alloc(len: usize) -> *mut u8 {
    if len > MAX_INPUT {
        return std::ptr::null_mut();
    }
    Box::into_raw(vec![0u8; len].into_boxed_slice()) as *mut u8
}

/// # Safety
/// `data,len` must describe one live allocation from zp_alloc, freed exactly once.
#[no_mangle]
pub unsafe extern "C" fn zp_dealloc(data: *mut u8, len: usize) {
    if !data.is_null() {
        unsafe {
            drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(data, len)));
        }
    }
}

/// # Safety
/// Input must be readable for len bytes and remain live for the synchronous call.
/// The returned result is owned; free with zp_result_free. No Go pointers retained.
#[no_mangle]
pub unsafe extern "C" fn zp_render(
    data: *const u8,
    len: usize,
    format: u32,
    profile: u32,
    width: u32,
    height: u32,
) -> *mut Vec<u8> {
    let result = catch_unwind(|| {
        if len > MAX_INPUT || (len != 0 && data.is_null()) {
            return error(1, u32::MAX, "invalid input buffer");
        }
        let input = if len == 0 {
            &[]
        } else {
            unsafe { slice::from_raw_parts(data, len) }
        };
        render_packet(input, format, profile, width, height)
    })
    .unwrap_or_else(|_| error(4, u32::MAX, "Rust panic"));
    Box::into_raw(Box::new(result))
}
/// # Safety
/// Result must be live. Returned storage is borrowed until zp_result_free.
#[no_mangle]
pub unsafe extern "C" fn zp_result_data(result: *const Vec<u8>) -> *const u8 {
    unsafe { (*result).as_ptr() }
}
/// # Safety
/// Result must be live.
#[no_mangle]
pub unsafe extern "C" fn zp_result_len(result: *const Vec<u8>) -> usize {
    unsafe { (*result).len() }
}
/// # Safety
/// Result must come from zp_render and be freed exactly once, with no live views.
#[no_mangle]
pub unsafe extern "C" fn zp_result_free(result: *mut Vec<u8>) {
    if !result.is_null() {
        unsafe {
            drop(Box::from_raw(result));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffi_result_outlives_input_and_later_errors() {
        let source = b"^XA^FO10,10^GB30,40,2^FS^XZ";
        // Exercise the same ownership boundary used by all four Go prototypes.
        unsafe {
            let input = zp_alloc(source.len());
            std::ptr::copy_nonoverlapping(source.as_ptr(), input, source.len());
            let result = zp_render(input, source.len(), 0, 0, 100, 100);
            zp_dealloc(input, source.len());
            let failure = zp_render(std::ptr::null(), 1, 0, 0, 100, 100);
            let failure_bytes =
                slice::from_raw_parts(zp_result_data(failure), zp_result_len(failure));
            assert_eq!(&failure_bytes[4..8], &1u32.to_le_bytes());
            zp_result_free(failure);
            let packet = slice::from_raw_parts(zp_result_data(result), zp_result_len(result));
            let options = zpl::Options {
                width: 100,
                height: 100,
                ..render::profiles::SPECIFICATION
            };
            let document = zpl::render(source, options).unwrap();
            assert_eq!(&packet[32..], Png.encode(&document.labels[0]).unwrap());
            zp_result_free(result);
        }
    }

    #[test]
    fn binary_input_and_all_profiles_preserve_native_output() {
        // Same raw-GFB semantics as zpl/tests/render.rs, including zero/high bytes.
        let source = b"^XA^GFB,4,4,1,\xff\x00\x5e\x7e^FS^XZ";
        for (id, profile) in [
            render::profiles::SPECIFICATION,
            render::profiles::ZD621_203_DPI,
            render::profiles::ZQ610_PLUS_203_DPI,
        ]
        .into_iter()
        .enumerate()
        {
            let packet = render_packet(source, 3, id as u32, 100, 100);
            assert_eq!(&packet[4..8], &0u32.to_le_bytes());
            let options = zpl::Options {
                width: 100,
                height: 100,
                ..profile
            };
            let document = zpl::render(source, options).unwrap();
            let expected = output::raster::rasterize(&document.labels[0]).unwrap();
            assert_eq!(&packet[32..], expected.pixels);
        }
    }

    #[test]
    fn rejected_inputs_do_not_allocate_or_dereference() {
        assert!(zp_alloc(MAX_INPUT + 1).is_null());
        unsafe {
            let result = zp_render(std::ptr::null(), MAX_INPUT + 1, 0, 0, 100, 100);
            assert_eq!((&*result)[4], 1);
            zp_result_free(result);
            zp_result_free(std::ptr::null_mut());
            zp_dealloc(std::ptr::null_mut(), 0);
        }
        assert_eq!(render_packet(b"", 0, 0, 100, 100)[4], 2);
    }
}
