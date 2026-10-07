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
    pub missing_only: bool,
    pub inspect_new_fonts: bool,
}

pub async fn recover(c: &RecoveryConfig, capture: &mut impl Capture) -> Result<Collection> {
    c.probes.validate()?;
    ensure!(
        !c.encodings.is_empty() && c.encodings.iter().all(|e| survey::supported(*e)),
        "unsupported or empty encoding selection"
    );
    if let Some(seed) = &c.seed {
        seed.validate()?;
    }
    let mut fonts = c
        .seed
        .as_ref()
        .map(|seed| seed.fonts.clone())
        .unwrap_or_default();
    if c.seed.is_none() && !c.inspect_new_fonts {
        let mut calibration = c.probes.clone();
        calibration.fonts.retain(|f| f.len() == 1);
        if !calibration.fonts.is_empty() {
            fonts =
                Collection::from_verified(&super::calibrate(&calibration, capture).await?)?.fonts;
        }
    }
    let mut inspections = vec![];
    for name in &c.probes.fonts {
        if fonts.iter().any(|f| f.name == *name) {
            continue;
        }
        let face = if c.inspect_new_fonts {
            let (face, inspection) = super::discover_native(&c.probes, name, capture).await?;
            inspections.push(inspection);
            face
        } else {
            Some(Font {
                name: name.clone(),
                source_sha256: hash(&serde_json::to_vec(&(name, capture.identity()))?),
                metrics: None,
                records: vec![],
                encodings: vec![],
            })
        };
        if let Some(face) = face {
            fonts.push(face);
        }
    }
    ensure!(
        !fonts.is_empty(),
        "no native bitmap faces established; inspect cached calibration previews"
    );
    fonts.sort_by(|a, b| a.name.cmp(&b.name));
    let mut provenance = c
        .seed
        .as_ref()
        .map(|seed| seed.provenance.clone())
        .unwrap_or_else(
            || json!({"printer":capture.identity(),"method":"automatic-preview-recovery"}),
        );
    if !inspections.is_empty() {
        provenance = json!({"previous":provenance,"native_inspections":inspections});
    }
    let mut result = Collection::new(fonts, provenance)?;
    for index in 0..result.fonts.len() {
        let font = &result.fonts[index];
        if !c.probes.fonts.contains(&font.name) {
            continue;
        }
        for &encoding in &c.encodings {
            let font = &mut result.fonts[index];
            let mut inputs = if !c.codes.is_empty() {
                c.codes.clone()
            } else if encoding == (Encoding::Input { ci: 28 }) {
                survey::unicode_candidates()
            } else {
                (0..=255).collect()
            };
            if c.missing_only {
                if let Some(map) = font.encodings.iter().find(|m| m.encoding == encoding) {
                    inputs.retain(|input| !map.entries.iter().any(|e| e.input == *input));
                }
                if inputs.is_empty() {
                    continue;
                }
            }
            let mut map = survey::run(
                font,
                encoding,
                &inputs,
                c.probes.width,
                c.probes.height,
                capture,
            )
            .await?;
            // A blank-only preview cannot remeasure an advance. Preserve stronger
            // existing sentinel/holdout evidence, while marking this reuse explicitly.
            let mut retained_blanks = vec![];
            if let Some(old) = font.encodings.iter().find(|m| m.encoding == encoding) {
                for entry in &mut map.entries {
                    if entry.status != crate::collection::Status::BlankUnresolved {
                        continue;
                    }
                    if let Some(previous) = old.entries.iter().find(|e| {
                        e.input == entry.input && e.status == crate::collection::Status::Matched
                    }) {
                        if previous.candidates.iter().all(|id| {
                            font.records
                                .iter()
                                .any(|r| r.id == *id && r.bitmap_hex.bytes().all(|b| b == b'0'))
                        }) {
                            retained_blanks.push(entry.input);
                            *entry = previous.clone();
                        }
                    }
                }
            }
            if !retained_blanks.is_empty() {
                map.provenance["retained_verified_blank_inputs"] = json!(retained_blanks);
            }
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
            capture.checkpoint(&result)?;
        }
    }
    result.provenance = json!({"previous":result.provenance,"recovery_scope":{"fonts":c.probes.fonts,"encodings":c.encodings,"codes":c.codes,"unicode_default":"U+0000-052F,U+2000-26FF,U+F000-F0FF,U+FFFD-FFFF","exhaustive":false}});
    result.seal()?;
    result.validate()?;
    Ok(result)
}
