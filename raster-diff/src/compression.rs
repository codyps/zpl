//! Shared in-tree zlib and checksum primitives for graphics and PNG.
pub(crate) fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}
pub(crate) fn zlib_store(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let count = data.len().div_ceil(65535).max(1);
    for i in 0..count {
        let start = i * 65535;
        let end = (start + 65535).min(data.len());
        let block = &data[start..end];
        out.push(u8::from(i + 1 == count));
        let len = block.len() as u16;
        out.extend(len.to_le_bytes());
        out.extend((!len).to_le_bytes());
        out.extend(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &v in data {
        a = (a + v as u32) % 65521;
        b = (b + a) % 65521;
    }
    out.extend(((b << 16) | a).to_be_bytes());
    out
}
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
}
impl Bits<'_> {
    fn get(&mut self, n: usize) -> Result<usize, String> {
        let mut v = 0;
        for i in 0..n {
            let b = *self
                .data
                .get(self.pos / 8)
                .ok_or("truncated deflate stream")?;
            v |= ((b >> (self.pos % 8)) as usize & 1) << i;
            self.pos += 1;
        }
        Ok(v)
    }
}
struct Huffman(Vec<(usize, usize, usize)>);
impl Huffman {
    fn new(lengths: &[usize]) -> Result<Self, String> {
        let mut counts = [0usize; 16];
        for &n in lengths {
            if n > 15 {
                return Err("invalid Huffman length".into());
            }
            if n > 0 {
                counts[n] += 1
            }
        }
        let mut next = [0usize; 16];
        let mut code = 0;
        for n in 1..16 {
            code = (code + counts[n - 1]) * 2;
            next[n] = code;
            if code + counts[n] > (1 << n) {
                return Err("oversubscribed Huffman tree".into());
            }
        }
        let mut entries = Vec::new();
        for (symbol, &n) in lengths.iter().enumerate() {
            if n > 0 {
                entries.push((n, next[n], symbol));
                next[n] += 1
            }
        }
        Ok(Self(entries))
    }
    fn read(&self, bits: &mut Bits) -> Result<usize, String> {
        let mut code = 0;
        for n in 1..16 {
            code = code * 2 + bits.get(1)?;
            if let Some(e) = self.0.iter().find(|e| e.0 == n && e.1 == code) {
                return Ok(e.2);
            }
        }
        Err("invalid Huffman symbol".into())
    }
}
pub fn inflate(data: &[u8], limit: usize) -> Result<Vec<u8>, String> {
    if data.len() < 6
        || data[0] & 15 != 8
        || data[0] >> 4 > 7
        || !((data[0] as u16) * 256 + data[1] as u16).is_multiple_of(31)
        || data[1] & 32 != 0
    {
        return Err("invalid zlib header".into());
    }
    let mut bits = Bits {
        data: &data[2..data.len() - 4],
        pos: 0,
    };
    let mut out = Vec::new();
    loop {
        let last = bits.get(1)? != 0;
        let kind = bits.get(2)?;
        if kind == 0 {
            bits.pos = bits.pos.div_ceil(8) * 8;
            let n = bits.get(16)?;
            if n ^ bits.get(16)? != 65535 {
                return Err("invalid stored block length".into());
            }
            if out.len() + n > limit {
                return Err("graphic exceeds declared size".into());
            }
            for _ in 0..n {
                out.push(bits.get(8)? as u8)
            }
        } else {
            let (lit, dist) = match kind {
                1 => {
                    let mut lengths = vec![8; 288];
                    lengths[144..256].fill(9);
                    lengths[256..280].fill(7);
                    (Huffman::new(&lengths)?, Huffman::new(&[5; 32])?)
                }
                2 => {
                    let nl = bits.get(5)? + 257;
                    let nd = bits.get(5)? + 1;
                    let nc = bits.get(4)? + 4;
                    let order = [
                        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
                    ];
                    let mut lengths = [0; 19];
                    for &i in &order[..nc] {
                        lengths[i] = bits.get(3)?
                    }
                    let tree = Huffman::new(&lengths)?;
                    let mut all = Vec::new();
                    while all.len() < nl + nd {
                        match tree.read(&mut bits)? {
                            v @ 0..=15 => all.push(v),
                            16 => {
                                let prev = *all.last().ok_or("repeat without predecessor")?;
                                let count = bits.get(2)? + 3;
                                all.extend(std::iter::repeat_n(prev, count))
                            }
                            17 => {
                                let n = bits.get(3)? + 3;
                                all.extend(std::iter::repeat_n(0, n))
                            }
                            18 => {
                                let n = bits.get(7)? + 11;
                                all.extend(std::iter::repeat_n(0, n))
                            }
                            _ => unreachable!(),
                        }
                    }
                    if all.len() != nl + nd || all[256] == 0 {
                        return Err("invalid dynamic Huffman tree".into());
                    }
                    (Huffman::new(&all[..nl])?, Huffman::new(&all[nl..])?)
                }
                _ => return Err("reserved deflate block".into()),
            };
            const LB: [usize; 29] = [
                3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83,
                99, 115, 131, 163, 195, 227, 258,
            ];
            const LE: [usize; 29] = [
                0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5,
                0,
            ];
            const DB: [usize; 30] = [
                1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769,
                1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
            ];
            loop {
                match lit.read(&mut bits)? {
                    v @ 0..=255 => {
                        if out.len() == limit {
                            return Err("graphic exceeds declared size".into());
                        }
                        out.push(v as u8)
                    }
                    256 => break,
                    v @ 257..=285 => {
                        let n = LB[v - 257] + bits.get(LE[v - 257])?;
                        let d = dist.read(&mut bits)?;
                        if d >= 30 {
                            return Err("invalid distance".into());
                        }
                        let distance = DB[d] + bits.get(if d < 4 { 0 } else { d / 2 - 1 })?;
                        if distance > out.len() || out.len() + n > limit {
                            return Err("invalid back reference or graphic size".into());
                        }
                        for _ in 0..n {
                            out.push(out[out.len() - distance])
                        }
                    }
                    _ => return Err("invalid length".into()),
                }
            }
        }
        if last {
            break;
        }
    }
    if bits.pos.div_ceil(8) != bits.data.len() {
        return Err("trailing compressed data".into());
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &v in &out {
        a = (a + v as u32) % 65521;
        b = (b + a) % 65521
    }
    if ((b << 16) | a) != u32::from_be_bytes(data[data.len() - 4..].try_into().unwrap()) {
        return Err("zlib checksum mismatch".into());
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_zlib_vectors() {
        let expected: Vec<u8> = (0..200).flat_map(|_| 0u8..=255).collect();
        for bytes in [
            &include_bytes!("../tests/fixtures/stored.zlib")[..],
            &include_bytes!("../tests/fixtures/fixed.zlib")[..],
            &include_bytes!("../tests/fixtures/dynamic.zlib")[..],
        ] {
            assert_eq!(inflate(bytes, expected.len()).unwrap(), expected);
            assert!(inflate(bytes, expected.len() - 1).is_err());
            for n in (0..bytes.len()).step_by((bytes.len() / 32).max(1)) {
                assert!(inflate(&bytes[..n], expected.len()).is_err())
            }
            let mut bad = bytes.to_vec();
            *bad.last_mut().unwrap() ^= 1;
            assert!(inflate(&bad, expected.len()).is_err());
        }
    }
    #[test]
    fn png_checksums() {
        assert_eq!(crc32(b"123456789"), 0xcbf43926);
        for n in [0, 1, 65535, 65536, 140000] {
            let data = vec![42; n];
            assert_eq!(inflate(&zlib_store(&data), n).unwrap(), data)
        }
    }
}
