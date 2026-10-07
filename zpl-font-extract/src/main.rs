use clap::{Args, Parser, Subcommand};
use eyre::{ensure, Result};
use std::{fs, path::PathBuf};
use zpl_font_extract::automatic::{
    self,
    capture::Printer,
    compile::compile,
    model::{write_json, Document},
    probe::Config,
};

#[derive(Parser)]
#[command(
    about = "Recover bitmap fonts from Zebra HTTP previews, verify JSON, and compile compact Rust tables"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Complete raw font inventory and multiple encoding maps (v2 JSON).
    Collection {
        #[command(subcommand)]
        command: zpl_font_extract::collection::cli::Command,
    },
    /// Calibrate, capture/cache, extract, verify fonts.json and compile Rust tables.
    Recover {
        #[command(flatten)]
        probes: Probes,
        /// Printer HTTP(S) origin. Also identifies the printer cache in offline mode.
        #[arg(long)]
        host: String,
        #[arg(long)]
        cache: PathBuf,
        /// New output directory; existing outputs are never overwritten.
        #[arg(long)]
        out: PathBuf,
        /// Replay cached evidence only; fail if any required preview is absent.
        #[arg(long)]
        offline: bool,
        #[arg(long, default_value_t = 5.0)]
        delay: f64,
        #[arg(long, default_value_t = 30)]
        timeout: u64,
    },
    /// Save the initial calibration ZPL/plan without printer access.
    Plan {
        #[command(flatten)]
        probes: Probes,
        #[arg(long)]
        out: PathBuf,
    },
    /// Validate an existing verified fonts.json and emit fonts.rs/bitmaps.bin.
    Compile {
        source: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long = "font")]
        fonts: Vec<String>,
    },
}
#[derive(Args)]
struct Probes {
    /// Repeat or comma-separate resident A-H/@ or named R:NAME.FNT selectors.
    #[arg(
        long = "font",
        value_delimiter = ',',
        default_value = "A,B,C,D,E,F,G,H,@"
    )]
    fonts: Vec<String>,
    /// Decimal input/source keys, comma separated. Defaults to printable ASCII.
    #[arg(long, value_delimiter = ',')]
    codes: Vec<u8>,
    #[arg(long, default_value_t = 27)]
    encoding: u8,
    /// Use CI0 source positions (requires --encoding 0); omit TAB formatting.
    #[arg(long)]
    source: bool,
    #[arg(long, default_value_t = 128)]
    bound: u16,
    #[arg(long, default_value_t = 832)]
    width: u32,
    #[arg(long, default_value_t = 4096)]
    height: u32,
    /// Two visible control keys for calibration; default A and B.
    #[arg(long, value_delimiter = ',', default_value = "65,66")]
    probes: Vec<u8>,
}
impl Probes {
    fn config(self) -> Result<Config> {
        ensure!(self.probes.len() == 2, "exactly two probe keys required");
        let c = Config {
            fonts: self.fonts,
            codes: if self.codes.is_empty() {
                if self.source {
                    (0..=255).collect()
                } else {
                    (32..=126).collect()
                }
            } else {
                self.codes
            },
            encoding: self.encoding,
            source: self.source,
            bound: self.bound,
            width: self.width,
            height: self.height,
            probes: [self.probes[0], self.probes[1]],
        };
        c.validate()?;
        Ok(c)
    }
}
#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Collection { command } => zpl_font_extract::collection::cli::run(command).await?,
        Command::Recover {
            probes,
            host,
            cache,
            out,
            offline,
            delay,
            timeout,
        } => {
            ensure!(!out.exists(), "output exists: {}", out.display());
            let c = probes.config()?;
            let mut printer = Printer::new(&host, &cache, offline, delay, timeout)?;
            let document = automatic::recover(&c, &mut printer).await?;
            fs::create_dir_all(&out)?;
            write_json(&out.join("fonts.json"), &document)?;
            compile(&document, &out.join("rust"))?;
            println!(
                "Verified {} fonts: {}",
                document.fonts.len(),
                out.join("fonts.json").display()
            );
        }
        Command::Plan { probes, out } => {
            ensure!(!out.exists(), "output exists: {}", out.display());
            let pages = automatic::initial_pages(&probes.config()?)?;
            fs::create_dir_all(&out)?;
            for (i, page) in pages.iter().enumerate() {
                write_json(&out.join(format!("page-{i:03}.json")), page)?;
                fs::write(out.join(format!("page-{i:03}.zpl")), page.zpl()?)?;
            }
            println!(
                "{} initial calibration pages; refinement is adaptive",
                pages.len()
            );
        }
        Command::Compile { source, out, fonts } => {
            let mut document = Document::load(&source)?;
            if !fonts.is_empty() {
                ensure!(
                    fonts
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        == fonts.len()
                        && fonts
                            .iter()
                            .all(|name| document.fonts.iter().any(|f| &f.name == name)),
                    "font selection must contain unique known names"
                );
                document.fonts.retain(|f| fonts.contains(&f.name));
                document.verification["content_sha256"] =
                    serde_json::json!(document.content_hash()?);
            }
            compile(&document, &out)?;
            println!(
                "Compiled {} fonts into {}",
                document.fonts.len(),
                out.display()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comma_separated_probe_keys_match_documented_cli() {
        let cli = Cli::try_parse_from([
            "zpl-font-extract",
            "plan",
            "--font",
            "Z:A.FNT",
            "--source",
            "--encoding",
            "0",
            "--probes",
            "48,49",
            "--out",
            "unused",
        ])
        .unwrap();
        let Command::Plan { probes, .. } = cli.command else {
            unreachable!()
        };
        assert_eq!(probes.config().unwrap().probes, [48, 49]);
    }
}
