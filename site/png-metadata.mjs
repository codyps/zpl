// PNG Third Edition §§5.3, 5.5 and 11.3.3.4: chunk framing, CRC, UTF-8 iTXt.
// https://www.w3.org/TR/png-3/#11iTXt
const encoder = new TextEncoder();
const crcTable = Uint32Array.from({ length: 256 }, (_, value) => {
  for (let bit = 0; bit < 8; bit++) value = (value >>> 1) ^ ((value & 1) ? 0xedb88320 : 0);
  return value >>> 0;
});

function textChunk(keyword, text) {
  // Uncompressed text, compression method 0, empty language/translated keyword.
  const data = encoder.encode(`${keyword}\0\0\0\0\0${text}`);
  const chunk = new Uint8Array(data.length + 12);
  const view = new DataView(chunk.buffer);
  view.setUint32(0, data.length);
  chunk.set(encoder.encode('iTXt'), 4);
  chunk.set(data, 8);
  let crc = 0xffffffff;
  for (const byte of chunk.subarray(4, -4)) crc = (crc >>> 8) ^ crcTable[(crc ^ byte) & 255];
  view.setUint32(chunk.length - 4, (crc ^ 0xffffffff) >>> 0);
  return chunk;
}

// The input is the library's PNG output. Preserve every existing chunk and pixel.
// JSON preserves source newlines, Unicode and embedded control bytes losslessly.
export function withPngMetadata(png, metadata) {
  const chunks = [textChunk('Software', `zpl ${metadata.version}`),
    textChunk('ZPL', JSON.stringify(metadata))];
  const end = png.length - 12; // Insert immediately before the final IEND chunk.
  const result = new Uint8Array(png.length + chunks.reduce((n, chunk) => n + chunk.length, 0));
  result.set(png.subarray(0, end));
  let offset = end;
  for (const chunk of chunks) { result.set(chunk, offset); offset += chunk.length; }
  result.set(png.subarray(end), offset);
  return result;
}
