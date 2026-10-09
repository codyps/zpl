//! Python adapter for the Rust parser, renderer and scene output adapters.
//! Ownership and interpreter detachment follow https://pyo3.rs/v0.29.3/.
mod config;

use config::{Compatibility, OutputLimits, RenderLimits, Syntax};
use pyo3::{
    create_exception,
    exceptions::{PyTypeError, PyValueError},
    prelude::*,
    types::{PyBytes, PyString},
};
use std::sync::Arc;
use zpl::{output, parse as framing, render as rendering};

create_exception!(zplkit, ParseError, PyValueError);
create_exception!(zplkit, RenderError, PyValueError);
create_exception!(zplkit, OutputError, PyValueError);

// Own the bytes before detaching from Python; bytearray/memoryview callers can
// explicitly snapshot their buffers with bytes(). Never decode binary ZPL.
fn source(input: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {
    if let Ok(bytes) = input.cast::<PyBytes>() {
        Ok(bytes.as_bytes().to_vec())
    } else if let Ok(text) = input.cast::<PyString>() {
        Ok(text.to_str()?.as_bytes().to_vec())
    } else {
        Err(PyTypeError::new_err("ZPL input must be str or bytes"))
    }
}

fn output_error(error: output::OutputError) -> PyErr {
    OutputError::new_err(error.to_string())
}

fn finite_limit(value: f64, name: &str) -> PyResult<()> {
    if !value.is_finite() || value < 0.0 {
        return Err(PyValueError::new_err(format!(
            "{name} must be finite and nonnegative"
        )));
    }
    Ok(())
}

fn output_limits(limits: Option<&OutputLimits>) -> PyResult<output::Limits> {
    let limits = limits.map_or_else(output::Limits::default, |v| v.inner);
    finite_limit(limits.coordinate_abs, "coordinate_abs")?;
    Ok(limits)
}

#[pyclass(frozen, from_py_object, module = "zplkit._native")]
#[derive(Clone, Copy)]
struct Options {
    inner: zpl::Options,
}

#[pymethods]
impl Options {
    #[new]
    #[pyo3(signature = (*, profile="zd621", width=None, height=None, dpi=None, compatibility=None))]
    fn new(
        profile: &str,
        width: Option<u32>,
        height: Option<u32>,
        dpi: Option<u32>,
        compatibility: Option<&Compatibility>,
    ) -> PyResult<Self> {
        let mut inner = match profile {
            "zd621" => rendering::profiles::ZD621_203_DPI,
            "specification" => rendering::profiles::SPECIFICATION,
            "zq610-plus" => rendering::profiles::ZQ610_PLUS_203_DPI,
            _ => {
                return Err(PyValueError::new_err(
                    "profile must be 'zd621', 'specification', or 'zq610-plus'",
                ))
            }
        };
        if let Some(n) = width {
            inner.width = n;
        }
        if let Some(n) = height {
            inner.height = n;
        }
        if let Some(n) = dpi {
            inner.dpi = n;
        }
        if let Some(c) = compatibility {
            inner.compatibility = c.inner;
        }
        if inner.width == 0 || inner.height == 0 || inner.dpi == 0 {
            return Err(PyValueError::new_err(
                "width, height, and dpi must be positive",
            ));
        }
        Ok(Self { inner })
    }
    #[getter]
    fn width(&self) -> u32 {
        self.inner.width
    }
    #[getter]
    fn height(&self) -> u32 {
        self.inner.height
    }
    #[getter]
    fn dpi(&self) -> u32 {
        self.inner.dpi
    }
    #[getter]
    fn compatibility(&self) -> Compatibility {
        Compatibility {
            inner: self.inner.compatibility,
        }
    }
    fn __repr__(&self) -> String {
        format!("{:?}", self.inner)
    }
}

#[pyclass(frozen, module = "zplkit._native")]
struct Scene {
    document: Arc<rendering::Document>,
    index: usize,
}
impl Scene {
    fn inner(&self) -> &output::Scene {
        &self.document.labels[self.index]
    }
}
#[pymethods]
impl Scene {
    #[getter]
    fn width(&self) -> u32 {
        self.inner().width
    }
    #[getter]
    fn height(&self) -> u32 {
        self.inner().height
    }
    #[getter]
    fn dpi(&self) -> u32 {
        self.inner().dpi
    }
    #[pyo3(signature = (*, limits=None))]
    fn png<'py>(
        &self,
        py: Python<'py>,
        limits: Option<&OutputLimits>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let limits = output_limits(limits)?;
        let bytes = py
            .detach(|| output::Png.encode_with_limits(self.inner(), limits))
            .map_err(output_error)?;
        Ok(PyBytes::new(py, &bytes))
    }
    #[pyo3(signature = (*, limits=None))]
    fn svg<'py>(
        &self,
        py: Python<'py>,
        limits: Option<&OutputLimits>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let limits = output_limits(limits)?;
        let bytes = py
            .detach(|| output::Svg.encode_with_limits(self.inner(), limits))
            .map_err(output_error)?;
        Ok(PyBytes::new(py, &bytes))
    }
    #[pyo3(signature = (*, limits=None))]
    fn pdf<'py>(
        &self,
        py: Python<'py>,
        limits: Option<&OutputLimits>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let limits = output_limits(limits)?;
        let bytes = py
            .detach(|| {
                output::Pdf.encode_pages_with_limits(std::slice::from_ref(self.inner()), limits)
            })
            .map_err(output_error)?;
        Ok(PyBytes::new(py, &bytes))
    }
    #[pyo3(signature = (*, limits=None))]
    fn rasterize(&self, py: Python<'_>, limits: Option<&OutputLimits>) -> PyResult<Raster> {
        let limits = output_limits(limits)?;
        let inner = py
            .detach(|| output::raster::rasterize_with_limits(self.inner(), limits))
            .map_err(output_error)?;
        Ok(Raster { inner })
    }
}

#[pyclass(frozen, module = "zplkit._native")]
struct Raster {
    inner: output::raster::Raster,
}
#[pymethods]
impl Raster {
    #[getter]
    fn width(&self) -> u32 {
        self.inner.width
    }
    #[getter]
    fn height(&self) -> u32 {
        self.inner.height
    }
    /// Row-major grayscale: one byte per dot, 0 black and 255 white.
    #[getter]
    fn pixels<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.inner.pixels)
    }
}

#[pyclass(frozen, module = "zplkit._native")]
struct Document {
    inner: Arc<rendering::Document>,
}
#[pymethods]
impl Document {
    #[getter]
    fn labels(&self) -> Vec<Scene> {
        (0..self.inner.labels.len())
            .map(|index| Scene {
                document: self.inner.clone(),
                index,
            })
            .collect()
    }
    #[getter]
    fn warnings(&self) -> Vec<String> {
        self.inner.warnings.clone()
    }
    #[pyo3(signature = (*, limits=None))]
    fn pdf<'py>(
        &self,
        py: Python<'py>,
        limits: Option<&OutputLimits>,
    ) -> PyResult<Bound<'py, PyBytes>> {
        let limits = output_limits(limits)?;
        let bytes = py
            .detach(|| output::Pdf.encode_pages_with_limits(&self.inner.labels, limits))
            .map_err(output_error)?;
        Ok(PyBytes::new(py, &bytes))
    }
}

/// Render UTF-8 text or raw ZPL bytes into label scenes and warnings.
#[pyfunction]
#[pyo3(signature = (input, options=None, *, limits=None))]
fn render(
    py: Python<'_>,
    input: &Bound<'_, PyAny>,
    options: Option<&Options>,
    limits: Option<&RenderLimits>,
) -> PyResult<Document> {
    let input = source(input)?;
    let options = options.map_or_else(zpl::Options::default, |v| v.inner);
    let limits = limits.map_or_else(rendering::Limits::default, |v| v.inner);
    finite_limit(limits.number_abs, "number_abs")?;
    finite_limit(limits.coordinate_abs, "coordinate_abs")?;
    match py.detach(|| rendering::render_with_limits(&input, options, limits)) {
        Ok(inner) => Ok(Document {
            inner: Arc::new(inner),
        }),
        Err(error) => {
            let exception = RenderError::new_err(error.to_string());
            exception.value(py).setattr("offset", error.offset)?;
            exception.value(py).setattr("message", error.message)?;
            Err(exception)
        }
    }
}

#[pyclass(frozen, module = "zplkit._native")]
struct Element {
    #[pyo3(get)]
    kind: &'static str,
    #[pyo3(get)]
    offset: usize,
    // Return bytes explicitly; Vec<u8> would otherwise become a Python list.
    data: Vec<u8>,
}
#[pymethods]
impl Element {
    #[getter]
    fn data<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        PyBytes::new(py, &self.data)
    }
    fn __bytes__<'py>(&self, py: Python<'py>) -> Bound<'py, PyBytes> {
        self.data(py)
    }
}

#[pyclass(frozen, module = "zplkit._native")]
struct ParseResult {
    elements: Vec<Py<Element>>,
    #[pyo3(get)]
    syntax: Syntax,
}

#[pymethods]
impl ParseResult {
    #[getter]
    fn elements(&self, py: Python<'_>) -> Vec<Py<Element>> {
        self.elements.iter().map(|e| e.clone_ref(py)).collect()
    }
}

/// Frame a complete stream losslessly, returning byte offsets and final syntax.
#[pyfunction]
#[pyo3(signature = (input, *, syntax=None))]
fn parse(
    py: Python<'_>,
    input: &Bound<'_, PyAny>,
    syntax: Option<&Syntax>,
) -> PyResult<ParseResult> {
    let input = source(input)?;
    let initial = syntax.map_or_else(framing::Syntax::default, |v| v.inner);
    let parsed = py.detach(|| {
        let mut parser = framing::ParseContext::with_syntax(&input, initial);
        let mut elements = Vec::new();
        loop {
            let offset = parser.position();
            let Some(element) = parser.next() else { break };
            let element = element?;
            let kind = match element {
                framing::Element::BeforeFirstCommand(_) => "before_first_command",
                framing::Element::FormatCommand(_) => "format_command",
                framing::Element::ControlCommand(_) => "control_command",
                framing::Element::ControlCharacter(_) => "control_character",
            };
            elements.push(Element {
                kind,
                offset,
                data: element.as_bytes().to_vec(),
            });
        }
        Ok::<_, framing::ParseError>((elements, parser.syntax()))
    });
    match parsed {
        Ok((elements, syntax)) => Ok(ParseResult {
            elements: elements
                .into_iter()
                .map(|e| Py::new(py, e))
                .collect::<PyResult<_>>()?,
            syntax: Syntax { inner: syntax },
        }),
        Err(error) => {
            let exception = ParseError::new_err(error.to_string());
            exception.value(py).setattr("offset", error.offset)?;
            exception
                .value(py)
                .setattr("kind", format!("{:?}", error.kind))?;
            Err(exception)
        }
    }
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("library_version", zpl::version::VERSION)?;
    m.py()
        .get_type::<ParseError>()
        .setattr("__module__", "zplkit.parse")?;
    m.py()
        .get_type::<RenderError>()
        .setattr("__module__", "zplkit.rendering")?;
    m.py()
        .get_type::<OutputError>()
        .setattr("__module__", "zplkit.output")?;
    m.add("ParseError", m.py().get_type::<ParseError>())?;
    m.add("RenderError", m.py().get_type::<RenderError>())?;
    m.add("OutputError", m.py().get_type::<OutputError>())?;
    m.add_class::<Options>()?;
    m.add_class::<Compatibility>()?;
    m.add_class::<RenderLimits>()?;
    m.add_class::<OutputLimits>()?;
    m.add_class::<Syntax>()?;
    m.add_class::<Element>()?;
    m.add_class::<ParseResult>()?;
    m.add_class::<Document>()?;
    m.add_class::<Scene>()?;
    m.add_class::<Raster>()?;
    m.add_function(wrap_pyfunction!(render, m)?)?;
    m.add_function(wrap_pyfunction!(parse, m)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use output::Adapter;

    #[test]
    fn python_outputs_match_native_adapters_for_every_profile() {
        Python::initialize();
        Python::attach(|py| {
            let input = b"^XA^FO2,3^GB10,8,2^FS^XZ^XA^FO1,1^GB3,4,1^FS^XZ";
            for profile in ["specification", "zd621", "zq610-plus"] {
                let options = Options::new(profile, Some(32), Some(24), None, None).unwrap();
                let native = zpl::render(input, options.inner).unwrap();
                let bound =
                    render(py, PyBytes::new(py, input).as_any(), Some(&options), None).unwrap();
                assert_eq!(bound.warnings(), native.warnings);
                assert_eq!(
                    bound.pdf(py, None).unwrap().as_bytes(),
                    output::Pdf.encode_pages(&native.labels).unwrap()
                );
                for (scene, expected) in bound.labels().iter().zip(&native.labels) {
                    assert_eq!(
                        scene.png(py, None).unwrap().as_bytes(),
                        output::Png.encode(expected).unwrap()
                    );
                    assert_eq!(
                        scene.svg(py, None).unwrap().as_bytes(),
                        output::Svg.encode(expected).unwrap()
                    );
                    assert_eq!(
                        scene.pdf(py, None).unwrap().as_bytes(),
                        output::Pdf.encode(expected).unwrap()
                    );
                    assert_eq!(
                        scene.rasterize(py, None).unwrap().inner.pixels,
                        output::raster::rasterize(expected).unwrap().pixels
                    );
                }
            }
        });
    }
}
