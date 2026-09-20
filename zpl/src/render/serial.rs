//! ^SN initial label data. Print-quantity iteration (^PQ) remains unsupported.
//!
//! Zebra ZPL II Programming Guide, ^SN, pp. 341–342: the rightmost numeric
//! sequence is indexed and N suppresses leading zeros, retaining field width.
//! https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pg-en.pdf
pub(super) fn initial_value(parameters: &[&str]) -> Result<Vec<u8>, String> {
    let value = parameters
        .first()
        .copied()
        .filter(|v| !v.is_empty())
        .unwrap_or("1");
    let step = parameters
        .get(1)
        .copied()
        .filter(|v| !v.is_empty())
        .unwrap_or("1");
    let magnitude = step.strip_prefix('-').unwrap_or(step);
    if magnitude.is_empty()
        || magnitude.len() > 12
        || !magnitude.bytes().all(|v| v.is_ascii_digit())
    {
        return Err("SN increment requires at most 12 decimal digits".into());
    }
    let zeros = parameters
        .get(2)
        .copied()
        .filter(|v| !v.is_empty())
        .unwrap_or("N");
    if !matches!(zeros, "Y" | "N") {
        return Err("SN leading zeros must be Y or N".into());
    }
    Ok(value.as_bytes().to_vec())
}

pub(super) fn suppress_zeros(
    value: &mut [u8],
    parameters: &[&str],
    allow_overlong: bool,
) -> Result<(), String> {
    if let Some(last) = value.iter().rposition(u8::is_ascii_digit) {
        let start = value[..=last]
            .iter()
            .rposition(|v| !v.is_ascii_digit())
            .map_or(0, |v| v + 1);
        if last - start + 1 > 12 {
            return if allow_overlong {
                Ok(())
            } else {
                Err("SN indexed number exceeds 12 digits".into())
            };
        }
        if parameters.get(2).copied().unwrap_or("N") != "Y" {
            // Keep at least one digit for zero; spaces also belong in barcode
            // data, as confirmed by the raw Code 128 reference fixture.
            for digit in &mut value[start..last] {
                if *digit != b'0' {
                    break;
                }
                *digit = b' ';
            }
        }
    }
    Ok(())
}
