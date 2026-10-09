//! Rustler boundary: https://docs.rs/rustler/0.38.0/rustler/.
//! Lengthy work uses dirty CPU schedulers, including parsing and term encoding:
//! https://www.erlang.org/doc/apps/erts/erl_nif.html#lengthy_work
mod config;

use config::{decode, Fields};
use rustler::{Atom, Binary, Encoder, Env, NifStruct, OwnedBinary, ResourceArc, Term};
use std::sync::Arc;
use zpl::{output, parse as framing, render as rendering};

mod atoms {
    rustler::atoms! {
        argument, parse, render, output, profile, specification, zd621, zq610_plus,
        options, compatibility, render_limits, output_limits, syntax,
        before_first_command, format_command, control_command, control_character,
        png, svg, pdf
    }
}

#[derive(NifStruct)]
#[module = "Zpl.Error"]
pub struct BindingError {
    __exception__: bool,
    stage: Atom,
    message: String,
    offset: Option<usize>,
    kind: Option<String>,
}

fn failure(
    stage: &str,
    message: impl Into<String>,
    offset: Option<usize>,
    kind: Option<String>,
) -> BindingError {
    BindingError {
        __exception__: true,
        stage: match stage {
            "parse" => atoms::parse(),
            "render" => atoms::render(),
            "output" => atoms::output(),
            _ => atoms::argument(),
        },
        message: message.into(),
        offset,
        kind,
    }
}

fn output_error(error: output::OutputError) -> BindingError {
    failure("output", error.to_string(), None, None)
}

fn binary<'a>(env: Env<'a>, bytes: &[u8]) -> Result<Binary<'a>, BindingError> {
    let mut result = OwnedBinary::new(bytes.len())
        .ok_or_else(|| failure("output", "binary allocation failed", None, None))?;
    result.as_mut_slice().copy_from_slice(bytes);
    Ok(result.release(env))
}

fn finite(value: f64, name: &str) -> Result<(), BindingError> {
    if !value.is_finite() || value < 0.0 {
        return Err(failure(
            "argument",
            format!("{name} must be finite and nonnegative"),
            None,
            None,
        ));
    }
    Ok(())
}

fn profile(profile: Atom) -> Result<zpl::Options, BindingError> {
    if profile == atoms::zd621() {
        Ok(rendering::profiles::ZD621_203_DPI)
    } else if profile == atoms::specification() {
        Ok(rendering::profiles::SPECIFICATION)
    } else if profile == atoms::zq610_plus() {
        Ok(rendering::profiles::ZQ610_PLUS_203_DPI)
    } else {
        Err(failure("argument", "unknown profile", None, None))
    }
}

fn options(fields: Fields<'_>) -> Result<zpl::Options, BindingError> {
    let selected = fields
        .get(&atoms::profile())
        .map(|v| decode(*v, "profile"))
        .transpose()?
        .unwrap_or(atoms::zd621());
    let mut options = profile(selected)?;
    for (key, value) in fields {
        match key
            .to_term(value.get_env())
            .atom_to_string()
            .unwrap()
            .as_str()
        {
            "profile" => (),
            "width" => options.width = decode(value, "width")?,
            "height" => options.height = decode(value, "height")?,
            "dpi" => options.dpi = decode(value, "dpi")?,
            "compatibility" => {
                options.compatibility =
                    config::compatibility(options.compatibility, decode(value, "compatibility")?)?
            }
            key => {
                return Err(failure(
                    "argument",
                    format!("unknown option: {key}"),
                    None,
                    None,
                ))
            }
        }
    }
    if options.width == 0 || options.height == 0 || options.dpi == 0 {
        return Err(failure(
            "argument",
            "width, height, and dpi must be positive",
            None,
            None,
        ));
    }
    Ok(options)
}

#[rustler::nif]
fn library_version() -> &'static str {
    zpl::version::VERSION
}

#[rustler::nif]
fn defaults<'a>(env: Env<'a>, kind: Atom, selected: Atom) -> Result<Term<'a>, BindingError> {
    let options = profile(selected)?;
    if kind == atoms::options() {
        let pairs = [
            ("width", options.width.encode(env)),
            ("height", options.height.encode(env)),
            ("dpi", options.dpi.encode(env)),
            (
                "compatibility",
                config::compatibility_fields(env, options.compatibility),
            ),
        ];
        let pairs: Vec<_> = pairs
            .into_iter()
            .map(|(k, v)| (Atom::from_str(env, k).unwrap(), v))
            .collect();
        Ok(Term::map_from_pairs(env, &pairs).unwrap())
    } else if kind == atoms::compatibility() {
        Ok(config::compatibility_fields(env, Default::default()))
    } else if kind == atoms::render_limits() {
        Ok(config::render_limits_fields(env, Default::default()))
    } else if kind == atoms::output_limits() {
        Ok(config::output_limits_fields(env, Default::default()))
    } else if kind == atoms::syntax() {
        Ok(config::syntax_fields(env, Default::default()))
    } else {
        Err(failure("argument", "unknown configuration", None, None))
    }
}

struct DocumentResource(Arc<rendering::Document>);
#[rustler::resource_impl]
impl rustler::Resource for DocumentResource {}
struct SceneResource {
    document: Arc<rendering::Document>,
    index: usize,
}
#[rustler::resource_impl]
impl rustler::Resource for SceneResource {}
impl SceneResource {
    fn scene(&self) -> &output::Scene {
        &self.document.labels[self.index]
    }
}

#[derive(NifStruct)]
#[module = "Zpl.Document"]
struct Document {
    labels: Vec<Scene>,
    warnings: Vec<String>,
    reference: ResourceArc<DocumentResource>,
}
#[derive(NifStruct)]
#[module = "Zpl.Scene"]
struct Scene {
    width: u32,
    height: u32,
    dpi: u32,
    reference: ResourceArc<SceneResource>,
}
#[derive(NifStruct)]
#[module = "Zpl.Raster"]
struct Raster<'a> {
    width: u32,
    height: u32,
    pixels: Binary<'a>,
}

#[rustler::nif(schedule = "DirtyCpu")]
fn render(
    input: Binary<'_>,
    fields: Fields<'_>,
    limits: Fields<'_>,
) -> Result<Document, BindingError> {
    let options = options(fields)?;
    let limits = config::render_limits(Default::default(), limits)?;
    finite(limits.number_abs, "number_abs")?;
    finite(limits.coordinate_abs, "coordinate_abs")?;
    let document = rendering::render_with_limits(input.as_slice(), options, limits)
        .map_err(|e| failure("render", e.message, Some(e.offset), None))?;
    let document = Arc::new(document);
    let labels = document
        .labels
        .iter()
        .enumerate()
        .map(|(index, s)| Scene {
            width: s.width,
            height: s.height,
            dpi: s.dpi,
            reference: ResourceArc::new(SceneResource {
                document: document.clone(),
                index,
            }),
        })
        .collect();
    Ok(Document {
        labels,
        warnings: document.warnings.clone(),
        reference: ResourceArc::new(DocumentResource(document)),
    })
}

fn output_limits(fields: Fields<'_>) -> Result<output::Limits, BindingError> {
    let limits = config::output_limits(Default::default(), fields)?;
    finite(limits.coordinate_abs, "coordinate_abs")?;
    Ok(limits)
}

fn encode(
    scene: &output::Scene,
    format: Atom,
    limits: output::Limits,
) -> Result<Vec<u8>, BindingError> {
    if format == atoms::png() {
        output::Png
            .encode_with_limits(scene, limits)
            .map_err(output_error)
    } else if format == atoms::svg() {
        output::Svg
            .encode_with_limits(scene, limits)
            .map_err(output_error)
    } else if format == atoms::pdf() {
        output::Pdf
            .encode_pages_with_limits(std::slice::from_ref(scene), limits)
            .map_err(output_error)
    } else {
        Err(failure(
            "argument",
            "format must be :png, :svg, or :pdf",
            None,
            None,
        ))
    }
}

#[rustler::nif(schedule = "DirtyCpu")]
fn scene_encode<'a>(
    env: Env<'a>,
    scene: ResourceArc<SceneResource>,
    format: Atom,
    limits: Fields<'a>,
) -> Result<Binary<'a>, BindingError> {
    binary(env, &encode(scene.scene(), format, output_limits(limits)?)?)
}

#[rustler::nif(schedule = "DirtyCpu")]
fn document_pdf<'a>(
    env: Env<'a>,
    document: ResourceArc<DocumentResource>,
    limits: Fields<'a>,
) -> Result<Binary<'a>, BindingError> {
    let bytes = output::Pdf
        .encode_pages_with_limits(&document.0.labels, output_limits(limits)?)
        .map_err(output_error)?;
    binary(env, &bytes)
}

#[rustler::nif(schedule = "DirtyCpu")]
fn rasterize<'a>(
    env: Env<'a>,
    scene: ResourceArc<SceneResource>,
    limits: Fields<'a>,
) -> Result<Raster<'a>, BindingError> {
    let raster = output::raster::rasterize_with_limits(scene.scene(), output_limits(limits)?)
        .map_err(output_error)?;
    Ok(Raster {
        width: raster.width,
        height: raster.height,
        pixels: binary(env, &raster.pixels)?,
    })
}

#[derive(NifStruct)]
#[module = "Zpl.Element"]
struct Element<'a> {
    kind: Atom,
    offset: usize,
    data: Binary<'a>,
}
#[derive(NifStruct)]
#[module = "Zpl.ParseResult"]
struct ParseResult<'a> {
    elements: Vec<Element<'a>>,
    syntax: Term<'a>,
}

#[rustler::nif(schedule = "DirtyCpu")]
fn parse<'a>(
    env: Env<'a>,
    input: Binary<'a>,
    syntax: Fields<'a>,
) -> Result<ParseResult<'a>, BindingError> {
    let syntax = config::syntax(Default::default(), syntax)?;
    let mut parser = framing::ParseContext::with_syntax(input.as_slice(), syntax);
    let mut elements = Vec::new();
    loop {
        let offset = parser.position();
        let Some(element) = parser.next() else { break };
        let element = element.map_err(|e| {
            failure(
                "parse",
                e.to_string(),
                Some(e.offset),
                Some(format!("{:?}", e.kind)),
            )
        })?;
        let kind = match element {
            framing::Element::BeforeFirstCommand(_) => atoms::before_first_command(),
            framing::Element::FormatCommand(_) => atoms::format_command(),
            framing::Element::ControlCommand(_) => atoms::control_command(),
            framing::Element::ControlCharacter(_) => atoms::control_character(),
        };
        // A sub-binary retains the original BEAM binary without UTF-8 conversion.
        let data = input
            .make_subbinary(offset, element.as_bytes().len())
            .map_err(|_| failure("parse", "invalid element span", Some(offset), None))?;
        elements.push(Element { kind, offset, data });
    }
    Ok(ParseResult {
        elements,
        syntax: config::syntax_fields(env, parser.syntax()),
    })
}

rustler::init!("Elixir.Zpl.Native");
