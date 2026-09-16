//! Original TCIF Linked Code 39, US20010045461A1 paragraphs 0024–0029.
//! <https://patents.google.com/patent/US20010045461A1/en>
//! The linked MicroPDF417 component is above the six-digit ECI Code 39.
use super::*;
pub(super) fn render(b: &Barcode, data: &[u8]) -> Result<Path, String> {
    if data.len() < 6 || !data[..6].iter().all(u8::is_ascii_digit) {
        return Err("TLC39 requires a six-digit ECI number".into());
    }
    let module = b.num(1, if b.dpi >= 600 { 4. } else { 2. }, 1., 10.)?;
    let ratio = b.num(2, 2., 2., 3.)?;
    let height = b.num(
        3,
        if b.dpi >= 600 {
            120.
        } else if b.dpi >= 300 {
            60.
        } else {
            40.
        },
        1.,
        9999.,
    )?;
    let mut path = code39::render(ascii(&data[..6])?, module, ratio, height)?;
    if data.get(6) != Some(&b',') {
        return Ok(path);
    }
    let payload = &data[7..];
    if payload.is_empty()
        || payload.len() > 150
        || payload.split(|&c| c == b',').any(|part| {
            part.is_empty() || part.len() > 25 || !part.iter().all(u8::is_ascii_alphanumeric)
        })
    {
        return Err("invalid TLC39 serial/additional data".into());
    }
    let payload: Vec<_> = payload
        .iter()
        .map(|&c| if c == b',' { 29 } else { c })
        .collect();
    let matrix = micropdf417::linked(&payload)?;
    let xscale = b.num(4, if b.dpi >= 600 { 4. } else { 2. }, 1., 10.)?;
    let yscale = b.num(5, if b.dpi >= 600 { 8. } else { 4. }, 1., 255.)?;
    let mut micro = b.matrix(&matrix, xscale, yscale)?;
    // Isolated Code 39 T, after the linear component's 10X right quiet zone.
    let (_, _, right, _) = super::super::bounds(&path);
    let mut flag_bits = Vec::new();
    runs(&mut flag_bits, &[1, 1, 1, 1, 2, 1, 2, 2, 1]);
    let mut linear = b.clone();
    linear.module = module;
    linear.height = height;
    let mut flag = linear.linear(&flag_bits, Some(ratio))?;
    flag.transform(|p| Point::new(p.x + right + 10. * module, p.y));
    path.segments.extend(flag.segments);
    let offset = matrix.h as f64 * yscale + 2. * yscale;
    path.transform(|p| Point::new(p.x, p.y + offset));
    micro.transform(|p| Point::new(p.x + module, p.y));
    path.segments.extend(micro.segments);
    Ok(path)
}
