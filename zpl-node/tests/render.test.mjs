import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { test } from 'node:test';
import { render, libraryVersion } from '@codyps/zpl';

const options = { width: 32, height: 24, profile: 'specification' };
const source = '^XA^FO2,2^GB8,6,2^FS^XZ';

test('ESM and CommonJS share the public API', () => {
  const cjs = createRequire(import.meta.url)('@codyps/zpl');
  assert.equal(cjs.render, render);
  assert.match(libraryVersion(), /^\d+\.\d+\.\d+/);
});

test('PNG returns owned bytes and native dimensions', () => {
  const result = render(source, options);
  assert.ok(Buffer.isBuffer(result.data));
  assert.deepEqual([...result.data.subarray(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10]);
  assert.equal(result.data.readUInt32BE(16), 32);
  assert.equal(result.data.readUInt32BE(20), 24);
  assert.deepEqual([result.width, result.height, result.labels], [32, 24, 1]);
  const saved = Buffer.from(result.data);
  render('^XA^XZ', options);
  assert.deepEqual(result.data, saved);
  assert.ok(Array.isArray(result.warnings));
});

test('SVG, multipage PDF, and selecting a label', () => {
  const input = source + '^XA^PW48^LL16^XZ';
  const svg = render(input, { ...options, format: 'svg', label: 1 });
  assert.match(svg.data.toString(), /<svg/);
  assert.deepEqual([svg.width, svg.height, svg.labels], [48, 16, 2]);
  const pdf = render(input, { ...options, format: 'pdf' }).data.toString('latin1');
  assert.match(pdf, /^%PDF-/);
  assert.match(pdf, /\/Count 2\b/);
  const single = render(input, { ...options, format: 'pdf', label: 1 }).data.toString('latin1');
  assert.match(single, /\/Count 1\b/);
});

test('binary graphics preserve invalid UTF-8 and typed array offsets', () => {
  // ^GF binary payload: one 8-dot row. Zebra Programming Guide, ^GF:
  // https://www.zebra.com/content/dam/support-dam/en/documentation/unrestricted/guide/software/zpl-zbi2-pm-en.pdf
  const bytes = Buffer.concat([Buffer.from('^XA^FO0,0^GFB,1,1,1,'), Buffer.from([0x80]), Buffer.from('^FS^XZ')]);
  const padded = Buffer.concat([Buffer.from('junk'), bytes, Buffer.from('junk')]);
  const view = new Uint8Array(padded.buffer, padded.byteOffset + 4, bytes.length);
  const expected = render('^XA^FO0,0^GFA,1,1,1,80^FS^XZ', options).data;
  assert.deepEqual(render(view, options).data, expected);
});

test('rejects invalid options before Wasm integer coercion and forwards renderer errors', () => {
  for (const width of [-1, 0, 1.5, NaN, Infinity, 2 ** 32, '32', null]) {
    assert.throws(() => render(source, { ...options, width }), RangeError);
  }
  assert.throws(() => render(source, { label: -1 }), RangeError);
  assert.throws(() => render(source, { format: 'jpg' }), TypeError);
  assert.throws(() => render(source, { profile: 'unknown' }), TypeError);
  assert.throws(() => render(source, null), TypeError);
  assert.throws(() => render({}), TypeError);
  assert.throws(() => render('', options), /ZPL byte 0: no labels/);
  assert.throws(() => render(source, { ...options, label: 1 }), /No label/);
  assert.throws(() => render('x'.repeat(1_048_577), options), /limit/);
});

test('profile defaults match Rust and specification honors PW/LL', () => {
  assert.equal(render('^XA^XZ', { format: 'svg' }).width, 832);
  assert.equal(render('^XA^XZ', { format: 'svg', profile: 'specification' }).width, 812);
  assert.equal(render('^XA^XZ', { format: 'svg', profile: 'zq610-plus' }).width, 384);
  const result = render('^XA^PW17^LL19^XZ', { ...options, format: 'svg' });
  assert.deepEqual([result.width, result.height], [17, 19]);
});
