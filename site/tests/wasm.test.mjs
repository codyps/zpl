import { readFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
import test from 'node:test';
import { withPngMetadata } from '../png-metadata.mjs';
import { readPng } from './png-reader.mjs';
import init, { render_preview, library_version } from '../../_site/pkg/zpl_wasm.js';

await init({ module_or_path: await readFile(new URL('../../_site/pkg/zpl_wasm_bg.wasm', import.meta.url)) });

test('compiled Wasm renders PNG, SVG and PDF with actual label dimensions', () => {
  const result = render_preview('^XA^PW120^LL80^FO10,10^GB40,20,3^FS^XZ', 812, 1218, 203, 0);
  try {
    assert.equal(result.width, 120);
    assert.equal(result.height, 80);
    assert.deepEqual([...result.png().slice(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10]);
    assert.match(new TextDecoder().decode(result.svg()), /<svg/);
    assert.match(new TextDecoder().decode(result.pdf()), /^%PDF-1\.7/);
  } finally { result.free(); }
});
test('bundled page example renders', async () => {
  const html = await readFile(new URL('../index.html', import.meta.url), 'utf8');
  const source = html.match(/<textarea[^>]*>([\s\S]*?)<\/textarea>/)[1];
  const result = render_preview(source, 812, 1218, 203, 0);
  try { assert.equal(result.labels, 1); assert.ok(result.png().length > 100); }
  finally { result.free(); }
});
test('errors cross the JS boundary, and later renders still work', () => {
  assert.throws(() => render_preview('', 100, 80, 203, 0), /label/);
  assert.throws(() => render_preview('^XA^XZ', 0, 80, 203, 0), /dimensions/);
  assert.throws(() => render_preview('x'.repeat(1_048_577), 100, 80, 203, 0), /1 MiB/);
  const result = render_preview('^XA^XZ^XA^XZ', 100, 80, 203, 1);
  try { assert.equal(result.labels, 2); } finally { result.free(); }
});


test('PNG metadata preserves Unicode source and original image chunks', async () => {
  const manifest = await readFile(new URL('../../zpl/Cargo.toml', import.meta.url), 'utf8');
  assert.equal(library_version(), manifest.match(/^version = "([^"]+)"/m)[1]);
  const result = render_preview('^XA^XZ', 100, 80, 203, 0);
  try {
    const metadata = { source: '^XA\r\n^FDcafé 日本語\0^FS^XZ', version: library_version(),
      width: 100, height: 80, dpi: 203, label: 1, profile: 'ZD621_203_DPI' };
    const png = result.png();
    const decoded = readPng(withPngMetadata(png, metadata));
    assert.deepEqual(JSON.parse(decoded.text.ZPL), metadata);
    assert.equal(decoded.text.Software, `zpl ${library_version()}`);
    assert.deepEqual(decoded.original, Buffer.from(png));
  } finally { result.free(); }
});


test('PDF selects the requested label and preserves physical dimensions', () => {
  const source = '^XA^PW203^LL406^XZ^XA^PW406^LL203^FO10,10^GB20,20,20^FS^XZ';
  for (const [label, size] of [[0, [72, 144]], [1, [144, 72]]]) {
    const result = render_preview(source, 812, 1218, 203, label);
    try {
      const pdf = new TextDecoder().decode(result.pdf());
      const box = pdf.match(/\/MediaBox \[([^\]]+)\]/)[1].split(' ').map(Number);
      assert.deepEqual(box, [0, 0, ...size]);
      assert.match(pdf, /\/Count 1 /);
      assert.equal(pdf.includes('f*'), label === 1);
    } finally { result.free(); }
  }
  const result = render_preview('^XA^XZ', 4096, 1, 1, 0);
  try { assert.match(new TextDecoder().decode(result.pdf()), /\/UserUnit 21 /); }
  finally { result.free(); }
});
