use eyre::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path};

pub fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
pub fn read(path: &Path) -> Result<Vec<u8>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 32 * 1024 * 1024,
        "file exceeds 32 MiB: {}",
        path.display()
    );
    Ok(bytes)
}
pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    atomic_write(path, &bytes)
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)?;
    Ok(())
}
pub fn selector(name: &str) -> bool {
    if name.len() == 1 && "ABCDEFGH@".contains(name) {
        return true;
    }
    let Some((drive, file)) = name.split_once(':') else {
        return false;
    };
    let Some(stem) = file.strip_suffix(".FNT") else {
        return false;
    };
    drive.len() == 1
        && "REBAZ".contains(drive)
        && (1..=64).contains(&stem.len())
        && stem
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Metrics {
    pub cell_height: u16,
    pub cell_width: u16,
    pub baseline: u16,
    pub space_advance: u16,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Glyph {
    pub key: u8,
    pub advance: u16,
    pub left: i16,
    pub top: i16,
    pub width: u16,
    pub height: u16,
    pub bitmap: Vec<String>,
}
impl Glyph {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.width <= 4096 && self.height <= 4096 && (self.width == 0) == (self.height == 0),
            "invalid glyph dimensions"
        );
        ensure!(
            self.bitmap.len() == usize::from(self.height),
            "invalid bitmap row count"
        );
        for row in &self.bitmap {
            ensure!(
                row.len() == usize::from(self.width).div_ceil(8) * 2
                    && row.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid hex row"
            );
            if !self.width.is_multiple_of(8) {
                let last = u8::from_str_radix(&row[row.len() - 2..], 16)?;
                ensure!(
                    last & ((1 << (8 - self.width % 8)) - 1) == 0,
                    "nonzero bitmap padding"
                );
            }
        }
        Ok(())
    }
    pub fn points(&self) -> BTreeSet<(i32, i32)> {
        let mut out = BTreeSet::new();
        for (y, row) in self.bitmap.iter().enumerate() {
            for (i, pair) in row.as_bytes().as_chunks::<2>().0.iter().enumerate() {
                let byte = u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap();
                for bit in 0..8 {
                    let x = i * 8 + bit;
                    if x < usize::from(self.width) && byte & (128 >> bit) != 0 {
                        out.insert((
                            i32::from(self.left) + x as i32,
                            i32::from(self.top) + y as i32,
                        ));
                    }
                }
            }
        }
        out
    }
    pub fn dense(&self) -> Vec<u8> {
        let mut bytes = vec![0; (usize::from(self.width) * usize::from(self.height)).div_ceil(8)];
        for (x, y) in self.points() {
            let bit = (y - i32::from(self.top)) as usize * usize::from(self.width)
                + (x - i32::from(self.left)) as usize;
            bytes[bit / 8] |= 128 >> (bit % 8);
        }
        bytes
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Face {
    pub name: String,
    pub metrics: Metrics,
    pub glyphs: Vec<Glyph>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    pub kind: String,
    pub encoding: u8,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Document {
    pub schema: String,
    pub version: u8,
    pub mapping: Mapping,
    pub fonts: Vec<Face>,
    pub verification: Value,
    #[serde(default)]
    pub provenance: Value,
}
impl Document {
    pub fn content_hash(&self) -> Result<String> {
        Ok(hash(&serde_json::to_vec(
            &json!({"schema":self.schema,"version":self.version,"mapping":self.mapping,"fonts":self.fonts}),
        )?))
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema == "zebra-bitmap-fonts" && self.version == 1,
            "unsupported font JSON schema/version"
        );
        ensure!(
            [0, 13, 27, 28].contains(&self.mapping.encoding)
                && (self.mapping.kind == "input"
                    || self.mapping.kind == "ci0-source" && self.mapping.encoding == 0),
            "unsupported key mapping"
        );
        ensure!(
            !self.fonts.is_empty() && self.fonts.len() <= 256,
            "invalid font count"
        );
        let mut last = None;
        for font in &self.fonts {
            ensure!(
                selector(&font.name) && last.is_none_or(|s: &str| s < font.name.as_str()),
                "invalid/unsorted/duplicate font name"
            );
            last = Some(&font.name);
            let m = &font.metrics;
            ensure!(
                (1..=4096).contains(&m.cell_height)
                    && (1..=4096).contains(&m.cell_width)
                    && m.space_advance > 0,
                "invalid font metrics"
            );
            ensure!(
                !font.glyphs.is_empty() && font.glyphs.len() <= 256,
                "invalid glyph count"
            );
            let mut previous = None;
            for g in &font.glyphs {
                g.validate()?;
                ensure!(
                    previous.is_none_or(|k| k < g.key),
                    "unsorted/duplicate glyph key"
                );
                previous = Some(g.key);
            }
        }
        let v = &self.verification;
        ensure!(
            v["status"] == "passed"
                && v["full_pages"].as_u64().is_some_and(|n| n > 0)
                && v["differing_pixels"].as_u64() == Some(0),
            "font JSON has not passed full-page verification"
        );
        ensure!(
            v["content_sha256"].as_str() == Some(&self.content_hash()?),
            "verified content hash mismatch"
        );
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self> {
        let d: Self = serde_json::from_slice(&read(path)?)?;
        d.validate()?;
        Ok(d)
    }
}
