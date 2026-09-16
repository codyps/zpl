use super::sfnt;
use eyre::{ensure, Result};
use image_diff::Raster;
use serde_json::{json, Value};
use std::fmt::Write as _;
#[derive(Clone, Debug)]
pub struct Probe {
    pub text: String,
    pub h: u32,
    pub w: u32,
    pub turns: usize,
    pub font: String,
    pub origin: String,
    pub x: u32,
    pub y: u32,
    pub cw: u32,
    pub ch: u32,
    pub ax: u32,
    pub ay: u32,
}
#[derive(Clone)]
pub struct Page {
    pub group: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub probes: Vec<Probe>,
    pub prefix: String,
}
impl Page {
    pub fn zpl(&self) -> String {
        let mut z = self.prefix.clone();
        write!(
            z,
            "^XA^PW{}^LL{}^LH0,0^LS0^LT0^PON^LRN^CI27",
            self.width, self.height
        )
        .unwrap();
        for p in &self.probes {
            let ori = ['N', 'R', 'I', 'B'][p.turns];
            write!(z, "\n^{}{},{}", p.origin, p.x + p.ax, p.y + p.ay).unwrap();
            if p.font == "0" {
                write!(z, "^A0{ori},{},{}", p.h, p.w).unwrap();
            } else {
                write!(z, "^A@{ori},{},{},{}", p.h, p.w, p.font).unwrap();
            }
            z.push_str("^FH^FD");
            for c in p.text.bytes() {
                write!(z, "_{c:02X}").unwrap();
            }
            z.push_str("^FS");
        }
        z.push_str("\n^XZ");
        z
    }
    pub fn metadata(&self) -> Value {
        json!({"name":self.name,"group":self.group,"canvas":[self.width,self.height],"zpl_sha256":super::super::font_support::sha256(self.zpl().as_bytes()),"probes":self.probes.iter().map(|p|json!({"text":p.text,"height":p.h,"width":p.w,"orientation":(["N","R","I","B"][p.turns]),"font":p.font,"origin":p.origin,"tile":[p.x,p.y,p.cw,p.ch],"anchor":[p.ax,p.ay]})).collect::<Vec<_>>()})
    }
}
// Keep each experiment's size, transform, font and origin explicit at its call site.
#[allow(clippy::too_many_arguments)]
pub fn pages(
    out: &mut Vec<Page>,
    group: &str,
    h: u32,
    w: u32,
    turns: usize,
    font: &str,
    texts: &[String],
    origin: &str,
    prefix: &str,
) {
    let margin = 24;
    let maxsize = h.max(w);
    let side = 5 * maxsize / 4 + 2 * margin;
    let mut current = Page {
        group: group.into(),
        name: String::new(),
        width: 768,
        height: 0,
        probes: vec![],
        prefix: prefix.into(),
    };
    let (mut x, mut y, mut row) = (0, 0, 0);
    for text in texts {
        let cw = if text.len() == 1 {
            side
        } else {
            (text.len() as u32 * maxsize + 2 * margin).min(768)
        };
        let ch = side;
        assert!(cw <= 768 && ch <= 1024);
        if x + cw > 768 {
            x = 0;
            y += row;
            row = 0;
        }
        if y + ch > 1024 {
            current.height = y + row;
            current.name = format!("{:03}-{group}", out.len());
            out.push(current.clone());
            current.probes.clear();
            x = 0;
            y = 0;
            row = 0;
        }
        let (ax, ay) = if origin == "FO" {
            (margin, margin)
        } else {
            let (a, b) = (margin, margin + h);
            match turns {
                0 => (a, b),
                1 => (side - b, a),
                2 => (side - a, side - b),
                3 => (b, side - a),
                _ => unreachable!(),
            }
        };
        current.probes.push(Probe {
            text: text.clone(),
            h,
            w,
            turns,
            font: font.into(),
            origin: origin.into(),
            x,
            y,
            cw,
            ch,
            ax,
            ay,
        });
        x += cw;
        row = row.max(ch);
    }
    if !current.probes.is_empty() {
        current.height = y + row;
        current.name = format!("{:03}-{group}", out.len());
        out.push(current);
    }
}
fn glyphs(s: &str) -> Vec<String> {
    s.chars().map(|c| c.to_string()).collect()
}
pub fn plan() -> Vec<Page> {
    let mut out = vec![];
    let chars = glyphs("HIl|OonmgjpAV_.");
    pages(&mut out, "repeat-start", 32, 0, 0, "0", &chars, "FT", "");
    for font in ["0", "Z:0.TTF", "E:TT0003M_.TTF"] {
        for (h, w, t) in [(32, 0, 0), (47, 29, 1), (96, 0, 0)] {
            pages(&mut out, "mapping", h, w, t, font, &chars, "FT", "");
        }
    }
    for shift in [false, true] {
        let data = sfnt::font(shift);
        let name = if shift { "R:ZRFH.TTF" } else { "R:ZRFN.TTF" };
        let hex: String = data.iter().map(|b| format!("{b:02X}")).collect();
        let prefix = format!("~DY{name},A,T,{},,{hex}\n", data.len());
        for (h, w, t) in [
            (16, 0, 0),
            (31, 0, 0),
            (32, 0, 0),
            (33, 0, 0),
            (48, 0, 0),
            (64, 0, 0),
            (32, 19, 0),
            (32, 0, 1),
            (32, 0, 2),
            (32, 0, 3),
        ] {
            pages(
                &mut out,
                "calibration",
                h,
                w,
                t,
                name,
                &glyphs("ABCDEFGH"),
                "FT",
                &prefix,
            );
        }
    }
    for h in 12..=64 {
        if !(34..=38).contains(&h) && !(58..=62).contains(&h) {
            pages(&mut out, "development", h, 0, 0, "0", &chars, "FT", "");
        }
    }
    for h in [256, 384, 512] {
        pages(
            &mut out,
            "large-development",
            h,
            0,
            0,
            "0",
            &glyphs("HIlOog"),
            "FT",
            "",
        );
    }
    for h in [24, 48, 96] {
        for t in 1..4 {
            pages(
                &mut out,
                "rotation-development",
                h,
                0,
                t,
                "0",
                &chars,
                "FT",
                "",
            );
        }
    }
    let metrics: Vec<String> = [
        "||", "|l|", "|ll|", "|llll|", "|H|", "|HH|", "|HHHH|", "| |", "|  |", "|AV|", "|To|",
        "|WA|",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for h in [16, 24, 32, 48, 64] {
        for origin in ["FT", "FO"] {
            pages(
                &mut out,
                "metrics-development",
                h,
                0,
                0,
                "0",
                &metrics,
                origin,
                "",
            );
        }
    }
    pages(
        &mut out,
        "ascii-development",
        32,
        0,
        0,
        "0",
        &glyphs(&(32u8..=126).map(char::from).collect::<String>()),
        "FT",
        "",
    );
    for h in (34..=38).chain(58..=62) {
        pages(&mut out, "sealed-normal", h, 0, 0, "0", &chars, "FT", "");
    }
    pages(
        &mut out,
        "sealed-large",
        448,
        0,
        0,
        "0",
        &glyphs("HIlOog"),
        "FT",
        "",
    );
    for (h, w, t) in [
        (41, 23, 0),
        (53, 71, 0),
        (41, 23, 1),
        (53, 71, 2),
        (43, 0, 3),
    ] {
        pages(
            &mut out,
            if t == 0 {
                "sealed-stretched"
            } else {
                "sealed-rotation"
            },
            h,
            w,
            t,
            "0",
            &chars,
            "FT",
            "",
        );
    }
    pages(
        &mut out,
        "sealed-text",
        29,
        0,
        0,
        "0",
        &["AV To WA 019".into(), "jumpy Hg_42".into()],
        "FT",
        "",
    );
    pages(&mut out, "repeat-end", 32, 0, 0, "0", &chars, "FT", "");
    out
}
pub fn normalize(r: &Raster, p: &Probe) -> Raster {
    let mut out = Raster {
        width: p.cw,
        height: p.ch,
        pixels: vec![255; (p.cw * p.ch) as usize],
    };
    for y in 0..p.ch {
        for x in 0..p.cw {
            let (rx, ry) = match p.turns {
                0 => (x, y),
                1 => (p.cw - 1 - y, x),
                2 => (p.cw - 1 - x, p.ch - 1 - y),
                3 => (y, p.ch - 1 - x),
                _ => unreachable!(),
            };
            out.pixels[(y * p.cw + x) as usize] =
                r.pixels[((p.y + ry) * r.width + p.x + rx) as usize];
        }
    }
    out
}
pub fn measurement(r: &Raster) -> Value {
    let mut xs = vec![false; r.width as usize];
    let (mut x0, mut y0, mut x1, mut y1, mut ink) = (r.width, r.height, 0, 0, 0);
    for y in 0..r.height {
        for x in 0..r.width {
            if r.pixels[(y * r.width + x) as usize] == 0 {
                xs[x as usize] = true;
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
                ink += 1;
            }
        }
    }
    let mut runs = vec![];
    let mut start = None;
    for (x, black) in xs.into_iter().chain([false]).enumerate() {
        match (start, black) {
            (None, true) => start = Some(x),
            (Some(a), false) => {
                runs.push([a, x]);
                start = None;
            }
            _ => {}
        }
    }
    json!({"ink":ink,"bounds":if ink>0{Some([x0,y0,x1,y1])}else{None},"column_runs":runs})
}
pub fn validate(r: &Raster, page: &Page) -> Result<()> {
    ensure!(
        (r.width, r.height) == (page.width, page.height),
        "{}: canvas changed from {}x{} to {}x{}",
        page.name,
        page.width,
        page.height,
        r.width,
        r.height
    );
    for p in &page.probes {
        let tile = normalize(r, p);
        let m = measurement(&tile);
        ensure!(
            p.text.trim().is_empty() || m["ink"].as_u64().unwrap() > 0,
            "{}: empty glyph {:?}",
            page.name,
            p.text
        );
        if let Some(b) = m["bounds"].as_array() {
            ensure!(
                b[0].as_u64().unwrap() > 1
                    && b[1].as_u64().unwrap() > 1
                    && b[2].as_u64().unwrap() < p.cw as u64 - 1
                    && b[3].as_u64().unwrap() < p.ch as u64 - 1,
                "{}: clipping in {:?}",
                page.name,
                p.text
            );
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn campaign_is_bounded_and_separated() {
        let p = plan();
        assert!(p.len() <= 200, "{}", p.len());
        for page in &p {
            assert!(page.height <= 1024);
            for tile in &page.probes {
                assert!(tile.x + tile.cw <= page.width && tile.y + tile.ch <= page.height);
                if page.group == "development" {
                    assert!(!(34..=38).contains(&tile.h) && !(58..=62).contains(&tile.h));
                }
            }
        }
    }
    #[test]
    fn detects_clipping_and_empty() {
        let page = plan().remove(0);
        let r = Raster {
            width: page.width,
            height: page.height,
            pixels: vec![255; (page.width * page.height) as usize],
        };
        assert!(validate(&r, &page).is_err());
    }
}
