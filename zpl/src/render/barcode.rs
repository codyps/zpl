use crate::output::Path;
mod aztec;
mod bits;
mod codabar;
mod codablock;
mod code11;
pub(super) mod code128;
pub(super) mod code39;
mod code49;
mod code49_patterns;
mod code93;
mod composite;
mod composite_a;
mod composite_b;
mod composite_c;
#[cfg(test)]
mod composite_tests;
mod data_matrix;
mod databar;
mod databar_expanded;
mod databar_limited;
mod databar_stacked;
mod ean13;
mod ean8;
mod gs1_128;
mod gs1_compaction;
mod industrial2of5;
mod intelligent_mail;
mod interleaved2of5;
mod logmars;
mod maxicode;
mod maxicode_modules;
mod micropdf417;
mod msi;
mod pdf417;
mod pdf417_patterns;
mod planet;
mod plessey;
mod postal;
mod postnet;
mod qr;
mod reed_solomon;
mod retail;
mod standard2of5;
mod tlc39;
mod two_of_five;
mod upc_extension;
mod upca;
mod upce;

use crate::output::Point;

#[derive(Clone)]
pub(super) struct Barcode {
    name: String,
    params: Vec<String>,
    pub show: bool,
    above: bool,
    pub height: f64,
    module: f64,
    ratio: f64,
    dpi: u32,
}
pub(super) fn supported(name: &str) -> bool {
    matches!(
        name,
        "B0" | "B1"
            | "B2"
            | "B4"
            | "B5"
            | "B7"
            | "B8"
            | "B9"
            | "BA"
            | "BB"
            | "BD"
            | "BE"
            | "BF"
            | "BI"
            | "BJ"
            | "BK"
            | "BL"
            | "BM"
            | "BO"
            | "BP"
            | "BQ"
            | "BR"
            | "BS"
            | "BT"
            | "BU"
            | "BX"
            | "BZ"
    )
}
impl Barcode {
    pub fn new(
        name: &str,
        p: &[&str],
        module: f64,
        ratio: f64,
        height: f64,
        dpi: u32,
    ) -> Result<Self, String> {
        let max = match name {
            "BD" | "BF" | "BL" => 3,
            "B5" | "B8" | "BE" | "BI" | "BJ" | "BS" | "B4" => 4,
            "B1" | "B9" | "BA" | "BP" | "BQ" | "BU" | "BZ" => 5,
            "B2" | "B7" | "BB" | "BM" | "BR" | "BT" => 6,
            "BK" | "BO" | "B0" => 7,
            "BX" => 8,
            _ => return Err("unknown barcode".into()),
        };
        if p.len() > max {
            return Err(format!("{name}: unexpected command parameters"));
        }
        let mut b = Self {
            name: name.into(),
            params: p.iter().map(|s| (*s).into()).collect(),
            show: false,
            above: false,
            height,
            module,
            ratio,
            dpi,
        };
        let layout = match name {
            "B1" | "BK" | "BM" | "BP" => Some((2, 3, 4)),
            "B2" | "B5" | "B8" | "B9" | "BA" | "BE" | "BI" | "BJ" | "BS" | "BU" | "BZ" => {
                Some((1, 2, 3))
            }
            _ => None,
        };
        if let Some((h, f, g)) = layout {
            b.height = b.num(h, height, 1., 32000.)?;
            b.show = b.flag(f, name != "BZ")?;
            b.above = b.flag(g, false)?;
        }
        if name == "BL" {
            b.height = b.num(1, height, 1., 32000.)?;
            b.show = true;
            b.above = b.flag(2, false)?;
        }
        if name == "B4" {
            b.height = b.num(1, height, 1., 32000.)?;
            b.show = b.param(2, "N") != "N";
            b.above = b.param(2, "N") == "A";
        }
        Ok(b)
    }
    fn param<'a>(&'a self, i: usize, default: &'a str) -> &'a str {
        self.params
            .get(i)
            .filter(|s| !s.is_empty())
            .map(String::as_str)
            .unwrap_or(default)
    }
    fn num(&self, i: usize, default: f64, min: f64, max: f64) -> Result<f64, String> {
        let n = match self.params.get(i).filter(|s| !s.is_empty()) {
            Some(s) => s
                .parse::<f64>()
                .map_err(|_| format!("{}: invalid number", self.name))?,
            None => default,
        };
        if !n.is_finite() || n < min || n > max {
            return Err(format!("{}: parameter {} out of range", self.name, i + 1));
        }
        Ok(n)
    }
    fn integer(&self, i: usize, default: usize, min: usize, max: usize) -> Result<usize, String> {
        let n = self.num(i, default as f64, min as f64, max as f64)?;
        if n.fract() != 0. {
            return Err("barcode parameter must be an integer".into());
        }
        Ok(n as usize)
    }
    fn flag(&self, i: usize, default: bool) -> Result<bool, String> {
        match self.param(i, if default { "Y" } else { "N" }) {
            "Y" => Ok(true),
            "N" => Ok(false),
            _ => Err(format!("{}: invalid Y/N flag", self.name)),
        }
    }
    fn require(&self, i: usize, default: &str, allowed: &[&str]) -> Result<(), String> {
        if allowed.contains(&self.param(i, default)) {
            Ok(())
        } else {
            Err(format!(
                "{}: parameter {} mode unsupported",
                self.name,
                i + 1
            ))
        }
    }
    fn scale(&self) -> f64 {
        match self.dpi {
            0..=150 => 1.,
            151..=250 => 2.,
            251..=450 => 3.,
            _ => 6.,
        }
    }
    fn linear(&self, modules: &[bool], wide: Option<f64>) -> Result<Path, String> {
        let mut path = Path::default();
        let mut x = 0.;
        let mut i = 0;
        while i < modules.len() {
            let start = i;
            let black = modules[i];
            while i < modules.len() && modules[i] == black {
                i += 1;
            }
            let count = i - start;
            let width = self.module * wide.filter(|_| count > 1).unwrap_or(count as f64);
            if black {
                path.rect(x, 0., width, self.height);
            }
            x += width;
        }
        Ok(path)
    }
    fn matrix(&self, m: &Matrix, x: f64, y: f64) -> Result<Path, String> {
        let mut p = Path::default();
        for row in 0..m.h {
            let mut col = 0;
            while col < m.w {
                if !m.get(col, row) {
                    col += 1;
                    continue;
                }
                let start = col;
                while col < m.w && m.get(col, row) {
                    col += 1;
                }
                p.rect(
                    start as f64 * x,
                    row as f64 * y,
                    (col - start) as f64 * x,
                    y,
                );
            }
        }
        Ok(p)
    }
    pub fn render(&self, bytes: &[u8], fw: f64, fh: f64) -> Result<(Path, f64), String> {
        if bytes.is_empty() {
            return Err(format!("{}: empty barcode data", self.name));
        }
        let mut p = match self.name.as_str() {
            "B0" | "BO" => aztec::render(self, bytes),
            "B1" => code11::render(self, bytes),
            "B2" => interleaved2of5::render(self, bytes),
            "B4" => code49::render(self, bytes),
            "B5" => planet::render(self, bytes),
            "B7" => pdf417::render(self, bytes),
            "B8" => ean8::render(self, bytes),
            "B9" => upce::render(self, bytes),
            "BA" => code93::render(self, bytes),
            "BB" => codablock::render(self, bytes),
            "BD" => maxicode::render(self, bytes),
            "BE" => ean13::render(self, bytes),
            "BF" => micropdf417::render(self, bytes),
            "BI" => industrial2of5::render(self, bytes),
            "BJ" => standard2of5::render(self, bytes),
            "BK" => codabar::render(self, bytes),
            "BL" => logmars::render(self, bytes),
            "BM" => msi::render(self, bytes),
            "BP" => plessey::render(self, bytes),
            "BQ" => qr::render(self, bytes),
            "BR" => databar::render(self, bytes),
            "BS" => upc_extension::render(self, bytes),
            "BT" => tlc39::render(self, bytes),
            "BU" => upca::render(self, bytes),
            "BX" => data_matrix::render(self, bytes),
            "BZ" => postal::render(self, bytes),
            _ => unreachable!(),
        }?;
        let (_, _, _, height) = super::bounds(&p);
        let mut baseline = height;
        if self.show {
            let value = self.interpretation(bytes)?;
            let mut t = super::font::text(&value, fw, fh)?;
            if self.above {
                p.transform(|p| Point::new(p.x, p.y + fh + 3.));
                baseline += fh + 3.;
            } else {
                t.transform(|p| Point::new(p.x, p.y + height + 3.));
            }
            p.segments.extend(t.segments);
        }
        Ok((p, baseline))
    }
    fn interpretation(&self, bytes: &[u8]) -> Result<String, String> {
        if self.name == "BP" {
            return plessey::interpretation(self, bytes);
        }
        if self.name == "BA" {
            return code93::interpretation(bytes);
        }
        let mut decimal = match self.name.as_str() {
            "B8" => Some(retail::checked(bytes, 8)?),
            "BE" => Some(retail::checked(bytes, 13)?),
            "BU" => Some(retail::checked(bytes, 12)?),
            "B9" => Some(upce::canonical(bytes)?),
            "BM" if self.flag(5, false)? => Some(msi::checks(bytes, self.param(1, "B"))?),
            _ => None,
        };
        if let Some(ref mut digits) = decimal {
            if matches!(self.name.as_str(), "BU" | "B9") && !self.flag(4, true)? {
                digits.pop();
            }
            return Ok(digits.iter().map(|v| (v + b'0') as char).collect());
        }
        Ok(ascii(bytes)?.to_string())
    }
}
fn ascii(bytes: &[u8]) -> Result<&str, String> {
    std::str::from_utf8(bytes).map_err(|_| "barcode requires ASCII data".into())
}

struct Matrix {
    w: usize,
    h: usize,
    cells: Vec<bool>,
}
impl Matrix {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            cells: vec![false; w * h],
        }
    }
    fn get(&self, x: usize, y: usize) -> bool {
        self.cells[y * self.w + x]
    }
    fn set(&mut self, x: usize, y: usize, v: bool) {
        self.cells[y * self.w + x] = v;
    }
}
fn digits(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() || !data.iter().all(u8::is_ascii_digit) {
        return Err("barcode requires decimal digits".into());
    }
    Ok(data.iter().map(|b| b - b'0').collect())
}
fn mod10(data: &[u8]) -> u8 {
    ((10 - data
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &v)| v as usize * if i % 2 == 0 { 3 } else { 1 })
        .sum::<usize>()
        % 10)
        % 10) as u8
}
fn append_pattern(out: &mut Vec<bool>, pattern: u32, width: usize) {
    out.extend((0..width).rev().map(|i| pattern & (1 << i) != 0));
}
fn runs(out: &mut Vec<bool>, widths: &[usize]) {
    for (i, &n) in widths.iter().enumerate() {
        out.extend(std::iter::repeat_n(i % 2 == 0, n));
    }
}
