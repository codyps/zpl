//! ^CV validation labels, Zebra Programming Guide p. 167.
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
use crate::output::Path;

pub(super) fn classify(error: &str) -> Option<u8> {
    if error.contains("not implemented")
        || error.contains("mode unsupported")
        || error.contains("currently requires")
        || error.contains("requires symbol checksums")
    {
        return None;
    }
    if error.contains("checksum") || error.contains("check digit") {
        Some(b'E')
    } else if error.contains("empty") || error.contains("too short") {
        Some(b'S')
    } else if error.contains("does not fit")
        || error.contains("capacity")
        || error.contains("exceeds")
        || error.contains("too long")
    {
        Some(b'L')
    } else if error.contains("character")
        || error.contains("alphabet")
        || error.contains("digits required")
        || error.contains("requires decimal digits")
        || error.contains("ASCII")
    {
        Some(b'C')
    } else if error.contains("invalid")
        || error.contains("must be")
        || error.contains("out of range")
    {
        Some(b'P')
    } else {
        None
    }
}

pub(super) fn render(code: u8, dpi: u32) -> Path {
    // Captured resident validation lettering, 203-DPI ZD621. Independent of
    // ^CF/^A selection; scaled in integral printer-dot units at other DPIs.
    let scale = dpi as f64 / 203.;
    let mut path = Path::default();
    path.rect(0., 0., 156. * scale, 30. * scale);
    let text = [
        b'I', b'N', b'V', b'A', b'L', b'I', b'D', b' ', b'-', b' ', code,
    ];
    for (i, c) in text.into_iter().enumerate() {
        let rows = match c {
            b' ' => [
                0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0x000,
                0x000, 0x000,
            ],
            b'-' => [
                0x000, 0x000, 0x000, 0x000, 0x000, 0x000, 0xffc, 0xffc, 0x000, 0x000, 0x000, 0x000,
                0x000, 0x000,
            ],
            b'A' => [
                0x0c0, 0x1e0, 0x3f0, 0x738, 0xe1c, 0xc0c, 0xc0c, 0xffc, 0xffc, 0xc0c, 0xc0c, 0xc0c,
                0xc0c, 0xc0c,
            ],
            b'C' => [
                0x3f0, 0x7f8, 0xe1c, 0xc0c, 0xc00, 0xc00, 0xc00, 0xc00, 0xc00, 0xc00, 0xc0c, 0xe1c,
                0x7f8, 0x3f0,
            ],
            b'D' => [
                0xfe0, 0xff0, 0xc38, 0xc18, 0xc0c, 0xc0c, 0xc0c, 0xc0c, 0xc0c, 0xc0c, 0xc1c, 0xc38,
                0xff0, 0xfe0,
            ],
            b'E' => [
                0xffc, 0xffc, 0xc00, 0xc00, 0xc00, 0xc00, 0xfe0, 0xfe0, 0xc00, 0xc00, 0xc00, 0xc00,
                0xffc, 0xffc,
            ],
            b'I' => [
                0x3f0, 0x3f0, 0x0c0, 0x0c0, 0x0c0, 0x0c0, 0x0c0, 0x0c0, 0x0c0, 0x0c0, 0x0c0, 0x0c0,
                0x3f0, 0x3f0,
            ],
            b'L' => [
                0xc00, 0xc00, 0xc00, 0xc00, 0xc00, 0xc00, 0xc00, 0xc00, 0xc00, 0xc00, 0xc00, 0xc00,
                0xffc, 0xffc,
            ],
            b'N' => [
                0xc0c, 0xc0c, 0xc0c, 0xe0c, 0xf0c, 0xf8c, 0xdcc, 0xcec, 0xc7c, 0xc3c, 0xc1c, 0xc0c,
                0xc0c, 0xc0c,
            ],
            b'P' => [
                0xff0, 0xff8, 0xc1c, 0xc0c, 0xc0c, 0xc1c, 0xff8, 0xff0, 0xc00, 0xc00, 0xc00, 0xc00,
                0xc00, 0xc00,
            ],
            b'S' => [
                0x3f0, 0x7f8, 0xe1c, 0xc0c, 0xc00, 0xe00, 0x7f0, 0x3f8, 0x01c, 0x00c, 0xc0c, 0xe1c,
                0x7f8, 0x3f0,
            ],
            b'V' => [
                0xc0c, 0xc0c, 0xc0c, 0xc0c, 0xc0c, 0xc0c, 0x618, 0x618, 0x330, 0x330, 0x1e0, 0x1e0,
                0x0c0, 0x0c0,
            ],
            _ => [0; 14],
        };
        for (y, row) in rows.into_iter().enumerate() {
            for x in 0..12 {
                if row & (1 << (11 - x)) != 0 {
                    // Even-odd subpaths cut the white lettering from the panel.
                    path.rect(
                        (12. + i as f64 * 12. + x as f64) * scale,
                        (8. + y as f64) * scale,
                        scale,
                        scale,
                    );
                }
            }
        }
    }
    path
}
