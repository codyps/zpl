//! JavaScript adapters. JS bindings: https://wasm-bindgen.github.io/wasm-bindgen/
mod encode;
mod parse;
use wasm_bindgen::prelude::*;
use zpl::{
    output::{Adapter, Pdf, Png, Svg},
    render, Options,
};

/// Version of the linked renderer, rather than the browser adapter package.
#[wasm_bindgen]
pub fn library_version() -> String {
    zpl::version::VERSION.into()
}

#[wasm_bindgen]
pub struct Preview {
    png: Vec<u8>,
    svg: Vec<u8>,
    pdf: Vec<u8>,
    warnings: String,
    pub width: u32,
    pub height: u32,
    pub labels: u32,
}

#[wasm_bindgen]
impl Preview {
    pub fn png(&self) -> Vec<u8> {
        self.png.clone()
    }
    pub fn svg(&self) -> Vec<u8> {
        self.svg.clone()
    }
    pub fn pdf(&self) -> Vec<u8> {
        self.pdf.clone()
    }
    pub fn warnings(&self) -> String {
        self.warnings.clone()
    }
}

fn preview(input: &str, width: u32, height: u32, dpi: u32, label: u32) -> Result<Preview, String> {
    if width == 0 || height == 0 || width > 4096 || height > 4096 || !(1..=1200).contains(&dpi) {
        return Err("Use dimensions from 1 to 4096 dots and DPI from 1 to 1200.".into());
    }
    let document = render(
        input.as_bytes(),
        Options {
            width,
            height,
            dpi,
            ..Options::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let scene = document
        .labels
        .get(label as usize)
        .ok_or("No label at this index. Include ^XA and ^XZ.")?;
    // ZPL can override the default dimensions with ^PW/^LL.
    if scene.width > 4096 || scene.height > 4096 {
        return Err("ZPL label exceeds the browser limit of 4096 dots per side.".into());
    }
    Ok(Preview {
        png: Png.encode(scene).map_err(|e| e.to_string())?,
        svg: Svg.encode(scene).map_err(|e| e.to_string())?,
        pdf: Pdf.encode(scene).map_err(|e| e.to_string())?,
        warnings: document.warnings.join("\n"),
        width: scene.width,
        height: scene.height,
        labels: document.labels.len() as u32,
    })
}

#[wasm_bindgen]
pub fn render_preview(
    input: &str,
    width: u32,
    height: u32,
    dpi: u32,
    label: u32,
) -> Result<Preview, JsError> {
    preview(input, width, height, dpi, label).map_err(|e| JsError::new(&e))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_native_adapters() {
        let input = "^XA^FO10,10^GB40,20,3^FS^XZ";
        let result = preview(input, 100, 80, 203, 0).unwrap();
        let doc = render(
            input.as_bytes(),
            Options {
                width: 100,
                height: 80,
                dpi: 203,
                ..zpl::render::profiles::SPECIFICATION
            },
        )
        .unwrap();
        assert_eq!(result.png, Png.encode(&doc.labels[0]).unwrap());
        assert_eq!(result.svg, Svg.encode(&doc.labels[0]).unwrap());
        assert_eq!(result.pdf, Pdf.encode(&doc.labels[0]).unwrap());
        assert_eq!((result.width, result.height, result.labels), (100, 80, 1));
    }
    #[test]
    fn bounds_and_multiple_labels() {
        assert!(preview("^XA^XZ", 0, 80, 203, 0).is_err());
        assert!(preview("^XA^PW5000^XZ", 100, 80, 203, 0).is_err());
        assert!(preview("", 100, 80, 203, 0).is_err());
        assert!(preview(&"x".repeat(1_048_577), 100, 80, 203, 0).is_err());
        let input = "^XA^XZ^XA^FO2,2^GB5,5,2^FS^XZ";
        assert_eq!(preview(input, 100, 80, 203, 1).unwrap().labels, 2);
        assert!(preview(input, 100, 80, 203, 2).is_err());
    }
}
