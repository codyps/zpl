//! Response-only PNG annotation: never rewrite the original bytes in the cache.
//! PNG §5 (chunk layout/CRC), §11.3.3.2 (tEXt), §11.3.3.4 (iTXt):
//! https://www.w3.org/TR/png-3/#5DataRep
//! https://www.w3.org/TR/png-3/#11tEXt
//! https://www.w3.org/TR/png-3/#11iTXt
use zebra_sgd::PrinterIdentity;

const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const MODEL: &str = "ZPL Printer Model";
const FIRMWARE: &str = "ZPL Printer Firmware";
const SERIAL: &str = "ZPL Printer Serial";
const SOURCE: &str = "ZPL Source";
const CONFIGURATION: &str = "ZPL Printer Configuration";

pub fn annotate(original: &[u8], identity: &PrinterIdentity, zpl: &str) -> eyre::Result<Vec<u8>> {
    // Admission validation bounds the source and rejects NUL. Keep this helper
    // independently safe for PNG iTXt, whose text must not contain NUL.
    eyre::ensure!(
        zpl.len() <= 1_048_576 && !zpl.contains('\0'),
        "invalid PNG ZPL source"
    );
    let chunks = chunks(original)?;
    let mut output = Vec::with_capacity(original.len() + 1024);
    output.extend_from_slice(SIGNATURE);
    for chunk in chunks {
        let kind = &chunk[4..8];
        // Replace only our own keys in the response. All original chunks,
        // including compressed pixels, remain byte-for-byte intact otherwise.
        if matches!(kind, b"tEXt" | b"zTXt" | b"iTXt") {
            let keyword = chunk[8..chunk.len() - 4].split(|b| *b == 0).next().unwrap();
            if [MODEL, FIRMWARE, SERIAL, SOURCE, CONFIGURATION]
                .iter()
                .any(|key| keyword == key.as_bytes())
            {
                continue;
            }
        }
        if kind == b"IEND" {
            for (key, value) in [
                (MODEL, &identity.model),
                (FIRMWARE, &identity.firmware),
                (SERIAL, &identity.serial),
            ] {
                eyre::ensure!(
                    !value.is_empty()
                        && value.len() <= 256
                        && value.bytes().all(|b| (32..=126).contains(&b)),
                    "invalid printer metadata"
                );
                let length = key.len() + 1 + value.len();
                output.extend_from_slice(&(length as u32).to_be_bytes());
                let start = output.len();
                output.extend_from_slice(b"tEXt");
                output.extend_from_slice(key.as_bytes());
                output.push(0);
                output.extend_from_slice(value.as_bytes());
                let crc = crc32fast::hash(&output[start..]);
                output.extend_from_slice(&crc.to_be_bytes());
            }
            write_itxt(&mut output, SOURCE, zpl)?;
            let configuration = serde_json::to_string(&serde_json::json!({
                "schema_version": 1,
                "source": "sgd",
                "capture": "before_preview",
                "settings": identity.configuration,
            }))?;
            write_itxt(&mut output, CONFIGURATION, &configuration)?;
        }
        output.extend_from_slice(chunk);
    }
    Ok(output)
}

fn write_itxt(output: &mut Vec<u8>, key: &str, text: &str) -> eyre::Result<()> {
    // PNG §11.3.3.4: keyword terminator, no compression, compression method 0,
    // empty language tag and translated keyword, followed by exact UTF-8 text.
    let length = u32::try_from(key.len() + 5 + text.len())?;
    output.extend_from_slice(&length.to_be_bytes());
    let start = output.len();
    output.extend_from_slice(b"iTXt");
    output.extend_from_slice(key.as_bytes());
    output.extend_from_slice(&[0; 5]);
    output.extend_from_slice(text.as_bytes());
    let crc = crc32fast::hash(&output[start..]);
    output.extend_from_slice(&crc.to_be_bytes());
    Ok(())
}

fn chunks(png: &[u8]) -> eyre::Result<Vec<&[u8]>> {
    eyre::ensure!(png.starts_with(SIGNATURE), "invalid printer PNG signature");
    let mut remaining = &png[SIGNATURE.len()..];
    let mut chunks = Vec::new();
    let mut has_data = false;
    while !remaining.is_empty() {
        eyre::ensure!(remaining.len() >= 12, "truncated printer PNG chunk");
        let length = u32::from_be_bytes(remaining[..4].try_into()?) as usize;
        eyre::ensure!(length <= remaining.len() - 12, "truncated printer PNG data");
        let (chunk, rest) = remaining.split_at(length + 12);
        let kind = &chunk[4..8];
        eyre::ensure!(
            crc32fast::hash(&chunk[4..length + 8])
                == u32::from_be_bytes(chunk[length + 8..].try_into()?),
            "invalid printer PNG CRC"
        );
        if chunks.is_empty() {
            eyre::ensure!(
                kind == b"IHDR" && length == 13,
                "missing printer PNG header"
            );
        } else {
            eyre::ensure!(kind != b"IHDR", "duplicate printer PNG header");
        }
        has_data |= kind == b"IDAT";
        chunks.push(chunk);
        if kind == b"IEND" {
            eyre::ensure!(
                length == 0 && rest.is_empty() && has_data,
                "invalid printer PNG end"
            );
            return Ok(chunks);
        }
        remaining = rest;
    }
    eyre::bail!("missing printer PNG end")
}

#[cfg(test)]
pub fn test_png() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .add_text_chunk("Original".into(), "preserved".into())
        .unwrap();
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&[0, 255]).unwrap();
    writer.finish().unwrap();
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_is_readable_without_changing_pixels_or_existing_chunks() {
        let original = test_png();
        let identity = PrinterIdentity {
            model: "ZTC ZD621-203dpi ZPL".into(),
            firmware: "V93.21.33Z".into(),
            serial: "TEST-SERIAL".into(),
            configuration: [("ezpl.print_width".into(), "832".into())].into(),
        };
        let zpl = "^XA\r\n^CI28^FO10,20^FDcafé 日本語^FS\n^XZ";
        let annotated = annotate(&original, &identity, zpl).unwrap();
        for chunk in chunks(&original).unwrap() {
            assert!(chunks(&annotated).unwrap().contains(&chunk));
        }
        let mut reader = png::Decoder::new(std::io::Cursor::new(&annotated))
            .read_info()
            .unwrap();
        let mut pixels = [0; 2];
        reader.next_frame(&mut pixels).unwrap();
        reader.finish().unwrap();
        assert_eq!(pixels, [0, 255]);
        let text = &reader.info().uncompressed_latin1_text;
        for (key, value) in [
            (MODEL, identity.model.as_str()),
            (FIRMWARE, identity.firmware.as_str()),
            (SERIAL, identity.serial.as_str()),
            ("Original", "preserved"),
        ] {
            assert!(text.iter().any(|t| t.keyword == key && t.text == value));
        }
        let itxt = &reader.info().utf8_text;
        assert_eq!(
            itxt.iter()
                .find(|t| t.keyword == SOURCE)
                .unwrap()
                .get_text()
                .unwrap(),
            zpl
        );
        let configuration: serde_json::Value = serde_json::from_str(
            &itxt
                .iter()
                .find(|t| t.keyword == CONFIGURATION)
                .unwrap()
                .get_text()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(configuration["capture"], "before_preview");
        assert_eq!(configuration["settings"]["ezpl.print_width"], "832");
        assert_eq!(annotate(&annotated, &identity, zpl).unwrap(), annotated);
        assert!(annotate(&original[..original.len() - 1], &identity, zpl).is_err());
        assert!(annotate(b"not a PNG", &identity, zpl).is_err());
        let mut corrupt = original.clone();
        corrupt[29] ^= 1;
        assert!(annotate(&corrupt, &identity, zpl).is_err());
    }
}
