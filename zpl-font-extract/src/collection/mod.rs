//! Complete stored bitmap records and separately evidenced input mappings.
//! See docs/complete-bitmap-collections.md for mapping semantics and limitations.
pub mod compile;
pub mod fnt;
pub mod survey;
pub mod transport;
use crate::automatic::model::{hash, read, selector, Glyph};
use eyre::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeSet, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub id: u16,
    pub advance: u16,
    pub left: i16,
    /// Relative to ^FT; the stored FNT y offset has the opposite sign.
    pub top: i16,
    pub width: u16,
    pub height: u16,
    pub flags: u16,
    /// Complete byte-padded rows, including any ink outside declared width.
    pub bitmap_hex: String,
}
impl Record {
    pub fn glyph(&self) -> Result<Glyph> {
        let bytes = unhex(&self.bitmap_hex)?;
        let stride = usize::from(self.width).div_ceil(8);
        let mut g = Glyph {
            advance: self.advance,
            left: self.left,
            top: self.top,
            width: u16::try_from(stride * 8)?,
            height: self.height,
            ..Glyph::default()
        };
        if stride == 0 || self.height == 0 {
            g.width = 0;
            g.height = 0;
        } else {
            g.bitmap = bytes.chunks_exact(stride).map(hex).collect();
        }
        g.validate()?;
        Ok(g)
    }
}
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn unhex(s: &str) -> Result<Vec<u8>> {
    ensure!(
        s.len().is_multiple_of(2) && s.bytes().all(|b| b.is_ascii_hexdigit()),
        "invalid bitmap hex"
    );
    s.as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| Ok(u8::from_str_radix(std::str::from_utf8(b)?, 16)?))
        .collect()
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Encoding {
    Input { ci: u8 },
    Ci0Source,
    CandidateCharacters,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Matched,
    BlankUnresolved,
    Unmatched,
    FilenameFallback,
    UnverifiedCandidate,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub input: u32,
    pub status: Status,
    pub candidates: Vec<u16>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncodingMap {
    pub encoding: Encoding,
    /// Exactly the tested inputs. An absent key is untested, never a missing glyph.
    pub entries: Vec<Entry>,
    pub provenance: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Font {
    pub name: String,
    pub source_sha256: String,
    pub header_hex: String,
    pub slot_count: u16,
    pub absent_slots: Vec<u16>,
    pub zero_record_slots: Vec<u16>,
    pub records: Vec<Record>,
    pub encodings: Vec<EncodingMap>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Coverage {
    pub font: String,
    pub archived_records: usize,
    pub visible_records: usize,
    pub observed_equivalent_records: usize,
    pub candidate_records: usize,
    pub unresolved_visible_record_ids: Vec<u16>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CodePage {
    pub ci: u8,
    pub byte_to_character: Vec<u16>,
    /// Offline candidate evidence, not proof of current printer behavior.
    pub provenance: Value,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Collection {
    pub schema: String,
    pub version: u8,
    pub fonts: Vec<Font>,
    pub provenance: Value,
    pub content_sha256: String,
    pub coverage: Vec<Coverage>,
    pub candidate_code_pages: Vec<CodePage>,
}
/// The complete multi-encoding document can exceed the legacy 32 MiB font limit.
pub const MAX_JSON_BYTES: u64 = 256 * 1024 * 1024;
fn read_document(path: &Path) -> Result<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    ensure!(
        file.metadata()?.len() <= MAX_JSON_BYTES,
        "collection JSON exceeds 256 MiB"
    );
    let mut bytes = vec![];
    file.take(MAX_JSON_BYTES + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() as u64 <= MAX_JSON_BYTES,
        "collection JSON exceeds 256 MiB"
    );
    Ok(bytes)
}
impl Collection {
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;
        ensure!(!path.exists(), "output exists");
        let mut bytes = serde_json::to_vec_pretty(self)?;
        bytes.push(b'\n');
        ensure!(
            bytes.len() as u64 <= MAX_JSON_BYTES,
            "collection JSON exceeds 256 MiB"
        );
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        crate::automatic::model::atomic_write(path, &bytes)
    }
    pub fn new(fonts: Vec<Font>, provenance: Value) -> Result<Self> {
        let mut c = Self {
            schema: "zebra-bitmap-collection".into(),
            version: 2,
            fonts,
            provenance,
            content_sha256: String::new(),
            coverage: vec![],
            candidate_code_pages: vec![],
        };
        c.seal()?;
        c.validate()?;
        Ok(c)
    }
    pub fn hash(&self) -> Result<String> {
        Ok(hash(&serde_json::to_vec(&(
            &self.schema,
            self.version,
            &self.fonts,
            &self.provenance,
            &self.coverage,
            &self.candidate_code_pages,
        ))?))
    }
    fn coverage(&self) -> Result<Vec<Coverage>> {
        self.fonts
            .iter()
            .map(|f| {
                let visible = f
                    .records
                    .iter()
                    .filter(|r| r.bitmap_hex.as_bytes().iter().any(|b| *b != b'0'))
                    .map(|r| r.id)
                    .collect::<BTreeSet<_>>();
                let observed = f
                    .encodings
                    .iter()
                    .flat_map(|m| &m.entries)
                    .filter(|e| e.status == Status::Matched)
                    .flat_map(|e| e.candidates.iter().copied())
                    .collect::<BTreeSet<_>>();
                let candidates = f
                    .encodings
                    .iter()
                    .flat_map(|m| &m.entries)
                    .filter(|e| e.status == Status::UnverifiedCandidate)
                    .flat_map(|e| e.candidates.iter().copied())
                    .collect::<BTreeSet<_>>();
                Ok(Coverage {
                    font: f.name.clone(),
                    archived_records: f.records.len(),
                    visible_records: visible.len(),
                    observed_equivalent_records: visible.intersection(&observed).count(),
                    candidate_records: visible.intersection(&candidates).count(),
                    unresolved_visible_record_ids: visible.difference(&observed).copied().collect(),
                })
            })
            .collect()
    }
    pub fn seal(&mut self) -> Result<()> {
        self.coverage = self.coverage()?;
        self.content_sha256 = self.hash()?;
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        let c: Self = serde_json::from_slice(&read_document(path)?)?;
        c.validate()?;
        Ok(c)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema == "zebra-bitmap-collection" && self.version == 2,
            "unsupported collection schema/version"
        );
        ensure!(
            self.content_sha256 == self.hash()?,
            "collection content hash mismatch"
        );
        ensure!(
            !self.fonts.is_empty() && self.fonts.len() <= 256,
            "invalid font count"
        );
        ensure!(
            self.coverage == self.coverage()?,
            "coverage differs from records/mappings"
        );
        let mut pages = BTreeSet::new();
        for page in &self.candidate_code_pages {
            ensure!(
                pages.insert(page.ci) && page.byte_to_character.len() == 256,
                "invalid/duplicate candidate code page"
            );
        }
        let mut names = BTreeSet::new();
        for f in &self.fonts {
            ensure!(
                selector(&f.name) && names.insert(&f.name),
                "invalid/duplicate font name"
            );
            ensure!(
                f.source_sha256.len() == 64
                    && f.source_sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid source hash"
            );
            ensure!(
                unhex(&f.header_hex)?.len() == 116 && (1..=4096).contains(&f.slot_count),
                "invalid stored header/count"
            );
            let mut slots = BTreeSet::new();
            let mut ids = BTreeSet::new();
            let mut previous = None;
            for r in &f.records {
                ensure!(
                    previous.is_none_or(|id| id < r.id),
                    "record IDs must be sorted and unique"
                );
                previous = Some(r.id);
                ensure!(
                    r.id < f.slot_count && slots.insert(r.id),
                    "duplicate/out-of-range record ID"
                );
                ids.insert(r.id);
                ensure!(
                    r.width <= 4096 && r.height <= 4096,
                    "excessive glyph dimensions"
                );
                ensure!(
                    unhex(&r.bitmap_hex)?.len()
                        == usize::from(r.width).div_ceil(8) * usize::from(r.height),
                    "bitmap length differs from dimensions"
                );
                r.glyph()?;
            }
            for &id in f.absent_slots.iter().chain(&f.zero_record_slots) {
                ensure!(
                    id < f.slot_count && slots.insert(id),
                    "duplicate/out-of-range absent slot"
                );
            }
            ensure!(
                slots.len() == usize::from(f.slot_count),
                "incomplete raw slot inventory"
            );
            let mut maps = BTreeSet::new();
            for m in &f.encodings {
                ensure!(
                    survey::supported(m.encoding) || m.encoding == Encoding::CandidateCharacters,
                    "unsupported encoding domain"
                );
                ensure!(maps.insert(m.encoding), "duplicate encoding map");
                let mut prev = None;
                for e in &m.entries {
                    ensure!(
                        prev.is_none_or(|v| v < e.input),
                        "unsorted/duplicate mapping input"
                    );
                    prev = Some(e.input);
                    ensure!(
                        match m.encoding {
                            Encoding::Input { ci: 28 } => char::from_u32(e.input).is_some(),
                            Encoding::CandidateCharacters => e.input <= 65535,
                            _ => e.input <= 255,
                        },
                        "input outside encoding domain"
                    );
                    ensure!(
                        e.candidates.iter().all(|id| ids.contains(id))
                            && e.candidates.windows(2).all(|p| p[0] < p[1]),
                        "invalid candidate IDs"
                    );
                    ensure!(
                        matches!(e.status, Status::Matched | Status::UnverifiedCandidate)
                            == !e.candidates.is_empty(),
                        "status/candidates disagree"
                    );
                    ensure!(
                        (e.status == Status::UnverifiedCandidate)
                            == (m.encoding == Encoding::CandidateCharacters),
                        "candidate evidence must be distinct"
                    );
                }
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests;

pub mod cli;
pub mod evidence;
