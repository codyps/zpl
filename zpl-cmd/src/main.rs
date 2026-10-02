//! Local command-line rendering; argument parsing follows Clap's derive API:
//! <https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html>
use std::{error::Error, fs, path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand, ValueEnum};
use zpl::{
    output::{Limits as OutputLimits, Pdf, Png, Svg},
    render::{profiles, render_with_limits, Limits},
    Options,
};

#[derive(Parser)]
#[command(
    version,
    about = "Render ZPL labels locally without a printer or external service"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Render a ZPL file to PNG, SVG or PDF, selected by the output extension.
    ///
    /// Resource budgets are unlimited unless a --max-* option is supplied.
    Render(RenderArgs),
}

#[derive(Args)]
struct RenderArgs {
    /// Input ZPL file.
    input: PathBuf,
    /// Output .png or .svg (one label), or .pdf (one page per label).
    output: PathBuf,
    /// Printer compatibility profile.
    #[arg(long, value_enum, default_value_t = Profile::Zd621)]
    profile: Profile,
    /// Honor explicit QR mask operands instead of the printer's mask selection.
    #[arg(long)]
    explicit_qr_mask: bool,
    /// Optional resource budgets; omitted budgets are unlimited.
    #[command(flatten)]
    limits: LimitArgs,
}

#[derive(Args, Default)]
struct LimitArgs {
    /// Maximum source bytes and expanded stored-format bytes (each).
    #[arg(long)]
    max_input_bytes: Option<usize>,
    /// Maximum labels, also bounding PDF pages.
    #[arg(long)]
    max_labels: Option<usize>,
    /// Maximum total scene path segments across labels.
    #[arg(long)]
    max_segments: Option<usize>,
    /// Maximum path segments retained in downloaded graphics.
    #[arg(long)]
    max_stored_graphic_segments: Option<usize>,
    /// Maximum pixels per label.
    #[arg(long)]
    max_pixels: Option<usize>,
    /// Maximum width or height in dots, including ZPL overrides.
    #[arg(long)]
    max_dimension: Option<u32>,
    /// Maximum absolute numeric ZPL operand.
    #[arg(long)]
    max_number: Option<f64>,
    /// Maximum decoded field bytes, including numbered/concatenated fields.
    #[arg(long)]
    max_field_bytes: Option<usize>,
    /// Maximum decoded bytes per inline or downloaded graphic.
    #[arg(long)]
    max_graphic_bytes: Option<usize>,
    /// Maximum stored format definitions.
    #[arg(long)]
    max_stored_formats: Option<usize>,
    /// Maximum stored format recall nesting depth.
    #[arg(long)]
    max_recall_depth: Option<usize>,
    /// Maximum stored format recalls across the input.
    #[arg(long)]
    max_recall_calls: Option<usize>,
    /// Maximum absolute path coordinate in dots.
    #[arg(long)]
    max_coordinate: Option<f64>,
    /// Maximum flattened raster edges per draw (PNG).
    #[arg(long)]
    max_flattened_segments: Option<usize>,
    /// Maximum cumulative edge/scanline visits (PNG).
    #[arg(long)]
    max_scan_work: Option<u64>,
}

impl LimitArgs {
    fn budgets(&self) -> Result<(Limits, OutputLimits), Box<dyn Error>> {
        let mut render = Limits::unlimited();
        let mut output = OutputLimits::unlimited();
        if let Some(n) = self.max_input_bytes {
            render.input_bytes = n;
        }
        if let Some(n) = self.max_labels {
            render.labels = n;
            output.pages = n;
        }
        if let Some(n) = self.max_segments {
            render.segments = n;
            output.segments = n;
        }
        if let Some(n) = self.max_stored_graphic_segments {
            render.stored_graphic_segments = n;
        }
        if let Some(n) = self.max_pixels {
            render.pixels = n;
            output.pixels = n;
        }
        if let Some(n) = self.max_dimension {
            render.dimension = n;
        }
        if let Some(n) = self.max_number {
            if !n.is_finite() || n < 0. {
                return Err("max-number must be finite and nonnegative".into());
            }
            render.number_abs = n;
        }
        if let Some(n) = self.max_field_bytes {
            render.field_bytes = n;
        }
        if let Some(n) = self.max_graphic_bytes {
            render.graphic_bytes = n;
        }
        if let Some(n) = self.max_stored_formats {
            render.stored_formats = n;
        }
        if let Some(n) = self.max_recall_depth {
            render.recall_depth = n;
        }
        if let Some(n) = self.max_recall_calls {
            render.recall_calls = n;
        }
        if let Some(n) = self.max_coordinate {
            if !n.is_finite() || n < 0. {
                return Err("max-coordinate must be finite and nonnegative".into());
            }
            render.coordinate_abs = n;
            output.coordinate_abs = n;
        }
        if let Some(n) = self.max_flattened_segments {
            output.flattened_segments = n;
        }
        if let Some(n) = self.max_scan_work {
            output.scan_work = n;
        }
        Ok((render, output))
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Profile {
    Zd621,
    Zd621Preview,
    Zq610Plus,
    Specification,
}

impl Profile {
    fn options(self) -> Options {
        match self {
            Self::Zd621 => profiles::ZD621_203_DPI,
            Self::Zd621Preview => {
                let mut options = profiles::ZD621_203_DPI;
                options.compatibility.preview_width_quantum = Some(64);
                options.compatibility.preview_width_latched_at_first_draw = true;
                options
            }
            Self::Zq610Plus => profiles::ZQ610_PLUS_203_DPI,
            Self::Specification => profiles::SPECIFICATION,
        }
    }
}

fn render_file(args: RenderArgs) -> Result<(), Box<dyn Error>> {
    let extension = args.output.extension().and_then(|s| s.to_str());
    if !matches!(extension, Some("png" | "svg" | "pdf")) {
        return Err("output extension must be svg, png or pdf".into());
    }
    let mut options = args.profile.options();
    if args.explicit_qr_mask {
        options.compatibility.qr_printer_mask_selection = false;
    }
    let input = fs::read(&args.input)
        .map_err(|error| format!("reading {}: {error}", args.input.display()))?;
    let (render_limits, output_limits) = args.limits.budgets()?;
    let document = render_with_limits(&input, options, render_limits)?;
    for warning in &document.warnings {
        eprintln!("warning: {warning}");
    }
    let bytes = match extension {
        Some("pdf") => Pdf.encode_pages_with_limits(&document.labels, output_limits)?,
        _ => {
            if document.labels.len() != 1 {
                return Err(
                    "SVG/PNG output requires exactly one label; use PDF for multiple labels".into(),
                );
            }
            if extension == Some("svg") {
                Svg.encode_with_limits(&document.labels[0], output_limits)?
            } else {
                Png.encode_with_limits(&document.labels[0], output_limits)?
            }
        }
    };
    fs::write(&args.output, bytes)
        .map_err(|error| format!("writing {}: {error}", args.output.display()))?;
    Ok(())
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Render(args) => render_file(args),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("zpl-cmd: {error}");
            ExitCode::FAILURE
        }
    }
}
