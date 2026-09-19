//! USPS POSTNET and PLANET two-height bar encoding.
use super::*;
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    match b.integer(4, 0, 0, 3)? {
        0 => postnet::render(b, data),
        1 => planet::render(b, data),
        3 => intelligent_mail::render(b, data),
        _ => Err("postal type 2 is reserved".into()),
    }
}
pub(super) fn draw(b: &Barcode, data: &[u8], planet: bool) -> Result<Path, String> {
    let mut d = digits(data)?;
    let check = (10 - d.iter().map(|&v| v as usize).sum::<usize>() % 10) % 10;
    d.push(check as u8);
    const TALL: [u8; 10] = [24, 3, 5, 6, 9, 10, 12, 17, 18, 20];
    let mut bars = vec![true];
    for v in d {
        bars.extend(
            (0..5)
                .rev()
                .map(|i| (TALL[v as usize] & (1 << i) != 0) ^ planet),
        );
    }
    bars.push(true);
    let mut p = Path::default();
    for (i, tall) in bars.into_iter().enumerate() {
        let height = if tall { b.height } else { b.height * 0.4 };
        p.rect(
            // ZD621 module-width probes (1, 2, 3 dots) produce pitches 2, 5,
            // and 7 dots: truncate the fixed 2.5-module pitch per bar. The
            // captured firmware ignores the documented ^BY ratio support.
            // Capture provenance: docs/printer-accuracy.md.
            i as f64 * b.postal_pitch(),
            b.height - height,
            b.module,
            height,
        );
    }
    Ok(p)
}
