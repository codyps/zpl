//! Import measured observations and separately labelled unverified mapping candidates.
use super::*;
use eyre::{ensure, eyre};
use serde_json::json;
use std::collections::BTreeMap;
pub fn merge(font: &mut Font, map: EncodingMap) -> Result<()> {
    if let Some(old) = font
        .encodings
        .iter_mut()
        .find(|m| m.encoding == map.encoding)
    {
        let mut entries = old
            .entries
            .iter()
            .cloned()
            .map(|e| (e.input, e))
            .collect::<BTreeMap<_, _>>();
        for e in map.entries {
            if let Some(prev) = entries.get(&e.input) {
                ensure!(
                    prev.status == e.status && prev.candidates == e.candidates,
                    "conflicting mapping for {} input {}",
                    font.name,
                    e.input
                );
            }
            entries.insert(e.input, e);
        }
        old.entries = entries.into_values().collect();
        old.provenance = json!({"merged":[old.provenance,map.provenance]});
    } else {
        font.encodings.push(map);
        font.encodings.sort_by_key(|m| m.encoding);
    }
    Ok(())
}
fn string(v: &Value) -> Result<&str> {
    v.as_str()
        .ok_or_else(|| eyre!("missing string in evidence"))
}
fn ids(v: &Value) -> Result<Vec<u16>> {
    v.as_array()
        .ok_or_else(|| eyre!("missing IDs"))?
        .iter()
        .map(|v| {
            Ok(u16::try_from(
                v.as_u64().ok_or_else(|| eyre!("invalid ID"))?,
            )?)
        })
        .collect()
}
pub fn import(c: &mut Collection, report: &Value, root: &Path) -> Result<()> {
    let mut next = c.clone();
    match string(&report["schema"])? {
        "zebra-bitmap-fonts" => {
            let document: crate::automatic::model::Document =
                serde_json::from_value(report.clone())?;
            document.validate()?;
            let encoding = if document.mapping.kind == "ci0-source" {
                Encoding::Ci0Source
            } else {
                Encoding::Input {
                    ci: document.mapping.encoding,
                }
            };
            for face in &document.fonts {
                let font = next
                    .fonts
                    .iter_mut()
                    .find(|f| f.name == face.name)
                    .ok_or_else(|| eyre!("unknown font {}", face.name))?;
                let mut records = BTreeMap::<_, Vec<_>>::new();
                for r in &font.records {
                    let g = r.glyph()?;
                    records
                        .entry((g.advance, g.points()))
                        .or_default()
                        .push(r.id);
                }
                let mut entries = vec![];
                for glyph in &face.glyphs {
                    let points = glyph.points();
                    let candidates = if points.is_empty() {
                        vec![]
                    } else {
                        records
                            .get(&(glyph.advance, points.clone()))
                            .cloned()
                            .unwrap_or_default()
                    };
                    let status = if points.is_empty() {
                        Status::BlankUnresolved
                    } else if candidates.is_empty() {
                        Status::Unmatched
                    } else {
                        Status::Matched
                    };
                    entries.push(Entry {
                        input: u32::from(glyph.key),
                        status,
                        candidates,
                    });
                }
                merge(
                    font,
                    EncodingMap {
                        encoding,
                        entries,
                        provenance: json!({"source":"verified-v1-collection","content_sha256":document.content_hash()?,"verification":document.verification}),
                    },
                )?;
            }
        }

        "zebra-font-encoding-observations-v1" => {
            let lineage = &report["lineage"];
            let capture = root.join(string(&lineage["capture"])?);
            ensure!(
                hash(&read(&capture.join("plan.json"))?) == string(&lineage["plan_sha256"])?,
                "evidence plan hash mismatch"
            );
            let receipt: Value = serde_json::from_slice(&read(&capture.join("capture.json"))?)?;
            ensure!(
                receipt["complete"] == true
                    && receipt["pages"] == lineage["pages"]
                    && receipt["plan_sha256"] == lineage["plan_sha256"],
                "incomplete/different capture receipt"
            );
            for page in lineage["pages"]
                .as_array()
                .ok_or_else(|| eyre!("missing capture pages"))?
            {
                let index = page["page"]
                    .as_u64()
                    .ok_or_else(|| eyre!("missing page index"))?;
                for (suffix, key) in [("png", "png_sha256"), ("zpl", "zpl_sha256")] {
                    ensure!(
                        hash(&read(&capture.join(format!("page-{index:03}.{suffix}")))?)
                            == string(&page[key])?,
                        "capture file hash mismatch"
                    );
                }
            }
            ensure!(
                hash(&read(&capture.join("repeat-first.png"))?)
                    == string(&lineage["repeat_first_sha256"])?,
                "repeat hash mismatch"
            );
            let ci = u8::try_from(
                report["encoding"]
                    .as_u64()
                    .ok_or_else(|| eyre!("missing encoding"))?,
            )?;
            let encoding = match report["mapping"].as_str().unwrap_or("input") {
                "input" => Encoding::Input { ci },
                "source" if ci == 0 => Encoding::Ci0Source,
                _ => return Err(eyre!("experimental source mappings cannot be imported")),
            };
            for (name, f) in report["fonts"]
                .as_object()
                .ok_or_else(|| eyre!("missing fonts"))?
            {
                let font = next
                    .fonts
                    .iter_mut()
                    .find(|f| f.name == *name)
                    .ok_or_else(|| eyre!("unknown font {name}"))?;
                ensure!(
                    f["reference_sha256"] == font.source_sha256,
                    "reference hash mismatch"
                );
                let mut entries = vec![];
                for (code, e) in f["entries"]
                    .as_object()
                    .ok_or_else(|| eyre!("missing entries"))?
                {
                    let status = match string(&e["status"])? {
                        "matched" => Status::Matched,
                        "blank-unresolved" => Status::BlankUnresolved,
                        "unmatched" => Status::Unmatched,
                        "filename-fallback" => Status::FilenameFallback,
                        _ => return Err(eyre!("unknown observation status")),
                    };
                    entries.push(Entry {
                        input: code.parse()?,
                        status,
                        candidates: if status == Status::Matched {
                            ids(&e["candidate_raw_ids"])?
                        } else {
                            vec![]
                        },
                    });
                }
                entries.sort_by_key(|e| e.input);
                merge(
                    font,
                    EncodingMap {
                        encoding,
                        entries,
                        provenance: json!({"imported_observations_sha256":hash(&serde_json::to_vec(report)?),"lineage":lineage}),
                    },
                )?;
            }
        }
        "zebra-font-mapping-candidates-v1" => {
            for page in report["code_pages"]
                .as_array()
                .ok_or_else(|| eyre!("missing candidate code pages"))?
            {
                let ci = u8::try_from(
                    page["id"]
                        .as_u64()
                        .ok_or_else(|| eyre!("missing code-page ID"))?,
                )?;
                let values = ids(&page["byte_to_character"])?;
                if let Some(old) = next.candidate_code_pages.iter().find(|p| p.ci == ci) {
                    ensure!(
                        old.byte_to_character == values,
                        "conflicting candidate code page"
                    );
                } else {
                    next.candidate_code_pages.push(CodePage{ci,byte_to_character:values,provenance:json!({"report_sha256":hash(&serde_json::to_vec(report)?),"provenance":page["provenance"]})});
                }
            }
            next.candidate_code_pages.sort_by_key(|p| p.ci);
            for (name, f) in report["font_candidates"]
                .as_object()
                .ok_or_else(|| eyre!("missing candidates"))?
            {
                let font = next
                    .fonts
                    .iter_mut()
                    .find(|f| f.name == *name)
                    .ok_or_else(|| eyre!("unknown font"))?;
                ensure!(
                    f["reference_sha256"] == font.source_sha256,
                    "reference hash mismatch"
                );
                let mut inputs: BTreeMap<u32, BTreeSet<u16>> = BTreeMap::new();
                for e in f["entries"]
                    .as_array()
                    .ok_or_else(|| eyre!("missing candidates"))?
                {
                    let id = u16::try_from(
                        e["raw_record_id"]
                            .as_u64()
                            .ok_or_else(|| eyre!("missing raw ID"))?,
                    )?;
                    for code in ids(&e["character_codes"])? {
                        inputs.entry(u32::from(code)).or_default().insert(id);
                    }
                }
                let entries = inputs
                    .into_iter()
                    .map(|(input, candidates)| Entry {
                        input,
                        status: Status::UnverifiedCandidate,
                        candidates: candidates.into_iter().collect(),
                    })
                    .collect();
                merge(
                    font,
                    EncodingMap {
                        encoding: Encoding::CandidateCharacters,
                        entries,
                        provenance: json!({"report_sha256":hash(&serde_json::to_vec(report)?),"provenance":f["provenance"]}),
                    },
                )?;
            }
        }
        _ => return Err(eyre!("unsupported evidence schema")),
    }
    next.seal()?;
    next.validate()?;
    *c = next;
    Ok(())
}
