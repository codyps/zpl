//! Mapping surveys: exact single/double/triple composition and named-font controls.
//! ^CI byte ranges: https://docs.zebra.com/us/en/printers/software/zpl-pg/c-zpl-zpl-commands/r-zpl-ci.html
use super::*;
use crate::automatic::{
    capture::Capture,
    model::Glyph,
    probe::{self, Config, Page, Tile},
};
use eyre::{ensure, eyre};
use raster_diff::Raster;
use serde_json::json;
use std::collections::BTreeMap;

pub fn supported(encoding: Encoding) -> bool {
    match encoding {
        Encoding::Ci0Source => true,
        Encoding::Input { ci } => {
            (0..=13).contains(&ci) || [27, 28, 31, 33, 34, 35, 36].contains(&ci)
        }
        _ => false,
    }
}
pub fn pages(
    font: &Font,
    encoding: Encoding,
    codes: &[u32],
    width: u32,
    height: u32,
) -> Result<Vec<Page>> {
    ensure!(
        supported(encoding),
        "unsupported preview encoding; UTF-16 HTTP transport remains disabled"
    );
    ensure!(
        !codes.is_empty() && codes.len() <= 65536,
        "survey input count must be 1..65536"
    );
    let mut c = Config {
        fonts: vec![font.name.clone()],
        width,
        height,
        source: encoding == Encoding::Ci0Source,
        encoding: if encoding == Encoding::Ci0Source {
            0
        } else {
            28
        },
        ..Config::default()
    };
    c.codes = vec![48, 49, 65, 66];
    c.validate()?;
    let bh = font
        .records
        .iter()
        .map(|r| u32::from(r.height) + u32::from(r.top.unsigned_abs()))
        .max()
        .unwrap_or(32)
        .max(32);
    let bw = font
        .records
        .iter()
        .map(|r| u32::from(r.width).div_ceil(8) * 8 + u32::from(r.left.unsigned_abs()))
        .max()
        .unwrap_or(32)
        .max(32);
    let advance = font
        .records
        .iter()
        .map(|r| u32::from(r.advance))
        .max()
        .unwrap_or(32)
        .max(32);
    let mut tiles = vec![];
    let codes = codes
        .iter()
        .copied()
        .chain([48, 49, 65, 66])
        .collect::<BTreeSet<_>>();
    for code in codes {
        let bytes = match encoding {
            Encoding::Input { ci: 28 } => char::from_u32(code)
                .ok_or_else(|| eyre!("invalid Unicode scalar"))?
                .to_string()
                .into_bytes(),
            _ => vec![u8::try_from(code)?],
        };
        for n in 1..=3 {
            let mut t = Tile::new(
                format!("{code}:{n}"),
                vec![0; n],
                1,
                1,
                false,
                bh,
                bw,
                advance,
            );
            if c.source {
                t.text = bytes.repeat(n);
            } else {
                t.field_bytes = Some(bytes.repeat(n));
            }
            tiles.push(t);
        }
        if [48, 49, 65, 66].contains(&code) {
            let mut t = Tile::new(
                format!("control:{code}"),
                vec![0],
                1,
                1,
                false,
                bh,
                bw,
                advance,
            );
            t.fallback = 'B';
            if c.source {
                t.text = bytes;
            } else {
                t.field_bytes = Some(bytes);
            }
            tiles.push(t);
        }
    }
    let ci = match encoding {
        Encoding::Input { ci } => ci,
        _ => 0,
    };
    let mut pages = probe::pack(
        &c,
        &font.name,
        &format!("mapping-{encoding:?}-{}", font.source_sha256),
        tiles,
    )?;
    for p in &mut pages {
        p.wire_encoding = Some(ci);
    }
    Ok(pages)
}
fn composition(g: &Glyph, n: u32) -> BTreeSet<(i32, i32)> {
    let points = g.points();
    (0..n)
        .flat_map(|i| {
            points
                .iter()
                .map(move |&(x, y)| (x + (i * u32::from(g.advance)) as i32, y))
        })
        .collect()
}
pub fn classify(
    font: &Font,
    encoding: Encoding,
    codes: &[u32],
    observations: &BTreeMap<String, Glyph>,
    provenance: Value,
) -> Result<EncodingMap> {
    let at = |key: &str| {
        observations
            .get(key)
            .ok_or_else(|| eyre!("missing probe {key}"))
    };
    let mut fallback = false;
    for code in [48, 49, 65, 66] {
        if at(&format!("{code}:1"))?.points() != at(&format!("control:{code}"))?.points() {
            fallback = true;
        }
    }
    let mut records = BTreeMap::<_, Vec<_>>::new();
    for r in &font.records {
        let g = r.glyph()?;
        records.entry(g.points()).or_default().push((r.id, g));
    }
    let mut entries = vec![];
    for code in codes
        .iter()
        .copied()
        .chain([48, 49, 65, 66])
        .collect::<BTreeSet<_>>()
    {
        let observed = (1..=3)
            .map(|n| Ok(at(&format!("{code}:{n}"))?.points()))
            .collect::<Result<Vec<_>>>()?;
        let mut candidates = vec![];
        if !fallback && !observed[0].is_empty() {
            for (id, g) in records.get(&observed[0]).into_iter().flatten() {
                if (2..=3).all(|n| composition(g, n) == observed[n as usize - 1]) {
                    candidates.push(*id);
                }
            }
        }
        let status = if fallback {
            Status::FilenameFallback
        } else if observed[0].is_empty() {
            Status::BlankUnresolved
        } else if candidates.is_empty() {
            Status::Unmatched
        } else {
            Status::Matched
        };
        entries.push(Entry {
            input: code,
            status,
            candidates,
        });
    }
    Ok(EncodingMap {
        encoding,
        entries,
        provenance,
    })
}
pub async fn run(
    font: &Font,
    encoding: Encoding,
    codes: &[u32],
    width: u32,
    height: u32,
    capture: &mut impl Capture,
) -> Result<EncodingMap> {
    let pages = pages(font, encoding, codes, width, height)?;
    let mut observations = BTreeMap::new();
    let mut evidence = vec![];
    let mut first = None;
    for p in &pages {
        let bytes = capture.png(p, false).await?;
        let raster = Raster::decode_png_with_threshold(&bytes, None).map_err(|e| eyre!(e))?;
        for (key, glyph) in probe::measure(p, &raster)? {
            ensure!(
                observations.insert(key, glyph).is_none(),
                "duplicate survey tile"
            );
        }
        if first.is_none() {
            first = Some(raster);
        }
        evidence
            .push(json!({"request_sha256":hash(p.zpl()?.as_bytes()),"png_sha256":hash(&bytes)}));
    }
    let repeat = capture.png(&pages[0], true).await?;
    ensure!(
        Some(Raster::decode_png_with_threshold(&repeat, None).map_err(|e| eyre!(e))?) == first,
        "repeat page differs"
    );
    classify(
        font,
        encoding,
        codes,
        &observations,
        json!({"printer":capture.identity(),"pages":evidence,"repeat_sha256":hash(&repeat),"composition_lengths":[1,2,3]}),
    )
}
/// Conservative repertoire for discovery. Callers can provide additional scalar
/// inputs; coverage is always the explicit tested domain, never all Unicode.
pub fn unicode_candidates() -> Vec<u32> {
    (0..=0x52f)
        .chain(0x2000..=0x26ff)
        .chain(0xf000..=0xf0ff)
        .chain([0xfffd, 0xfffe, 0xffff])
        .collect()
}
