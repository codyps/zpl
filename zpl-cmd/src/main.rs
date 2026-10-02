//! Local command-line rendering; argument parsing follows Clap's derive API:
//! <https://docs.rs/clap/latest/clap/_derive/_tutorial/index.html>
use std::{error::Error, fs, path::PathBuf, process::ExitCode};

use clap::{Args, Parser, Subcommand, ValueEnum};
use zpl::{
    output::{Adapter, Pdf, Png, Svg},
    render::{profiles, render},
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
    let document = render(&input, options)?;
    for warning in &document.warnings {
        eprintln!("warning: {warning}");
    }
    let bytes = match extension {
        Some("pdf") => Pdf.encode_pages(&document.labels)?,
        _ => {
            if document.labels.len() != 1 {
                return Err(
                    "SVG/PNG output requires exactly one label; use PDF for multiple labels".into(),
                );
            }
            if extension == Some("svg") {
                Svg.encode(&document.labels[0])?
            } else {
                Png.encode(&document.labels[0])?
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
