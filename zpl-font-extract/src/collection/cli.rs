use super::*;
use crate::automatic::{capture::Printer, model::write_json};
use clap::Subcommand;
use std::path::PathBuf;
#[derive(Subcommand)]
pub enum Command {
    /// Read all type-1 bitmap font objects on selected printer drives via SGD.
    Inventory {
        #[arg(long)]
        printer: String,
        #[arg(long, default_value = "Z")]
        drives: String,
        #[arg(long)]
        out: PathBuf,
    },
    /// Decode every stored bitmap FNT from a directory without printer access.
    Import {
        directory: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Probe input-to-record mappings; preserve explicit unresolved and ambiguous results.
    Survey {
        source: PathBuf,
        #[arg(long)]
        host: String,
        #[arg(long)]
        cache: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        offline: bool,
        #[arg(long = "font", value_delimiter = ',')]
        fonts: Vec<String>,
        /// source or supported ^CI IDs. UTF-16 HTTP previews are intentionally disabled.
        #[arg(
            long = "encoding",
            value_delimiter = ',',
            default_value = "source,0,1,2,3,4,5,6,7,8,9,10,11,12,13,27,28,31,33,34,35,36"
        )]
        encodings: Vec<String>,
        /// Explicit input keys; without this, all bytes / the Unicode discovery repertoire.
        #[arg(long, value_delimiter = ',')]
        codes: Vec<u32>,
        #[arg(long, default_value_t = 832)]
        width: u32,
        #[arg(long, default_value_t = 4096)]
        height: u32,
        #[arg(long, default_value_t = 5.0)]
        delay: f64,
        #[arg(long, default_value_t = 30)]
        timeout: u64,
        /// Dedicated read-only JSON settings channel for serial/firmware cache scoping.
        #[arg(long, default_value_t = 9200)]
        identity_port: u16,
    },
    /// Import hash-checked prior observations or explicitly labelled unverified candidates.
    MergeEvidence {
        source: PathBuf,
        #[arg(long, required = true)]
        report: Vec<PathBuf>,
        #[arg(long, default_value = ".")]
        evidence_root: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    /// Compile complete records and encoding maps into zpl-bitmap-fonts tables.
    Compile {
        source: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}
pub async fn run(command: Command) -> Result<()> {
    match command {
        Command::Inventory {
            printer,
            drives,
            out,
        } => {
            let c = transport::collect(&printer, &drives, &out)?;
            c.save(&out.join("fonts.json"))?;
        }
        Command::Import { directory, out } => fnt::import_directory(&directory)?.save(&out)?,
        Command::Compile { source, out } => compile::compile(&Collection::load(&source)?, &out)?,
        Command::MergeEvidence {
            source,
            report,
            evidence_root,
            out,
        } => {
            ensure!(!out.exists(), "output exists");
            let mut c = Collection::load(&source)?;
            for p in report {
                evidence::import(&mut c, &serde_json::from_slice(&read(&p)?)?, &evidence_root)?;
            }
            c.save(&out)?;
        }
        Command::Survey {
            source,
            host,
            cache,
            out,
            offline,
            fonts,
            encodings,
            codes,
            width,
            height,
            delay,
            timeout,
            identity_port,
        } => {
            ensure!(!out.exists(), "output exists");
            let mut c = Collection::load(&source)?;
            ensure!(
                fonts.iter().all(|n| c.fonts.iter().any(|f| &f.name == n)),
                "unknown font selection"
            );
            let encodings = encodings
                .iter()
                .map(|e| {
                    if e == "source" {
                        Ok(Encoding::Ci0Source)
                    } else {
                        Ok(Encoding::Input { ci: e.parse()? })
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            ensure!(
                encodings.iter().all(|&e| survey::supported(e)),
                "unsupported survey encoding"
            );
            let mut printer = Printer::new(&host, &cache, offline, delay, timeout)?;
            let url = reqwest::Url::parse(&host)?;
            let hostname = url
                .host_str()
                .ok_or_else(|| eyre::eyre!("missing printer host"))?
                .trim_matches(['[', ']']);
            let profile_path = cache.join("collection-printer.json");
            let identity = if offline {
                let saved: Value = serde_json::from_slice(&read(&profile_path)?)?;
                ensure!(saved["host"] == host, "cached identity host differs");
                transport::validate_identity(&saved["identity"])?;
                saved["identity"].clone()
            } else {
                let identity = transport::identity(hostname, identity_port)?;
                write_json(
                    &profile_path,
                    &serde_json::json!({"host":host,"identity":identity}),
                )?;
                identity
            };
            printer = printer.with_identity(identity.clone());
            for f in &mut c.fonts {
                if !fonts.is_empty() && !fonts.contains(&f.name) {
                    continue;
                }
                for &encoding in &encodings {
                    let inputs = if !codes.is_empty() {
                        codes.clone()
                    } else if encoding == (Encoding::Input { ci: 28 }) {
                        survey::unicode_candidates()
                    } else {
                        (0..=255).collect()
                    };
                    let map =
                        survey::run(f, encoding, &inputs, width, height, &mut printer).await?;
                    evidence::merge(f, map)?;
                }
            }
            if !offline {
                ensure!(
                    transport::identity(hostname, identity_port)? == identity,
                    "printer identity changed during survey"
                );
            }
            c.seal()?;
            c.save(&out)?;
        }
    }
    Ok(())
}
