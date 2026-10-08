//! Byte-preserving Node adapter using the same scenes and output adapters as Rust.
use wasm_bindgen::prelude::*;
use zpl::{
    output::{Adapter, Pdf, Png, Svg},
    render::{profiles, Document},
    Options,
};

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
) -> Result<EncodedLabel, JsError> {
    let defaults = match profile {
        "specification" => profiles::SPECIFICATION,
        "zd621" => profiles::ZD621_203_DPI,
        "zq610-plus" => profiles::ZQ610_PLUS_203_DPI,
        _ => return Err(JsError::new("Unknown rendering profile")),
    };
    let document = zpl::render(
        input,
        Options {
            width: width.unwrap_or(defaults.width),
            height: height.unwrap_or(defaults.height),
            dpi: dpi.unwrap_or(defaults.dpi),
            ..defaults
        },
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
