use super::model::*;
use eyre::{ensure, eyre, Result};
use raster_diff::Raster;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// ZPL Programming Guide ^A pp. 60–61, ^FO p. 201, ^FT p. 205.
/// https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
/// Positive nearest-integer bitmap magnification, capped at ten.
pub fn zoom(request: u16, native: u16) -> u16 {
    ((2 * u32::from(request) + u32::from(native)) / (2 * u32::from(native))).clamp(1, 10) as u16
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Config {
    pub fonts: Vec<String>,
    pub codes: Vec<u8>,
    pub encoding: u8,
    pub source: bool,
    pub bound: u16,
    pub width: u32,
    pub height: u32,
    /// Two visibly distinct control glyphs; source fonts can require custom keys.
    pub probes: [u8; 2],
}
impl Default for Config {
    fn default() -> Self {
        Self {
            fonts: "ABCDEFGH@".chars().map(|c| c.to_string()).collect(),
            codes: (32..=126).collect(),
            encoding: 27,
            source: false,
            bound: 128,
            width: 832,
            height: 4096,
            probes: [65, 66],
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.fonts.is_empty()
                && self.fonts.len() <= 256
                && self.fonts.iter().all(|f| selector(f))
                && self.fonts.iter().collect::<BTreeSet<_>>().len() == self.fonts.len(),
            "invalid or duplicate font selectors"
        );
        ensure!(
            (2..=256).contains(&self.bound)
                && (256..=4096).contains(&self.width)
                && self.width.is_multiple_of(64)
                && (256..=16384).contains(&self.height)
                && self.width * self.height <= 32 * 1024 * 1024,
            "invalid search bound/canvas"
        );
        ensure!(
            [0, 13, 27, 28].contains(&self.encoding) && (!self.source || self.encoding == 0),
            "invalid encoding/source mapping"
        );
        ensure!(
            !self.codes.is_empty()
                && self.codes.len() <= 256
                && self.probes[0] != self.probes[1]
                && !self.probes.contains(&32)
                && !self.probes.contains(&9),
            "invalid glyph/probe keys"
        );
        for c in self.codes.iter().chain(&self.probes) {
            ensure!(
                self.source || (32..=126).contains(c) || self.encoding >= 27 && *c >= 160,
                "unsupported input key {c}"
            );
        }
        Ok(())
    }
    pub fn keys(&self) -> Vec<u8> {
        self.codes
            .iter()
            .copied()
            .chain(self.probes)
            .chain([32])
            .filter(|c| !self.source || *c != 9)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Tile {
    pub key: String,
    pub text: Vec<u8>,
    pub h: u16,
    pub w: u16,
    pub fo: bool,
    pub width: u32,
    pub height: u32,
    pub ox: u32,
    pub oy: u32,
    pub x: u32,
    pub y: u32,
    pub fallback: char,
}
impl Tile {
    // Keep requested size and canvas bounds explicit at each experiment site.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        key: impl Into<String>,
        text: Vec<u8>,
        h: u16,
        w: u16,
        fo: bool,
        bh: u32,
        bw: u32,
        advance: u32,
    ) -> Self {
        Self {
            key: key.into(),
            width: 32 + bw + text.len().saturating_sub(1) as u32 * advance,
            height: 32 + bh * if fo { 1 } else { 2 },
            ox: 16,
            oy: 16 + if fo { 0 } else { bh },
            text,
            h,
            w,
            fo,
            x: 0,
            y: 0,
            fallback: 'A',
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Page {
    pub font: String,
    pub stage: String,
    pub number: usize,
    pub config: Config,
    pub tiles: Vec<Tile>,
}
impl Page {
    pub fn markers(&self) -> Result<Vec<(u32, u32, u32, u32)>> {
        let digest = hash(&serde_json::to_vec(self)?);
        let bytes = (0..16)
            .step_by(2)
            .map(|i| u8::from_str_radix(&digest[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        let mut rects = vec![
            (3, 3, 3, 3),
            (self.config.width - 6, self.config.height - 6, 3, 3),
        ];
        for bit in 0..64 {
            rects.push((
                16 + bit * 3,
                if bytes[bit as usize / 8] & (128 >> (bit % 8)) != 0 {
                    6
                } else {
                    10
                },
                2,
                2,
            ));
        }
        Ok(rects)
    }
    pub fn background(&self) -> Result<Raster> {
        let mut r = Raster {
            width: self.config.width,
            height: self.config.height,
            pixels: vec![255; (self.config.width * self.config.height) as usize],
        };
        for (x, y, w, h) in self.markers()? {
            for dy in 0..h {
                for dx in 0..w {
                    r.pixels[((y + dy) * r.width + x + dx) as usize] = 0;
                }
            }
        }
        Ok(r)
    }
    pub fn zpl(&self) -> Result<String> {
        let c = &self.config;
        let mut s = format!(
            "^XA^PW{}^LL{}^LH0,0^LS0^LT0^PON^LRN^FWN^CI{}",
            c.width, c.height, c.encoding
        );
        if c.encoding == 28 {
            s.push_str("^PA0,0,0,0");
        }
        for (x, y, w, h) in self.markers()? {
            s.push_str(&format!("^FO{x},{y}^GB{w},{h},{},B,0^FS", w.max(h)));
        }
        for t in &self.tiles {
            let mut bytes = Vec::new();
            if c.source {
                let codes = t.text.iter().copied().collect::<BTreeSet<_>>();
                let destinations = (33u8..127).filter(|c| ![94, 95, 126].contains(c));
                ensure!(codes.len() <= 91, "too many distinct source keys in field");
                let mapping = codes
                    .into_iter()
                    .zip(destinations)
                    .map(|(code, dest)| (code, if code == 32 { 32 } else { dest }))
                    .collect::<BTreeMap<_, _>>();
                s.push_str("^CI0");
                for (&code, &dest) in &mapping {
                    if dest != 32 {
                        s.push_str(&format!(",{code},{dest}"));
                    }
                }
                bytes.extend(t.text.iter().map(|c| mapping[c]));
            } else if c.encoding == 28 {
                for &code in &t.text {
                    let mut buf = [0; 4];
                    bytes.extend_from_slice(char::from(code).encode_utf8(&mut buf).as_bytes());
                }
            } else {
                bytes.extend_from_slice(&t.text);
            }
            let command = if self.font == "@" {
                "GS".to_string()
            } else if self.font.contains(':') {
                "A@".to_string()
            } else {
                format!("A{}", self.font)
            };
            let filename = if self.font.contains(':') {
                s.push_str(&format!("^CF{},1,1", t.fallback));
                format!(",{}", self.font)
            } else {
                String::new()
            };
            s.push_str(
                &format!(
                    "^{}{}, {},0^{command}N,{},{}{filename}^FH^FD",
                    if t.fo { "FO" } else { "FT" },
                    t.x + t.ox,
                    t.y + t.oy,
                    t.h,
                    t.w
                )
                .replace(", ", ","),
            );
            for b in bytes {
                s.push_str(&format!("_{b:02X}"));
            }
            s.push_str("^FS");
        }
        if c.source {
            s.push_str("^CI0");
            for c in 33..127 {
                s.push_str(&format!(",{c},{c}"));
            }
        }
        s.push_str("^XZ\n");
        Ok(s)
    }
}
pub fn pack(c: &Config, font: &str, stage: &str, mut tiles: Vec<Tile>) -> Result<Vec<Page>> {
    tiles.sort_by(|a, b| {
        b.height
            .cmp(&a.height)
            .then(b.width.cmp(&a.width))
            .then(a.key.cmp(&b.key))
    });
    let mut pages = Vec::new();
    let (mut x, mut y, mut rh) = (16, 24, 0);
    let mut current = Vec::new();
    for mut t in tiles {
        ensure!(
            t.width <= c.width - 32 && t.height <= c.height - 40,
            "probe {} exceeds canvas; increase --width/--height or reduce --bound",
            t.key
        );
        if x + t.width > c.width - 16 {
            (x, y, rh) = (16, y + rh, 0);
        }
        if y + t.height > c.height - 16 {
            pages.push(Page {
                font: font.into(),
                stage: stage.into(),
                number: pages.len(),
                config: c.clone(),
                tiles: current,
            });
            current = Vec::new();
            (x, y, rh) = (16, 24, 0);
        }
        t.x = x;
        t.y = y;
        x += t.width;
        rh = rh.max(t.height);
        current.push(t);
    }
    if !current.is_empty() {
        pages.push(Page {
            font: font.into(),
            stage: stage.into(),
            number: pages.len(),
            config: c.clone(),
            tiles: current,
        });
    }
    ensure!(
        !pages.is_empty() && pages.len() <= 512,
        "invalid probe page count"
    );
    Ok(pages)
}
pub fn measure(page: &Page, image: &Raster) -> Result<BTreeMap<String, Glyph>> {
    ensure!(
        (image.width, image.height) == (page.config.width, page.config.height),
        "preview dimensions differ from native canvas"
    );
    let mut outside = image.clone();
    let mut result = BTreeMap::new();
    for t in &page.tiles {
        let mut points = Vec::new();
        for y in 0..t.height {
            for x in 0..t.width {
                let i = ((t.y + y) * image.width + t.x + x) as usize;
                if image.pixels[i] == 0 {
                    ensure!(
                        x > 1 && y > 1 && x + 2 < t.width && y + 2 < t.height,
                        "clipped probe {}",
                        t.key
                    );
                    points.push((x, y));
                }
                outside.pixels[i] = 255;
            }
        }
        let mut g = Glyph::default();
        if !points.is_empty() {
            let minx = points.iter().map(|p| p.0).min().unwrap();
            let maxx = points.iter().map(|p| p.0).max().unwrap();
            let miny = points.iter().map(|p| p.1).min().unwrap();
            let maxy = points.iter().map(|p| p.1).max().unwrap();
            g.left = i16::try_from(minx as i32 - t.ox as i32)?;
            g.top = i16::try_from(miny as i32 - t.oy as i32)?;
            g.width = u16::try_from(maxx - minx + 1)?;
            g.height = u16::try_from(maxy - miny + 1)?;
            let mut rows = vec![vec![0u8; usize::from(g.width).div_ceil(8)]; usize::from(g.height)];
            for (x, y) in points {
                rows[(y - miny) as usize][((x - minx) / 8) as usize] |= 128 >> ((x - minx) % 8);
            }
            g.bitmap = rows
                .into_iter()
                .map(|r| r.iter().map(|b| format!("{b:02x}")).collect())
                .collect();
        }
        ensure!(
            result.insert(t.key.clone(), g).is_none(),
            "duplicate probe key"
        );
    }
    ensure!(
        outside == page.background()?,
        "page identity/registration mismatch or stray ink"
    );
    Ok(result)
}
pub fn shape(a: &Glyph, b: &Glyph) -> bool {
    (a.width, a.height, &a.bitmap) == (b.width, b.height, &b.bitmap)
}
pub fn stretch(a: &Glyph, b: &Glyph, vertical: bool) -> Result<u16> {
    ensure!(
        a.width > 0
            && a.height > 0
            && b.width.is_multiple_of(a.width)
            && b.height.is_multiple_of(a.height),
        "noninteger or blank magnification"
    );
    let (sx, sy) = (b.width / a.width, b.height / a.height);
    let (scale, other) = if vertical { (sy, sx) } else { (sx, sy) };
    ensure!(
        (1..=10).contains(&scale) && other == 1,
        "unexpected independent-axis magnification"
    );
    let mut points = BTreeSet::new();
    for (x, y) in a.points() {
        for dy in 0..sy {
            for dx in 0..sx {
                points.insert((
                    (x - i32::from(a.left)) * i32::from(sx) + i32::from(dx) + i32::from(b.left),
                    (y - i32::from(a.top)) * i32::from(sy) + i32::from(dy) + i32::from(b.top),
                ));
            }
        }
    }
    ensure!(
        points == b.points(),
        "magnified pixels differ from exact bitmap replication"
    );
    Ok(scale)
}
pub fn compose<'a>(glyphs: impl IntoIterator<Item = &'a Glyph>) -> BTreeSet<(i32, i32)> {
    let mut points = BTreeSet::new();
    let mut pen = 0;
    for g in glyphs {
        points.extend(g.points().into_iter().map(|(x, y)| (x + pen, y)));
        pen += i32::from(g.advance);
    }
    points
}
pub fn infer_advance(observed: &Glyph, native: &Glyph, glyph: &Glyph) -> Result<u16> {
    let actual = observed.points();
    let n = native.points();
    let mut base = n.clone();
    base.extend(
        glyph
            .points()
            .into_iter()
            .map(|(x, y)| (x + i32::from(native.advance), y)),
    );
    ensure!(
        base.is_subset(&actual),
        "context does not contain isolated glyphs"
    );
    let max = i32::from(observed.left) + i32::from(observed.width)
        - i32::from(native.left)
        - i32::from(native.width)
        - i32::from(native.advance);
    let remaining = actual.difference(&base).next();
    let candidates: BTreeSet<i32> = if let Some(&(x, y)) = remaining {
        n.iter()
            .filter(|p| p.1 == y)
            .map(|p| x - p.0 - i32::from(native.advance))
            .collect()
    } else {
        (0..=max).collect()
    };
    let matches = candidates
        .into_iter()
        .filter(|a| {
            *a >= 0 && *a <= max && {
                let mut expected = base.clone();
                expected.extend(
                    n.iter()
                        .map(|(x, y)| (x + i32::from(native.advance) + a, *y)),
                );
                expected == actual
            }
        })
        .collect::<Vec<_>>();
    ensure!(
        matches.len() == 1 && matches[0] > 0,
        "ambiguous, ignored, or contextual glyph advance"
    );
    Ok(u16::try_from(matches[0])?)
}
pub fn distinguish(candidates: &[u16], bound: u16) -> Result<u16> {
    let (score, q) = (1..=2 * bound)
        .map(|q| {
            let mut counts = [0usize; 11];
            for &n in candidates {
                counts[usize::from(zoom(q, n))] += 1;
            }
            (
                candidates.len() * candidates.len() - counts.iter().map(|n| n * n).sum::<usize>(),
                q,
            )
        })
        .max_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)))
        .ok_or_else(|| eyre!("no distinguishing request"))?;
    ensure!(score > 0, "cannot distinguish matrix sizes");
    Ok(q)
}
pub fn scaled_tile(c: &Config, vertical: bool, q: u16) -> Tile {
    let bound = u32::from(c.bound);
    let extent = bound.max(u32::from(q) + bound.div_ceil(2));
    Tile::new(
        format!("{}:{q}", if vertical { "height" } else { "width" }),
        vec![c.probes[0]],
        if vertical { q } else { 1 },
        if vertical { 1 } else { q },
        false,
        if vertical { extent } else { bound },
        if vertical { bound } else { extent },
        2 * bound,
    )
}
