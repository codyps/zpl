//! Original GS1-128 composite assembly, ISO/IEC 24723:2010 §§7.4, 11–12.
//! <https://www.iso.org/standard/51425.html>
//! ZPL field syntax: linear AI element string | composite AI element string.
use super::*;
pub(super) fn render(
    b: &Barcode,
    data: &[u8],
    cc_c: bool,
    separator: usize,
) -> Result<Path, String> {
    let split = data
        .iter()
        .position(|&c| c == b'|')
        .ok_or("GS1 composite requires linear|2D field data")?;
    let (linear_data, secondary) = (&data[..split], &data[split + 1..]);
    if secondary.is_empty() || secondary.len() > 2361 {
        return Err("invalid GS1 composite secondary length".into());
    }
    let (linear, target) = gs1_128::encode(linear_data, cc_c)?;
    let bits = gs1_compaction::encode(secondary, vec![false], |n| {
        if cc_c {
            composite_c::size(n, linear.len()).map(|s| s.bits)
        } else {
            composite_a::capacity(n).or_else(|| composite_b::capacity(n))
        }
    })?;
    let (matrix, row_height) = if cc_c {
        let size = composite_c::size(bits.len(), linear.len()).ok_or("CC-C capacity exceeded")?;
        (composite_c::encode(&bits, size), 3.)
    } else if composite_a::CAPACITY.contains(&bits.len()) {
        (composite_a::encode(&bits), 2.)
    } else {
        (composite_b::encode(&bits)?, 2.)
    };
    // CC-C's first interior space is X=8; the linear start's second module
    // is X=1. CC-A/B's right quiet-zone module follows its 99X body.
    let offset = if cc_c { -7isize } else { target as isize - 99 };
    let linear_x = (-offset).max(0) as f64 * b.module;
    let composite_x = offset.max(0) as f64 * b.module;
    let two_d_height = matrix.h as f64 * row_height * b.module;
    let mut path = b.matrix(&matrix, b.module, row_height * b.module)?;
    path.transform(|p| Point::new(p.x + composite_x, p.y));
    let mut separator_barcode = b.clone();
    separator_barcode.height = separator as f64 * b.module;
    let mut sep =
        separator_barcode.linear(&linear.iter().map(|&v| !v).collect::<Vec<_>>(), None)?;
    sep.transform(|p| Point::new(p.x + linear_x, p.y + two_d_height));
    let mut bars = b.linear(&linear, None)?;
    bars.transform(|p| {
        Point::new(
            p.x + linear_x,
            p.y + two_d_height + separator as f64 * b.module,
        )
    });
    path.segments.extend(sep.segments);
    path.segments.extend(bars.segments);
    Ok(path)
}
