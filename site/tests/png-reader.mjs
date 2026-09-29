import assert from 'node:assert/strict';
import { crc32 } from 'node:zlib';

// Independently validate framing and CRC using Node's zlib implementation.
// PNG Third Edition §§5.3, 5.5, 11.3.3.4: https://www.w3.org/TR/png-3/
export function readPng(bytes) {
  const png = Buffer.from(bytes);
  assert.deepEqual([...png.subarray(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10]);
  const text = {};
  const original = [png.subarray(0, 8)];
  let offset = 8;
  while (offset < png.length) {
    const length = png.readUInt32BE(offset);
    const end = offset + length + 12;
    assert.ok(end <= png.length);
    assert.equal(crc32(png.subarray(offset + 4, end - 4)), png.readUInt32BE(end - 4));
    const type = png.toString('ascii', offset + 4, offset + 8);
    if (type === 'iTXt') {
      const data = png.subarray(offset + 8, end - 4);
      const separator = data.indexOf(0);
      assert.deepEqual([...data.subarray(separator, separator + 5)], [0, 0, 0, 0, 0]);
      text[data.toString('ascii', 0, separator)] = data.toString('utf8', separator + 5);
    } else original.push(png.subarray(offset, end));
    if (type === 'IEND') assert.equal(end, png.length);
    offset = end;
  }
  return { text, original: Buffer.concat(original) };
}
