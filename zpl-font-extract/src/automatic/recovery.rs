//! One recovery sequence: calibration, glyph discovery, encoding maps and verification.
use super::{capture::Capture, model::hash, probe::Config};
use crate::collection::{evidence, survey, Collection, Encoding, Font};
use eyre::{ensure, Result};
use serde_json::json;

pub struct RecoveryConfig {
    pub probes: Config,
    pub encodings: Vec<Encoding>,
    /// Empty selects all byte inputs and the Unicode discovery repertoire.
    pub codes: Vec<u32>,
    pub seed: Option<Collection>,
}

pub async fn recover(c: &RecoveryConfig, capture: &mut impl Capture) -> Result<Collection> {
    c.probes.validate()?;
    ensure!(
        !c.encodings.is_empty() && c.encodings.iter().all(|e| survey::supported(*e)),
        "unsupported or empty encoding selection"
    );
    let mut result = if let Some(seed) = &c.seed {
        seed.validate()?;
        seed.clone()
    } else {
        // Resident calibration retains native metric checks and full-page holdouts.
        let mut calibration = c.probes.clone();
        calibration.fonts.retain(|f| f.len() == 1);
        let mut fonts = if calibration.fonts.is_empty() {
            vec![]
        } else {
            Collection::from_verified(&super::calibrate(&calibration, capture).await?)?.fonts
        };
        for name in &c.probes.fonts {
            if !fonts.iter().any(|f| f.name == *name) {
                fonts.push(Font {
                    name: name.clone(),
                    source_sha256: hash(&serde_json::to_vec(&(name, capture.identity()))?),
                    metrics: None,
                    records: vec![],
                    encodings: vec![],
                });
            }
        }
        fonts.sort_by(|a, b| a.name.cmp(&b.name));
        Collection::new(
            fonts,
            json!({"printer":capture.identity(),"method":"automatic-preview-recovery"}),
        )?
    };
    ensure!(
        c.probes
            .fonts
            .iter()
            .all(|n| result.fonts.iter().any(|f| f.name == *n)),
        "unknown font selection"
    );
    for font in &mut result.fonts {
        if !c.probes.fonts.contains(&font.name) {
            continue;
        }
        for &encoding in &c.encodings {
            let inputs = if !c.codes.is_empty() {
                c.codes.clone()
            } else if encoding == (Encoding::Input { ci: 28 }) {
                survey::unicode_candidates()
            } else {
                (0..=255).collect()
            };
            let map = survey::run(
                font,
                encoding,
                &inputs,
                c.probes.width,
                c.probes.height,
                capture,
            )
            .await?;
            // An earlier unmatched observation did not select a record. A newly
            // discovered glyph can resolve it without contradicting a prior match.
            if let Some(old) = font.encodings.iter_mut().find(|m| m.encoding == encoding) {
                let resolved = map
                    .entries
                    .iter()
                    .filter(|e| e.status == crate::collection::Status::Matched)
                    .map(|e| e.input)
                    .collect::<std::collections::BTreeSet<_>>();
                old.entries.retain(|entry| {
                    !(entry.status == crate::collection::Status::Unmatched
                        && resolved.contains(&entry.input))
                });
            }
            evidence::merge(font, map)?;
        }
    }
    result.provenance = json!({"previous":result.provenance,"recovery_scope":{"fonts":c.probes.fonts,"encodings":c.encodings,"codes":c.codes,"unicode_default":"U+0000-052F,U+2000-26FF,U+F000-F0FF,U+FFFD-FFFF","exhaustive":false}});
    result.seal()?;
    result.validate()?;
    Ok(result)
}
