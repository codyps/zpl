//! Code 39 (ISO/IEC 16388), retained from the original local renderer.
//! Specification links and implementation limits: docs/barcodes.md.
use crate::output::Path;
const CHECK_CHARS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%";

pub(super) fn standalone(b: &super::Barcode, bytes: &[u8]) -> Result<Path, String> {
    // Zebra ^B3 pp. 70–72: optional Mod-43 checksum precedes the stop
    // character. Interpretation formatting is handled by the shared renderer.
    let value = super::ascii(bytes)?;
    if value.contains('*') {
        return Err("unsupported Code 39 character".into());
    }
    let checked;
    let encoded = if b.flag(1, false)? {
        checked = with_checksum(value)?;
        &checked
    } else {
        value
    };
    // ^BY p. 148: nearest-dot worked example versus measured ZD621
    // truncation. Quantize before accumulating element positions.
    let wide = b.module * b.ratio;
    let wide = if b.compatibility.code39_floor_wide_elements {
        wide.floor()
    } else {
        wide.round()
    };
    render(encoded, b.module, wide / b.module, b.height)
}

pub(in crate::render) fn with_checksum(s: &str) -> Result<String, String> {
    let sum = s.chars().try_fold(0, |sum, c| {
        CHECK_CHARS
            .find(c)
            .map(|index| sum + index)
            .ok_or("unsupported Code 39 character")
    })?;
    let mut result = s.to_owned();
    result.push(CHECK_CHARS.as_bytes()[sum % 43] as char);
    Ok(result)
}
pub(in crate::render) fn render(s: &str, module: f64, ratio: f64, h: f64) -> Result<Path, String> {
    const CHARS: &str = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ-. $/+%*";
    const PATTERNS: [u16; 44] = [
        0x034, 0x121, 0x061, 0x160, 0x031, 0x130, 0x070, 0x025, 0x124, 0x064, 0x109, 0x049, 0x148,
        0x019, 0x118, 0x058, 0x00d, 0x10c, 0x04c, 0x01c, 0x103, 0x043, 0x142, 0x013, 0x112, 0x052,
        0x007, 0x106, 0x046, 0x016, 0x181, 0x0c1, 0x1c0, 0x091, 0x190, 0x0d0, 0x085, 0x184, 0x0c4,
        0x0a8, 0x0a2, 0x08a, 0x02a, 0x094,
    ];
    let mut p = Path::default();
    let mut x = 0.;
    for c in std::iter::once('*')
        .chain(s.chars())
        .chain(std::iter::once('*'))
    {
        let idx = CHARS.find(c).ok_or("unsupported Code 39 character")?;
        let mask = PATTERNS[idx];
        for i in 0..9 {
            let w = module
                * if mask & (1 << (8 - i)) != 0 {
                    ratio
                } else {
                    1.
                };
            if i % 2 == 0 {
                p.rect(x, 0., w, h)
            }
            x += w;
        }
        x += module;
    }
    Ok(p)
}
