//! Byte-preserving Node adapter using the same scenes and output adapters as Rust.
use std::sync::OnceLock;
use wasm_bindgen::prelude::*;
use zpl::{
    fonts::{resolve_rom_font, Face, Fonts},
    output::{Adapter, Pdf, Png, Svg},
    render::{profiles, Document, Limits},
    truetype::Hinting,
    Options,
};

// Append-only render-local storage: OnceLock lets the Send + Sync Rust
// resolver retain borrowed font bytes without leaking allocations or extending
// lifetimes unsafely. Resolved faces are dropped before this storage.
#[derive(Default)]
struct FontData {
    bytes: OnceLock<Vec<u8>>,
    next: OnceLock<Box<FontData>>,
}
impl FontData {
    fn insert(&self, mut bytes: Vec<u8>) -> &[u8] {
        let mut node = self;
        loop {
            match node.bytes.set(bytes) {
                Ok(()) => return node.bytes.get().unwrap(),
                Err(value) => bytes = value,
            }
            node = node.next.get_or_init(Default::default);
        }
    }
}
impl Drop for FontData {
    fn drop(&mut self) {
        // Avoid recursive destruction for jobs resolving many distinct names.
        let mut next = self.next.take();
        while let Some(mut node) = next {
            next = node.next.take();
        }
    }
}

fn resolve<'a>(
    callback: &js_sys::Function,
    name: &str,
    data: &'a FontData,
) -> Result<Option<Face<'a>>, String> {
    let value = callback
        .call1(&JsValue::UNDEFINED, &JsValue::from_str(name))
        .map_err(|error| {
            error
                .as_string()
                .or_else(|| {
                    js_sys::Reflect::get(&error, &JsValue::from_str("message"))
                        .ok()
                        .and_then(|message| message.as_string())
                })
                .unwrap_or_else(|| "font resolver failed".into())
        })?;
    if value.is_null() || value.is_undefined() {
        return Ok(None);
    }
    // The Node wrapper converts its opaque ROM-font handles to lookup names.
    if let Some(rom_name) = value.as_string() {
        return resolve_rom_font(&rom_name);
    }
    let bytes = value
        .dyn_into::<js_sys::Uint8Array>()
        .map_err(|_| "resolveFont must return Uint8Array, null, or undefined (not a Promise)")?;
    if bytes.length() > 16 * 1024 * 1024 {
        return Err("external font exceeds 16 MiB TrueType limit".into());
    }
    Face::truetype(data.insert(bytes.to_vec()), Hinting::Native).map(Some)
}

/// Validate a ROM lookup before the Node wrapper creates an opaque face handle.
#[wasm_bindgen]
pub fn has_rom_font(name: &str) -> Result<bool, JsError> {
    resolve_rom_font(name)
        .map(|face| face.is_some())
        .map_err(|error| JsError::new(&error))
}

#[wasm_bindgen]
pub struct EncodedLabel {
    body: Vec<u8>,
    warnings: Vec<String>,
    pub width: u32,
    pub height: u32,
    pub labels: u32,
}

#[wasm_bindgen]
impl EncodedLabel {
    pub fn take_body(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.body)
    }

    pub fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }
}

fn encode(
    mut document: Document,
    format: &str,
    label: Option<u32>,
) -> Result<EncodedLabel, String> {
    let labels = document.labels.len() as u32;
    let index = label.unwrap_or(0) as usize;
    let scene = document
        .labels
        .get(index)
        .ok_or("No label at this index. Include ^XA and ^XZ.")?;
    let (width, height) = (scene.width, scene.height);
    let body = match format {
        "png" => Png.encode(scene),
        "svg" => Svg.encode(scene),
        "pdf" if label.is_none() => Pdf.encode_pages(&document.labels),
        "pdf" => Pdf.encode(scene),
        _ => return Err("format must be png, svg, or pdf".into()),
    }
    .map_err(|error| error.to_string())?;
    Ok(EncodedLabel {
        body,
        warnings: std::mem::take(&mut document.warnings),
        width,
        height,
        labels,
    })
}

/// Omitted dimensions use the selected Rust profile's defaults. PDF includes all
/// labels unless an index is supplied. Input stays bytes across the JS boundary:
/// https://wasm-bindgen.github.io/wasm-bindgen/reference/types/number-slices.html
#[wasm_bindgen]
pub fn render_encoded(
    input: &[u8],
    width: Option<u32>,
    height: Option<u32>,
    dpi: Option<u32>,
    profile: &str,
    format: &str,
    label: Option<u32>,
    input_bytes: Option<u32>,
    font_bytes: Option<u32>,
    resolver: Option<js_sys::Function>,
) -> Result<EncodedLabel, JsError> {
    let defaults = match profile {
        "specification" => profiles::SPECIFICATION,
        "zd621" => profiles::ZD621_203_DPI,
        "zq610-plus" => profiles::ZQ610_PLUS_203_DPI,
        _ => return Err(JsError::new("Unknown rendering profile")),
    };
    let data = FontData::default();
    let mut fonts = Fonts::new();
    if let Some(callback) = resolver {
        let data = &data;
        // wasm-bindgen implements Send + Sync for JS values on our non-atomic
        // Wasm target. Capture the callback directly; nested renders own theirs.
        // https://docs.rs/wasm-bindgen/0.2.128/src/wasm_bindgen/lib.rs.html
        fonts.set_resolver(move |name| resolve(&callback, name, data));
    }
    let defaults_limits = Limits::default();
    let limits = Limits {
        input_bytes: input_bytes.map_or(defaults_limits.input_bytes, |n| n as usize),
        font_bytes: font_bytes.map_or(defaults_limits.font_bytes, |n| n as usize),
        ..defaults_limits
    };
    let document = zpl::render::render_with_fonts_and_limits(
        input,
        Options {
            width: width.unwrap_or(defaults.width),
            height: height.unwrap_or(defaults.height),
            dpi: dpi.unwrap_or(defaults.dpi),
            ..defaults
        },
        &fonts,
        limits,
    )
    .map_err(|error| JsError::new(&error.to_string()))?;
    encode(document, format, label).map_err(|error| JsError::new(&error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_outputs_match_native_adapters() {
        let input = b"^XA^PW32^LL24^FO2,2^GB8,6,2^FS^XZ^XA^PW48^LL16^XZ";
        for format in ["png", "svg", "pdf"] {
            for index in [None, Some(0), Some(1)] {
                let document = zpl::render(input, profiles::SPECIFICATION).unwrap();
                let scene = &document.labels[index.unwrap_or(0) as usize];
                let expected = match format {
                    "png" => Png.encode(scene).unwrap(),
                    "svg" => Svg.encode(scene).unwrap(),
                    _ if index.is_none() => Pdf.encode_pages(&document.labels).unwrap(),
                    _ => Pdf.encode(scene).unwrap(),
                };
                let result = encode(document, format, index).unwrap();
                assert_eq!(result.body, expected);
                assert_eq!(result.labels, 2);
                assert_eq!(
                    (result.width, result.height),
                    if index == Some(1) { (48, 16) } else { (32, 24) }
                );
            }
        }
        assert!(encode(
            zpl::render(input, profiles::SPECIFICATION).unwrap(),
            "png",
            Some(2)
        )
        .is_err());
    }
}
