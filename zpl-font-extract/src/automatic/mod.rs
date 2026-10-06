//! Automatic bitmap discovery from printer previews, with no resident metric table.
//! Infer native metrics from magnification transitions and verify recovered glyphs
//! against independently composed preview pages.
//! Sampling is independent of the renderer and never reads firmware font files.
pub mod capture;
pub mod compile;
pub mod model;
pub mod probe;
use capture::Capture;
use eyre::{ensure, eyre, Result, WrapErr};
use model::*;
use probe::*;
use raster_diff::Raster;
use serde_json::{json, Value};
use std::collections::BTreeMap;

type Observations = BTreeMap<String, Glyph>;
fn decode(bytes: &[u8]) -> Result<Raster> {
    Raster::decode_png_with_threshold(bytes, None).map_err(|s| eyre!(s))
}
async fn batch(
    capture: &mut impl Capture,
    pages: &[Page],
    lineage: &mut Vec<Value>,
) -> Result<(Observations, Vec<Raster>)> {
    let mut result = BTreeMap::new();
    let mut images = Vec::new();
    for p in pages {
        let bytes = capture.png(p, false).await?;
        let image = decode(&bytes)?;
        for (k, v) in measure(p, &image)
            .wrap_err_with(|| format!("{} {} page {}", p.font, p.stage, p.number))?
        {
            ensure!(result.insert(k, v).is_none(), "duplicate batch probe key");
        }
        lineage.push(json!({"font":p.font,"stage":p.stage,"page":p.number,"request_sha256":hash(p.zpl()?.as_bytes()),"png_sha256":hash(&bytes)}));
        images.push(image);
    }
    let repeated = capture.png(&pages[0], true).await?;
    ensure!(
        decode(&repeated)? == images[0],
        "repeated first page differs; printer preview is unstable"
    );
    lineage.push(
        json!({"font":pages[0].font,"stage":pages[0].stage,"repeat_first_sha256":hash(&repeated)}),
    );
    Ok((result, images))
}
fn at<'a>(o: &'a Observations, key: &str) -> Result<&'a Glyph> {
    o.get(key).ok_or_else(|| eyre!("missing probe {key}"))
}
fn baseline(a: &Glyph, b: &Glyph) -> Result<u16> {
    ensure!(
        a.width > 0 && shape(a, b) && a.left == b.left,
        "FO/FT controls differ or are blank"
    );
    Ok(u16::try_from(i32::from(b.top) - i32::from(a.top) + 1)?)
}
fn initial(c: &Config) -> Vec<Tile> {
    let b = u32::from(c.bound);
    let [a, z] = c.probes;
    let mut tiles = vec![];
    for (key, text, fo) in [
        ("native", vec![a], false),
        ("origin", vec![a], true),
        ("pair", vec![a, a], false),
        ("space", vec![a, 32, a], false),
        ("second", vec![z], false),
        ("second-origin", vec![z], true),
    ] {
        tiles.push(Tile::new(key, text, 1, 1, fo, b, b, 2 * b));
    }
    // Filename selection must not silently fall back to the current default face.
    for code in [a, z] {
        for fallback in ['A', 'B'] {
            let mut t = Tile::new(
                format!("fallback-{code}-{fallback}"),
                vec![code],
                1,
                1,
                false,
                b,
                b,
                2 * b,
            );
            t.fallback = fallback;
            tiles.push(t);
        }
    }
    for vertical in [false, true] {
        for q in [8, 32, 128, 2 * c.bound]
            .into_iter()
            .filter(|q| *q <= 2 * c.bound)
            .collect::<std::collections::BTreeSet<_>>()
        {
            tiles.push(scaled_tile(c, vertical, q));
        }
    }
    tiles
}
/// Initial pages can be inspected without contacting a printer. Later plans are
/// adaptive and are retained next to every cached PNG.
pub fn initial_pages(c: &Config) -> Result<Vec<Page>> {
    c.validate()?;
    let mut pages = vec![];
    for f in &c.fonts {
        pages.extend(pack(c, f, "calibration-0", initial(c))?);
    }
    Ok(pages)
}
async fn recover_face(
    c: &Config,
    font: &str,
    capture: &mut impl Capture,
    lineage: &mut Vec<Value>,
) -> Result<(Face, usize, Value)> {
    let pages = pack(c, font, "calibration-0", initial(c))?;
    let (first, _) = batch(capture, &pages, lineage).await?;
    let mut native = at(&first, "native")?.clone();
    native.key = c.probes[0];
    let base = baseline(&native, at(&first, "origin")?)?;
    ensure!(
        baseline(at(&first, "second")?, at(&first, "second-origin")?)? == base,
        "independent baseline controls disagree"
    );
    for code in c.probes {
        ensure!(
            at(&first, &format!("fallback-{code}-A"))?
                == at(&first, &format!("fallback-{code}-B"))?,
            "font selection follows default face; requested filename was not selected"
        );
    }
    let pair = at(&first, "pair")?;
    let space = at(&first, "space")?;
    native.advance = u16::try_from(
        i32::from(pair.left) + i32::from(pair.width)
            - i32::from(native.left)
            - i32::from(native.width),
    )?;
    let space_advance = u16::try_from(
        i32::from(space.left) + i32::from(space.width)
            - i32::from(pair.left)
            - i32::from(pair.width),
    )?;
    let blank = Glyph {
        key: 32,
        advance: space_advance,
        ..Glyph::default()
    };
    ensure!(
        native.advance > 0
            && space_advance > 0
            && compose([&native, &native]) == pair.points()
            && compose([&native, &blank, &native]) == space.points(),
        "native/space advances do not recompose"
    );
    let mut candidates = [
        (1..=c.bound).collect::<Vec<_>>(),
        (1..=c.bound).collect::<Vec<_>>(),
    ];
    let mut observations = first;
    for round in 0..32 {
        for (i, axis) in ["width", "height"].into_iter().enumerate() {
            for (key, g) in &observations {
                if let Some(q) = key.strip_prefix(&format!("{axis}:")) {
                    let q = q.parse::<u16>()?;
                    let scale = stretch(&native, g, i == 1)?;
                    candidates[i].retain(|n| zoom(q, *n) == scale);
                }
            }
            ensure!(
                !candidates[i].is_empty(),
                "no {axis} fits bitmap scaling model/search bound"
            );
        }
        if candidates.iter().all(|c| c.len() == 1) {
            break;
        }
        ensure!(round < 31, "calibration did not converge");
        let mut tiles = vec![];
        for (i, cs) in candidates.iter().enumerate() {
            if cs.len() > 1 {
                tiles.push(scaled_tile(c, i == 1, distinguish(cs, c.bound)?));
            }
        }
        let p = pack(c, font, &format!("calibration-{}", round + 1), tiles)?;
        observations = batch(capture, &p, lineage).await?.0;
    }
    let metrics = Metrics {
        cell_width: candidates[0][0],
        cell_height: candidates[1][0],
        baseline: base,
        space_advance,
    };
    let (h, w) = (
        u32::from(metrics.cell_height),
        u32::from(metrics.cell_width),
    );
    let adv = 2 * u32::from(native.advance.max(space_advance).max(metrics.cell_width));
    let mut tiles = vec![];
    for key in c.keys() {
        tiles.push(Tile::new(
            format!("glyph:{key}"),
            vec![key],
            1,
            1,
            false,
            h,
            w,
            adv,
        ));
        tiles.push(Tile::new(
            format!("advance:{key}"),
            vec![native.key, key, native.key],
            1,
            1,
            false,
            h,
            w,
            adv,
        ));
    }
    tiles.push(Tile::new(
        "baseline",
        vec![c.probes[1]],
        1,
        1,
        true,
        3 * h,
        w,
        adv,
    ));
    for vertical in [false, true] {
        tiles.push(Tile::new(
            if vertical {
                "double-height"
            } else {
                "double-width"
            },
            vec![native.key],
            if vertical { 2 * metrics.cell_height } else { 1 },
            if vertical { 1 } else { 2 * metrics.cell_width },
            false,
            if vertical { 2 * h } else { h },
            if vertical { w } else { 2 * w },
            adv,
        ));
    }
    let p = pack(c, font, "glyphs", tiles)?;
    let (glyph_obs, _) = batch(capture, &p, lineage).await?;
    let mut glyphs = vec![];
    let mut excluded = BTreeMap::new();
    for key in c.keys() {
        let mut g = at(&glyph_obs, &format!("glyph:{key}"))?.clone();
        g.key = key;
        match infer_advance(at(&glyph_obs, &format!("advance:{key}"))?, &native, &g) {
            Ok(a) => g.advance = a,
            Err(e) if c.source && key != 32 && !c.probes.contains(&key) => {
                excluded.insert(key, e.to_string());
                continue;
            }
            Err(e) => return Err(e.wrap_err(format!("{font} key {key}"))),
        }
        glyphs.push(g);
    }
    ensure!(
        glyphs.iter().find(|g| g.key == native.key) == Some(&native)
            && glyphs.iter().find(|g| g.key == 32) == Some(&blank),
        "reference glyph or space changed since calibration"
    );
    let second = glyphs
        .iter()
        .find(|g| g.key == c.probes[1])
        .ok_or_else(|| eyre!("missing second control"))?;
    ensure!(
        baseline(second, at(&glyph_obs, "baseline")?)? == base,
        "baseline changed since calibration"
    );
    ensure!(
        stretch(&native, at(&glyph_obs, "double-width")?, false)? == 2
            && stretch(&native, at(&glyph_obs, "double-height")?, true)? == 2,
        "independent matrix validation failed"
    );
    let face = Face {
        name: font.into(),
        metrics,
        glyphs,
    };
    // Verify the serialized/reparsed JSON, not merely the in-memory measurements.
    let face: Face = serde_json::from_slice(&serde_json::to_vec(&face)?)?;
    let mut keys = face.glyphs.iter().map(|g| g.key).collect::<Vec<_>>();
    keys.sort_by_key(|k| hash(format!("holdout:{font}:{k}").as_bytes()));
    let max_advance = u32::from(face.glyphs.iter().map(|g| g.advance).max().unwrap());
    let bh = h.max(
        face.glyphs
            .iter()
            .map(|g| u32::from(g.height) + u32::from(g.top.unsigned_abs()))
            .max()
            .unwrap(),
    );
    let bw = w.max(
        face.glyphs
            .iter()
            .map(|g| u32::from(g.width) + u32::from(g.left.unsigned_abs()))
            .max()
            .unwrap(),
    );
    ensure!(bw + 64 < c.width, "glyph too wide for verification canvas");
    let per = ((c.width - 64 - bw) / max_advance).clamp(1, 80) as usize;
    let tiles = keys
        .chunks(per)
        .enumerate()
        .map(|(i, keys)| {
            Tile::new(
                format!("holdout:{i}"),
                keys.to_vec(),
                1,
                1,
                false,
                bh,
                bw,
                max_advance,
            )
        })
        .collect();
    let verification = pack(c, font, "verification", tiles)?;
    let (_, images) = batch(capture, &verification, lineage).await?;
    for (page, image) in verification.iter().zip(&images) {
        let mut expected = page.background()?;
        for t in &page.tiles {
            let gs = t
                .text
                .iter()
                .map(|k| face.glyphs.iter().find(|g| g.key == *k).unwrap());
            for (x, y) in compose(gs) {
                let (x, y) = (x + (t.x + t.ox) as i32, y + (t.y + t.oy) as i32);
                ensure!(
                    x >= 0 && y >= 0 && x < expected.width as i32 && y < expected.height as i32,
                    "verification ink exceeds canvas"
                );
                expected.pixels[y as usize * expected.width as usize + x as usize] = 0;
            }
        }
        ensure!(
            &expected == image,
            "JSON does not reproduce independent full verification page {}",
            page.number
        );
    }
    Ok((face, verification.len(), json!(excluded)))
}
/// Run calibration, adaptive refinement, sampling and fresh independent holdouts.
/// Cached Capture implementations make the same function usable completely offline.
pub async fn recover(c: &Config, capture: &mut impl Capture) -> Result<Document> {
    c.validate()?;
    let mut lineage = vec![];
    let mut fonts = vec![];
    let mut count = 0;
    let mut exclusions = BTreeMap::new();
    for font in &c.fonts {
        let (f, pages, excluded) = recover_face(c, font, capture, &mut lineage)
            .await
            .wrap_err_with(|| format!("recovering {font}"))?;
        fonts.push(f);
        count += pages;
        exclusions.insert(font.clone(), excluded);
    }
    fonts.sort_by(|a, b| a.name.cmp(&b.name));
    let mut d = Document {
        schema: "zebra-bitmap-fonts".into(),
        version: 1,
        mapping: Mapping {
            kind: if c.source { "ci0-source" } else { "input" }.into(),
            encoding: c.encoding,
        },
        fonts,
        verification: json!({"status":"passed","full_pages":count,"differing_pixels":0,"repeated_first_page_exact":true,"all_exported_glyphs_in_independent_capture":true}),
        provenance: json!({"printer":capture.identity(),"lineage":lineage,"excluded_inputs":exclusions,"omitted_format_inputs":if c.source {vec![9]} else {vec![]},"config":c}),
    };
    d.verification["content_sha256"] = json!(d.content_hash()?);
    d.validate()?;
    Ok(d)
}
#[cfg(test)]
mod tests;
