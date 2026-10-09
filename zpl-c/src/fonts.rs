//! Adapters for native font providers. Glyph layout follows zpl::fonts::BitmapFont;
//! callbacks follow the C ABI, never Rust trait-object layouts.
use super::*;
use std::{borrow::Cow, collections::BTreeMap, ffi::c_void, sync::Arc};
use zpl::{fonts as native, truetype::Hinting};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ZplBitmapMetrics {
    pub width: u32,
    pub height: u32,
    pub baseline: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ZplGlyph {
    pub advance: u32,
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub bitmap: ZplBytes,
}
pub type ZplGlyphCallback = unsafe extern "C" fn(
    user_data: *mut c_void,
    codepoint: u32,
    out: *mut ZplGlyph,
    error: *mut ZplBytes,
) -> i32;
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ZplBitmapProvider {
    pub metrics: ZplBitmapMetrics,
    pub user_data: *mut c_void,
    pub glyph: Option<ZplGlyphCallback>,
}
struct BitmapProvider(ZplBitmapProvider);

// SAFETY: registration requires callbacks and their user data to support
// concurrent lookup. C callers must keep returned memory valid until render ends.
unsafe impl Send for BitmapProvider {}
unsafe impl Sync for BitmapProvider {}
impl native::BitmapFont for BitmapProvider {
    fn metrics(&self) -> native::BitmapMetrics {
        native::BitmapMetrics {
            width: self.0.metrics.width,
            height: self.0.metrics.height,
            baseline: self.0.metrics.baseline,
        }
    }
    fn glyph(
        &self,
        character: char,
    ) -> std::result::Result<Option<Cow<'_, native::Glyph>>, String> {
        let empty = ZplBytes {
            data: ptr::null(),
            len: 0,
        };
        let mut glyph = ZplGlyph {
            advance: 0,
            left: 0,
            top: 0,
            width: 0,
            height: 0,
            bitmap: empty,
        };
        let mut error = empty;
        // SAFETY: enforced as a caller obligation at registration; descriptor
        // and output slots are live, aligned, and initialized for this call.
        let status = unsafe {
            self.0.glyph.ok_or("null glyph callback")?(
                self.0.user_data,
                character as u32,
                &mut glyph,
                &mut error,
            )
        };
        match status {
            1 => return Ok(None),
            0 => {}
            _ => {
                let message = unsafe { bytes(error.data, error.len) }.map_err(|e| e.message)?;
                return Err(if message.is_empty() {
                    "font provider callback failed".into()
                } else {
                    String::from_utf8_lossy(message).into_owned()
                });
            }
        }
        if glyph.width > 4096
            || glyph.height > 4096
            || glyph.advance > 4096
            || glyph.left.unsigned_abs() > 4096
            || glyph.top.unsigned_abs() > 4096
        {
            return Err("invalid glyph metrics or bitmap".into());
        }
        let stride = glyph.width.div_ceil(8) as usize;
        if glyph.bitmap.len != stride * glyph.height as usize {
            return Err("invalid glyph bitmap length".into());
        }
        let bitmap =
            unsafe { bytes(glyph.bitmap.data, glyph.bitmap.len) }.map_err(|e| e.message)?;
        let rows = (0..glyph.height as usize)
            .map(|row| bitmap[row * stride..(row + 1) * stride].to_vec())
            .collect();
        Ok(Some(Cow::Owned(native::Glyph {
            codepoint: character as u32,
            advance: glyph.advance,
            left: glyph.left,
            top: glyph.top,
            width: glyph.width,
            height: glyph.height,
            bitmap: rows,
        })))
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Id(char),
    Name(String),
}
enum Resource {
    Bitmap(Arc<BitmapProvider>),
    TrueType(Vec<u8>, Hinting),
}
#[derive(Default)]
pub struct ZplFonts {
    resources: BTreeMap<Key, Resource>,
}
fn font_failure(message: String) -> Failure {
    Failure {
        status: ZPL_FONT_ERROR,
        offset: usize::MAX,
        message,
    }
}
impl Resource {
    fn insert<'a>(&'a self, key: &Key, fonts: &mut native::Fonts<'a>) -> Result<()> {
        match (key, self) {
            (Key::Id(id), Self::Bitmap(provider)) => {
                fonts.insert_bitmap_font(*id, provider.clone())
            }
            (Key::Name(name), Self::Bitmap(provider)) => {
                fonts.insert_named_bitmap_font(name, provider.clone())
            }
            (Key::Id(id), Self::TrueType(data, hinting)) => {
                fonts.insert_truetype(*id, data, *hinting)
            }
            (Key::Name(name), Self::TrueType(data, hinting)) => {
                fonts.insert_named_truetype(name, data, *hinting)
            }
        }
        .map_err(font_failure)
    }
}
impl ZplFonts {
    pub(super) fn native(&self) -> Result<native::Fonts<'_>> {
        let mut fonts = native::Fonts::new();
        for (key, resource) in &self.resources {
            resource.insert(key, &mut fonts)?;
        }
        Ok(fonts)
    }
    fn insert(&mut self, key: Key, resource: Resource) -> Result<()> {
        // Native validation precedes replacement, including ID/name/face checks.
        resource.insert(&key, &mut native::Fonts::new())?;
        self.resources.insert(key, resource);
        Ok(())
    }
}
fn id_key(id: u32) -> Result<Key> {
    char::from_u32(id)
        .map(Key::Id)
        .ok_or_else(|| Failure::argument("invalid font ID scalar"))
}
unsafe fn name_key(data: *const u8, len: usize) -> Result<Key> {
    let name = std::str::from_utf8(unsafe { bytes(data, len)? })
        .map_err(|_| Failure::argument("font name must be UTF-8"))?;
    // Native insert validates names. Normalize equivalent accepted names so
    // replacing a named resource also releases its previous bytes/provider.
    let name = name.trim().to_ascii_uppercase();
    Ok(Key::Name(if name.contains(':') {
        name
    } else {
        format!("R:{name}")
    }))
}
unsafe fn provider(value: *const ZplBitmapProvider) -> Result<Resource> {
    let value = *unsafe { required(value)? };
    if value.glyph.is_none() {
        return Err(Failure::argument("null glyph callback"));
    }
    Ok(Resource::Bitmap(Arc::new(BitmapProvider(value))))
}
unsafe fn truetype(data: *const u8, len: usize, hinting: u32) -> Result<Resource> {
    let hinting = match hinting {
        0 => Hinting::None,
        1 => Hinting::Native,
        _ => return Err(Failure::argument("unknown hinting mode")),
    };
    Ok(Resource::TrueType(
        unsafe { bytes(data, len)? }.to_vec(),
        hinting,
    ))
}
#[no_mangle]
pub unsafe extern "C" fn zpl_fonts_new(out: *mut *mut ZplFonts) -> i32 {
    boundary(|| {
        let out = unsafe { destination(out)? };
        *out = ptr::null_mut();
        *out = Box::into_raw(Box::new(ZplFonts::default()));
        Ok(())
    })
}
#[no_mangle]
pub unsafe extern "C" fn zpl_fonts_free(value: *mut ZplFonts) {
    if !value.is_null() {
        drop(unsafe { Box::from_raw(value) });
    }
}
#[no_mangle]
pub unsafe extern "C" fn zpl_fonts_insert_bitmap(
    value: *mut ZplFonts,
    id: u32,
    font: *const ZplBitmapProvider,
) -> i32 {
    boundary(|| unsafe { destination(value)? }.insert(id_key(id)?, unsafe { provider(font)? }))
}
#[no_mangle]
pub unsafe extern "C" fn zpl_fonts_insert_named_bitmap(
    value: *mut ZplFonts,
    name: *const u8,
    name_len: usize,
    font: *const ZplBitmapProvider,
) -> i32 {
    boundary(|| {
        unsafe { destination(value)? }.insert(unsafe { name_key(name, name_len)? }, unsafe {
            provider(font)?
        })
    })
}
#[no_mangle]
pub unsafe extern "C" fn zpl_fonts_insert_truetype(
    value: *mut ZplFonts,
    id: u32,
    data: *const u8,
    len: usize,
    hinting: u32,
) -> i32 {
    boundary(|| {
        unsafe { destination(value)? }.insert(id_key(id)?, unsafe { truetype(data, len, hinting)? })
    })
}
#[no_mangle]
pub unsafe extern "C" fn zpl_fonts_insert_named_truetype(
    value: *mut ZplFonts,
    name: *const u8,
    name_len: usize,
    data: *const u8,
    len: usize,
    hinting: u32,
) -> i32 {
    boundary(|| {
        unsafe { destination(value)? }.insert(unsafe { name_key(name, name_len)? }, unsafe {
            truetype(data, len, hinting)?
        })
    })
}
