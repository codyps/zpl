use clap::{Args, Parser, Subcommand};
use eyre::{ensure, Result};
use std::{fs, path::PathBuf};
use zpl_font_extract::{
    automatic::{
        self,
        capture::Printer,
        model::{read, write_json},
        probe::Config,
        RecoveryConfig,
    },
    collection::{compile, evidence, identity, Collection, Encoding},
};

#[derive(Parser)]
#[command(
    about = "Automatically recover glyphs and encodings, verify JSON, and compile compact tables"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Discover fonts, recover glyphs and encoding maps, verify, and compile.
    Recover {
        #[command(flatten)]
        probes: Probes,
        /// Optional portable or verified legacy JSON; omitted discovers printer fonts.
        #[arg(long)]
        source: Option<PathBuf>,
        #[arg(long)]
        host: String,
        #[arg(long)]
        cache: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        offline: bool,
        #[arg(long, default_value_t = 5.0)]
        delay: f64,
        #[arg(long, default_value_t = 30)]
        timeout: u64,
        #[arg(long, default_value_t = 9200)]
        identity_port: u16,
        #[arg(long, default_value_t = 9100)]
        sgd_port: u16,
    },
    /// Inspect initial calibration pages; later recovery pages are adaptive.
    Plan {
        #[command(flatten)]
        probes: Probes,
        #[arg(long)]
        out: PathBuf,
    },
    /// Compile current JSON, automatically migrating verified legacy input.
    Compile {
        source: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long = "font")]
        fonts: Vec<String>,
    },
    /// Merge existing measured observations or unverified candidate reports.
    MergeEvidence {
        source: PathBuf,
        #[arg(long, required = true)]
        report: Vec<PathBuf>,
        #[arg(long, default_value = ".")]
        evidence_root: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}
#[derive(Args)]
struct Probes {
    /// Font selectors; omitted uses input JSON fonts or automatic directory discovery.
    #[arg(long = "font", value_delimiter = ',')]
    fonts: Vec<String>,
    #[arg(
        long = "encoding",
        value_delimiter = ',',
        default_value = "source,0,1,2,3,4,5,6,7,8,9,10,11,12,13,27,28,31,33,34,35,36"
    )]
    encodings: Vec<String>,
    /// Input keys; omitted sweeps every byte and the Unicode discovery repertoire.
    #[arg(long, value_delimiter = ',')]
    codes: Vec<u32>,
    #[arg(long, default_value_t = 128)]
    bound: u16,
    #[arg(long, default_value_t = 832)]
    width: u32,
    #[arg(long, default_value_t = 4096)]
    height: u32,
    #[arg(long, value_delimiter = ',', default_value = "65,66")]
    probes: Vec<u8>,
}
impl Probes {
    fn config(&self, fonts: Vec<String>) -> Result<Config> {
        ensure!(
            self.probes.len() == 2,
            "exactly two calibration probes required"
        );
        let c = Config {
            fonts,
            codes: (32..=126).collect(),
            encoding: 27,
            source: false,
            bound: self.bound,
            width: self.width,
            height: self.height,
            probes: [self.probes[0], self.probes[1]],
        };
        c.validate()?;
        Ok(c)
    }
    fn encodings(&self) -> Result<Vec<Encoding>> {
        self.encodings
            .iter()
            .map(|e| {
                if e == "source" {
                    Ok(Encoding::Ci0Source)
                } else {
                    Ok(Encoding::Input { ci: e.parse()? })
                }
            })
            .collect()
    }
}
#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Recover {
            probes,
            source,
            host,
            cache,
            out,
            offline,
            delay,
            timeout,
            identity_port,
            sgd_port,
        } => {
            ensure!(!out.exists(), "output exists");
            let seed = source.as_deref().map(Collection::load_input).transpose()?;
            let mut printer = Printer::new(&host, &cache, offline, delay, timeout)?;
            let url = reqwest::Url::parse(&host)?;
            let hostname = url
                .host_str()
                .ok_or_else(|| eyre::eyre!("missing printer host"))?
                .trim_matches(['[', ']']);
            let profile = cache.join("collection-printer.json");
            let id = if offline {
                let saved: serde_json::Value = serde_json::from_slice(&read(&profile)?)?;
                ensure!(saved["host"] == host, "cached identity host differs");
                identity::validate_identity(&saved["identity"])?;
                saved["identity"].clone()
            } else {
                let id = identity::identity(hostname, identity_port)?;
                write_json(&profile, &serde_json::json!({"host":host,"identity":id}))?;
                id
            };
            let fonts = if !probes.fonts.is_empty() {
                probes.fonts.clone()
            } else if let Some(c) = &seed {
                c.fonts.iter().map(|f| f.name.clone()).collect()
            } else {
                let path = cache.join("font-selectors.json");
                if offline {
                    let saved: serde_json::Value = serde_json::from_slice(&read(&path)?)?;
                    ensure!(
                        saved["identity"] == id,
                        "cached font directory identity differs"
                    );
                    serde_json::from_value(saved["fonts"].clone())?
                } else {
                    let names = identity::fonts(hostname, sgd_port)?;
                    write_json(&path, &serde_json::json!({"identity":id,"fonts":names}))?;
                    names
                }
            };
            printer = printer.with_identity(id.clone());
            let config = RecoveryConfig {
                probes: probes.config(fonts)?,
                encodings: probes.encodings()?,
                codes: probes.codes,
                seed,
            };
            let c = automatic::recover(&config, &mut printer).await?;
            if !offline {
                ensure!(
                    identity::identity(hostname, identity_port)? == id,
                    "printer identity changed during recovery"
                );
            }
            fs::create_dir_all(&out)?;
            c.save(&out.join("fonts.json"))?;
            compile::compile(&c, &out.join("rust"))?;
            println!(
                "Recovered {} fonts; inspect coverage and unresolved inputs in {}",
                c.fonts.len(),
                out.join("fonts.json").display()
            );
        }
        Command::Plan { probes, out } => {
            ensure!(!out.exists(), "output exists");
            let fonts = if probes.fonts.is_empty() {
                Config::default().fonts
            } else {
                probes.fonts.clone()
            };
            let pages = automatic::initial_pages(&probes.config(fonts)?)?;
            fs::create_dir_all(&out)?;
            for (i, p) in pages.iter().enumerate() {
                write_json(&out.join(format!("page-{i:03}.json")), p)?;
                fs::write(out.join(format!("page-{i:03}.zpl")), p.zpl()?)?;
            }
        }
        Command::Compile { source, out, fonts } => {
            let mut c = Collection::load_input(&source)?;
            if !fonts.is_empty() {
                ensure!(
                    fonts.iter().all(|n| c.fonts.iter().any(|f| f.name == *n)),
                    "unknown font selection"
                );
                c.fonts.retain(|f| fonts.contains(&f.name));
                c.seal()?;
            }
            compile::compile(&c, &out)?;
        }
        Command::MergeEvidence {
            source,
            report,
            evidence_root,
            out,
        } => {
            ensure!(!out.exists(), "output exists");
            let mut c = Collection::load_input(&source)?;
            for p in report {
                evidence::import(&mut c, &serde_json::from_slice(&read(&p)?)?, &evidence_root)?;
            }
            c.save(&out)?;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn one_recovery_cli_with_unicode_and_multiple_encodings() {
        let c = Cli::try_parse_from([
            "zpl-font-extract",
            "recover",
            "--host",
            "http://printer/",
            "--cache",
            "cache",
            "--out",
            "out",
            "--codes",
            "65,923",
            "--encoding",
            "27,28",
        ])
        .unwrap();
        let Command::Recover { probes, .. } = c.command else {
            panic!()
        };
        assert_eq!(probes.codes, [65, 923]);
        assert_eq!(probes.encodings().unwrap().len(), 2);
        assert!(Cli::try_parse_from(["zpl-font-extract", "collection"]).is_err());
    }
}
