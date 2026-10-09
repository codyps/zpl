//! C ABI for parsing, rendering, and scene output. See include/zpl.h for ownership.
//! ABI/layout rules: https://doc.rust-lang.org/reference/type-layout.html#the-c-representation
//! Pointer contracts: https://doc.rust-lang.org/std/slice/fn.from_raw_parts.html
#![deny(unsafe_op_in_unsafe_fn)]
// All exported unsafe functions share the pointer contract documented below and
// in the installed C header, instead of repeating it on each declaration.
#![allow(clippy::missing_safety_doc)]
mod config;
pub use config::*;
use std::{
    cell::RefCell,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr, slice,
};
use zpl::{output, parse, render};

pub const ZPL_OK: i32 = 0;
pub const ZPL_INVALID_ARGUMENT: i32 = 1;
pub const ZPL_PARSE_ERROR: i32 = 2;
pub const ZPL_RENDER_ERROR: i32 = 3;
pub const ZPL_OUTPUT_ERROR: i32 = 4;
pub const ZPL_PANIC: i32 = 5;

type Result<T> = std::result::Result<T, Failure>;
#[derive(Debug)]
struct Failure {
    status: i32,
    offset: usize,
    message: String,
}
impl Failure {
    fn argument(message: &str) -> Self {
        Self {
            status: ZPL_INVALID_ARGUMENT,
            offset: usize::MAX,
            message: message.into(),
        }
    }
}
thread_local! {
    static ERROR: RefCell<Option<Failure>> = const { RefCell::new(None) };
}
fn boundary(f: impl FnOnce() -> Result<()>) -> i32 {
    let result = catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|_| {
        Err(Failure {
            status: ZPL_PANIC,
            offset: usize::MAX,
            message: "Rust panic in ZPL library".into(),
        })
    });
    let error = result.err();
    let status = error.as_ref().map_or(ZPL_OK, |e| e.status);
    ERROR.with(|slot| *slot.borrow_mut() = error);
    status
}
fn output_failure(error: output::OutputError) -> Failure {
    Failure {
        status: ZPL_OUTPUT_ERROR,
        offset: usize::MAX,
        message: error.to_string(),
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ZplBytes {
    pub data: *const u8,
    pub len: usize,
}
impl ZplBytes {
    fn new(bytes: &[u8]) -> Self {
        Self {
            data: bytes.as_ptr(),
            len: bytes.len(),
        }
    }
}
#[repr(C)]
pub struct ZplError {
    pub status: i32,
    pub offset: usize,
    pub message: ZplBytes,
}
#[repr(C)]
pub struct ZplSceneInfo {
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
}
#[repr(C)]
pub struct ZplElement {
    pub kind: u32,
    pub offset: usize,
    pub data: ZplBytes,
}

pub struct ZplDocument(render::Document);
pub struct ZplScene(output::Scene);
pub struct ZplBuffer(Vec<u8>);
pub struct ZplParsed {
    source: Vec<u8>,
    elements: Vec<(u32, usize, usize)>,
    syntax: parse::Syntax,
}

// Foreign callers must supply valid aligned pointers, with no overlapping
// mutable storage. Nulls are checked; arbitrary dangling pointers cannot be.
unsafe fn required<'a, T>(value: *const T) -> Result<&'a T> {
    unsafe { value.as_ref() }.ok_or_else(|| Failure::argument("null required pointer"))
}
unsafe fn destination<'a, T>(value: *mut T) -> Result<&'a mut T> {
    unsafe { value.as_mut() }.ok_or_else(|| Failure::argument("null output pointer"))
}
unsafe fn bytes<'a>(data: *const u8, len: usize) -> Result<&'a [u8]> {
    if len > isize::MAX as usize {
        return Err(Failure::argument("input length exceeds isize::MAX"));
    }
    if len == 0 {
        return Ok(&[]);
    }
    if data.is_null() {
        return Err(Failure::argument("null input with nonzero length"));
    }
    Ok(unsafe { slice::from_raw_parts(data, len) })
}
unsafe fn render_limits(value: *const ZplRenderLimits) -> Result<render::Limits> {
    unsafe { value.as_ref() }.map_or_else(|| Ok(render::Limits::default()), |v| v.native())
}
unsafe fn output_limits(value: *const ZplOutputLimits) -> Result<output::Limits> {
    unsafe { value.as_ref() }.map_or_else(|| Ok(output::Limits::default()), |v| v.native())
}

#[no_mangle]
pub extern "C" fn zpl_abi_version() -> u32 {
    1
}
#[no_mangle]
pub extern "C" fn zpl_library_version() -> ZplBytes {
    ZplBytes::new(zpl::version::VERSION.as_bytes())
}
#[no_mangle]
pub extern "C" fn zpl_last_error() -> ZplError {
    ERROR.with(|slot| match slot.borrow().as_ref() {
        Some(e) => ZplError {
            status: e.status,
            offset: e.offset,
            message: ZplBytes::new(e.message.as_bytes()),
        },
        None => ZplError {
            status: ZPL_OK,
            offset: usize::MAX,
            message: ZplBytes {
                data: ptr::null(),
                len: 0,
            },
        },
    })
}

#[no_mangle]
pub unsafe extern "C" fn zpl_options_init(profile: u32, out: *mut ZplOptions) -> i32 {
    boundary(|| {
        let value = match profile {
            0 => render::profiles::ZD621_203_DPI,
            1 => render::profiles::SPECIFICATION,
            2 => render::profiles::ZQ610_PLUS_203_DPI,
            _ => return Err(Failure::argument("unknown profile")),
        };
        *unsafe { destination(out)? } = value.into();
        Ok(())
    })
}
macro_rules! init {
    ($function:ident, $c:ty, $native:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn $function(out: *mut $c) -> i32 {
            boundary(|| {
                *unsafe { destination(out)? } = <$native>::default().into();
                Ok(())
            })
        }
    };
}
init!(zpl_render_limits_init, ZplRenderLimits, render::Limits);
init!(zpl_output_limits_init, ZplOutputLimits, output::Limits);
init!(zpl_syntax_init, ZplSyntax, parse::Syntax);

#[no_mangle]
pub unsafe extern "C" fn zpl_render(
    data: *const u8,
    len: usize,
    options: *const ZplOptions,
    limits: *const ZplRenderLimits,
    out: *mut *mut ZplDocument,
) -> i32 {
    boundary(|| {
        let out = unsafe { destination(out)? };
        *out = ptr::null_mut();
        let options = unsafe { options.as_ref() }
            .map_or_else(|| Ok(zpl::Options::default()), |v| v.native())?;
        let document = render::render_with_limits(unsafe { bytes(data, len)? }, options, unsafe {
            render_limits(limits)?
        })
        .map_err(|e| Failure {
            status: ZPL_RENDER_ERROR,
            offset: e.offset,
            message: e.to_string(),
        })?;
        *out = Box::into_raw(Box::new(ZplDocument(document)));
        Ok(())
    })
}

macro_rules! free {
    ($function:ident, $ty:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn $function(value: *mut $ty) {
            if !value.is_null() {
                drop(unsafe { Box::from_raw(value) });
            }
        }
    };
}
free!(zpl_document_free, ZplDocument);
free!(zpl_scene_free, ZplScene);
free!(zpl_buffer_free, ZplBuffer);
free!(zpl_parsed_free, ZplParsed);

#[no_mangle]
pub unsafe extern "C" fn zpl_document_label_count(value: *const ZplDocument) -> usize {
    unsafe { value.as_ref() }.map_or(0, |v| v.0.labels.len())
}
#[no_mangle]
pub unsafe extern "C" fn zpl_document_warning_count(value: *const ZplDocument) -> usize {
    unsafe { value.as_ref() }.map_or(0, |v| v.0.warnings.len())
}
#[no_mangle]
pub unsafe extern "C" fn zpl_document_warning(
    value: *const ZplDocument,
    index: usize,
    out: *mut ZplBytes,
) -> i32 {
    boundary(|| {
        let out = unsafe { destination(out)? };
        *out = ZplBytes {
            data: ptr::null(),
            len: 0,
        };
        let warning = unsafe { required(value)? }
            .0
            .warnings
            .get(index)
            .ok_or_else(|| Failure::argument("warning index out of range"))?;
        *out = ZplBytes::new(warning.as_bytes());
        Ok(())
    })
}
/// Return an owned scene snapshot, independent of the document lifetime.
#[no_mangle]
pub unsafe extern "C" fn zpl_document_label(
    value: *const ZplDocument,
    index: usize,
    out: *mut *mut ZplScene,
) -> i32 {
    boundary(|| {
        let out = unsafe { destination(out)? };
        *out = ptr::null_mut();
        let scene = unsafe { required(value)? }
            .0
            .labels
            .get(index)
            .ok_or_else(|| Failure::argument("label index out of range"))?;
        *out = Box::into_raw(Box::new(ZplScene(scene.clone())));
        Ok(())
    })
}
#[no_mangle]
pub unsafe extern "C" fn zpl_scene_info(value: *const ZplScene, out: *mut ZplSceneInfo) -> i32 {
    boundary(|| {
        let scene = &unsafe { required(value)? }.0;
        *unsafe { destination(out)? } = ZplSceneInfo {
            width: scene.width,
            height: scene.height,
            dpi: scene.dpi,
        };
        Ok(())
    })
}
#[no_mangle]
pub unsafe extern "C" fn zpl_scene_encode(
    value: *const ZplScene,
    format: u32,
    limits: *const ZplOutputLimits,
    out: *mut *mut ZplBuffer,
) -> i32 {
    boundary(|| {
        let out = unsafe { destination(out)? };
        *out = ptr::null_mut();
        let scene = &unsafe { required(value)? }.0;
        let limits = unsafe { output_limits(limits)? };
        let result = match format {
            0 => output::Png.encode_with_limits(scene, limits),
            1 => output::Svg.encode_with_limits(scene, limits),
            2 => output::Pdf.encode_pages_with_limits(slice::from_ref(scene), limits),
            3 => output::raster::rasterize_with_limits(scene, limits).map(|r| r.pixels),
            _ => return Err(Failure::argument("unknown output format")),
        }
        .map_err(output_failure)?;
        *out = Box::into_raw(Box::new(ZplBuffer(result)));
        Ok(())
    })
}
#[no_mangle]
pub unsafe extern "C" fn zpl_document_pdf(
    value: *const ZplDocument,
    limits: *const ZplOutputLimits,
    out: *mut *mut ZplBuffer,
) -> i32 {
    boundary(|| {
        let out = unsafe { destination(out)? };
        *out = ptr::null_mut();
        let document = unsafe { required(value)? };
        let result = output::Pdf
            .encode_pages_with_limits(&document.0.labels, unsafe { output_limits(limits)? })
            .map_err(output_failure)?;
        *out = Box::into_raw(Box::new(ZplBuffer(result)));
        Ok(())
    })
}
#[no_mangle]
pub unsafe extern "C" fn zpl_buffer_bytes(value: *const ZplBuffer) -> ZplBytes {
    unsafe { value.as_ref() }.map_or(
        ZplBytes {
            data: ptr::null(),
            len: 0,
        },
        |v| ZplBytes::new(&v.0),
    )
}

#[no_mangle]
pub unsafe extern "C" fn zpl_parse(
    data: *const u8,
    len: usize,
    syntax: *const ZplSyntax,
    out: *mut *mut ZplParsed,
) -> i32 {
    boundary(|| {
        let out = unsafe { destination(out)? };
        *out = ptr::null_mut();
        let source = unsafe { bytes(data, len)? }.to_vec();
        let syntax = unsafe { syntax.as_ref() }
            .map_or_else(|| Ok(parse::Syntax::default()), |v| v.native())?;
        let mut parser = parse::ParseContext::with_syntax(&source, syntax);
        let mut elements = Vec::new();
        loop {
            let offset = parser.position();
            let Some(element) = parser.next() else { break };
            let element = element.map_err(|e| Failure {
                status: ZPL_PARSE_ERROR,
                offset: e.offset,
                message: e.to_string(),
            })?;
            let kind = match element {
                parse::Element::BeforeFirstCommand(_) => 0,
                parse::Element::FormatCommand(_) => 1,
                parse::Element::ControlCommand(_) => 2,
                parse::Element::ControlCharacter(_) => 3,
            };
            elements.push((kind, offset, element.as_bytes().len()));
        }
        let syntax = parser.syntax();
        *out = Box::into_raw(Box::new(ZplParsed {
            source,
            elements,
            syntax,
        }));
        Ok(())
    })
}
#[no_mangle]
pub unsafe extern "C" fn zpl_parsed_count(value: *const ZplParsed) -> usize {
    unsafe { value.as_ref() }.map_or(0, |v| v.elements.len())
}
#[no_mangle]
pub unsafe extern "C" fn zpl_parsed_syntax(value: *const ZplParsed, out: *mut ZplSyntax) -> i32 {
    boundary(|| {
        *unsafe { destination(out)? } = unsafe { required(value)? }.syntax.into();
        Ok(())
    })
}
#[no_mangle]
pub unsafe extern "C" fn zpl_parsed_element(
    value: *const ZplParsed,
    index: usize,
    out: *mut ZplElement,
) -> i32 {
    boundary(|| {
        let value = unsafe { required(value)? };
        let &(kind, offset, len) = value
            .elements
            .get(index)
            .ok_or_else(|| Failure::argument("element index out of range"))?;
        *unsafe { destination(out)? } = ZplElement {
            kind,
            offset,
            data: ZplBytes::new(&value.source[offset..offset + len]),
        };
        Ok(())
    })
}

#[cfg(test)]
mod tests;
