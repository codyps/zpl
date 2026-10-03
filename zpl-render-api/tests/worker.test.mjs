import assert from 'node:assert/strict';
import { after, afterEach, before, test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { setTimeout as sleep } from 'node:timers/promises';
import { createTestHarness } from 'wrangler';
import { inflateSync } from 'node:zlib';

// Real workerd, compiled Wasm, HTTP parsing, and Cloudflare rate-limit bindings.
// Use the deployment configuration for bindings and the production module bundle.
// https://developers.cloudflare.com/workers/testing/test-harness/get-started/
const server = createTestHarness({
  workers: [{ configPath: fileURLToPath(new URL('../wrangler.jsonc', import.meta.url)) }],
});
before(() => server.listen());
afterEach(() => server.reset());
after(() => server.close());
const endpoint = 'https://example.test/v1/printers/8dpmm/labels/4x6/0/';

test('deployed module serves health, compressed PNG, and useful errors', async () => {
  const health = await server.fetch('https://example.test/health');
  assert.equal(health.status, 200);
  assert.equal((await health.json()).status, 'ok');
  const response = await server.fetch(endpoint, { method: 'POST', headers: { 'Content-Type': 'application/x-www-form-urlencoded' }, body: '^XA^FO40,40^A0N,32,32^FDShipping label^FS^FO40,100^BY2^BCN,80,N,N,N^FD1234567890^FS^XZ' });
  assert.equal(response.status, 200, await response.clone().text());
  assert.equal(response.headers.get('X-Total-Count'), '1');
  const png = Buffer.from(await response.arrayBuffer());
  assert.equal(png.readUInt32BE(16), 812);
  assert.equal(png.readUInt32BE(20), 1218);
  assert.ok(png.length < 50_000, `PNG bytes: ${png.length}`);
  const idat = [];
  for (let offset = 8; offset < png.length;) {
    const length = png.readUInt32BE(offset);
    if (png.toString('ascii', offset + 4, offset + 8) === 'IDAT') idat.push(png.subarray(offset + 8, offset + 8 + length));
    offset += length + 12;
  }
  assert.equal(inflateSync(Buffer.concat(idat)).length, 813 * 1218);
  const bad = await server.fetch(endpoint, { method: 'POST', body: '^XA^ZZ^XZ' });
  assert.equal(bad.status, 400);
  assert.match(await bad.text(), /unsupported command/);
});

test('real Wasm rejects limits, preserves binary multipart, and counts labels', async () => {
  const form = new FormData();
  form.set('file', new Blob([Buffer.from('^XA^XZ^XA^FO0,0^GFB,1,1,1,\x80^FS^XZ', 'latin1')]), 'binary.zpl');
  // Serialize the platform FormData before sending it through the harness.
  const upload = new Request(endpoint.replace('/0/', '/1/'), { method: 'POST', body: form });
  const selected = await server.fetch(upload.url, {
    method: 'POST', headers: Object.fromEntries(upload.headers), body: await upload.arrayBuffer(),
  });
  assert.equal(selected.status, 200, await selected.clone().text());
  assert.equal(selected.headers.get('X-Total-Count'), '2');
  // Fixed geometry exceeds the aggregate path budget across otherwise valid labels.
  const excessiveBatch = ('^XA' + '^FO2,2^GB10,10,10^FS'.repeat(500) + '^XZ').repeat(50);
  for (const body of ['^XA^PW10000^XZ', '^XA^XZ'.repeat(51), excessiveBatch]) {
    const response = await server.fetch(endpoint, { method: 'POST', body });
    const message = await response.text();
    assert.equal(response.status, 413, message);
    if (body === excessiveBatch) assert.match(message, /document path limit/);
  }
});

// LabelZoom v2: PNG selects the first scene; PDF retains every scene in order.
// https://docs.labelzoom.com/reference/supported-formats/#multi-label-jobs
test('LabelZoom routes render real PNG and multipage PDF with exact DPI and size overrides', async () => {
  const base = 'https://example.test/api/v2/convert/zpl/to/';
  const body = Buffer.from('^XA^PW300^LL600^FO0,0^GFB,1,1,1,\x80^FS^XZ^XA^PW600^LL300^XZ', 'latin1');
  const headers = { 'Content-Type': 'application/octet-stream', 'CF-Connecting-IP': '192.0.2.51' };
  const pngResponse = await server.fetch(base + 'png?dpi=300', { method: 'POST', headers, body });
  assert.equal(pngResponse.status, 200, await pngResponse.clone().text());
  assert.equal(pngResponse.headers.get('Content-Type'), 'image/png');
  assert.equal(pngResponse.headers.get('X-Total-Count'), '2');
  const png = Buffer.from(await pngResponse.arrayBuffer());
  assert.equal(png.readUInt32BE(16), 300);
  assert.equal(png.readUInt32BE(20), 600);
  const pdfResponse = await server.fetch(base + 'pdf?dpi=300', {
    method: 'POST', headers: { ...headers, Accept: 'application/pdf' }, body,
  });
  assert.equal(pdfResponse.status, 200, await pdfResponse.clone().text());
  assert.equal(pdfResponse.headers.get('Content-Type'), 'application/pdf');
  assert.equal(pdfResponse.headers.get('Cache-Control'), 'no-store');
  assert.equal(pdfResponse.headers.get('X-Total-Count'), '2');
  const pdf = await pdfResponse.text();
  assert.match(pdf, /^%PDF-1.7/);
  assert.match(pdf, /\/Type \/Pages \/Count 2/);
  assert.ok(pdf.indexOf('/MediaBox [0 0 72.000000000000 144.000000000000]') < pdf.indexOf('/MediaBox [0 0 144.000000000000 72.000000000000]'));
  const query = new URLSearchParams({ params: JSON.stringify({ dpi: 203, label: { width: 4, height: 6 } }), dpi: '300' });
  const fixed = await server.fetch(base + `png?${query}`, { method: 'POST', headers, body });
  assert.equal(fixed.status, 200);
  const fixedPng = Buffer.from(await fixed.arrayBuffer());
  assert.equal(fixedPng.readUInt32BE(16), 1200);
  assert.equal(fixedPng.readUInt32BE(20), 1800);
});

test('LabelZoom routes retain Wasm validation and recover after rejected requests', async () => {
  const headers = { 'Content-Type': 'text/plain', 'CF-Connecting-IP': '192.0.2.52' };
  for (const format of ['png', 'pdf']) {
    const url = `https://example.test/api/v2/convert/zpl/to/${format}`;
    for (const [body, status] of [['^XA^XZ^XA^ZZ^XZ', 400], ['^XA^PW10000^XZ', 413], ['^XA^XZ'.repeat(51), 413]]) {
      const rejected = await server.fetch(url, { method: 'POST', headers, body });
      assert.equal(rejected.status, status, await rejected.clone().text());
      assert.match(rejected.headers.get('Content-Type'), /^text\/plain/);
      await rejected.arrayBuffer();
    }
    const recovered = await server.fetch(url, { method: 'POST', headers, body: '^XA^XZ' });
    assert.equal(recovered.status, 200);
    await recovered.arrayBuffer();
  }
});

test('Cloudflare rate-limit binding rejects excess requests', async () => {
  // Miniflare resets counters at wall-clock-aligned 10-second boundaries.
  // Start in a fresh window so a mid-burst reset cannot hide the rejection.
  // https://github.com/cloudflare/workers-sdk/blob/miniflare%405.20261001.0-alpha/packages/miniflare/src/workers/ratelimit/ratelimit-object.worker.ts#L91-L105
  await sleep(10_000 - (Date.now() % 10_000) + 50);
  for (let i = 0; i <= 30; i++) {
    const response = await server.fetch(endpoint, { method: 'POST', body: '^XA^XZ', headers: { 'CF-Connecting-IP': '192.0.2.99' } });
    await response.arrayBuffer();
    assert.equal(response.status, i < 30 ? 200 : 429, `request ${i + 1}`);
    if (i === 30) assert.equal(response.headers.get('Retry-After'), '10');
  }
});

test('workerd handles the largest canvas and rejects graphic expansion without poisoning Wasm', async () => {
  const headers = { 'CF-Connecting-IP': '192.0.2.77' };
  const maximum = await server.fetch('https://example.test/v1/printers/24dpmm/labels/6.726x4.009/0/', {
    method: 'POST', headers, body: '^XA^FO0,0^GB4096,2441,2^FS^XZ',
  });
  assert.equal(maximum.status, 200, await maximum.clone().text());
  const png = Buffer.from(await maximum.arrayBuffer());
  assert.equal(png.readUInt32BE(16), 4096);
  assert.equal(png.readUInt32BE(20), 2441);
  const graphic = '~DGR:STRESS.GRF,25000,125,' + 'AA'.repeat(25_000) + '^XA^XGR:STRESS.GRF,1,1^FS^XZ';
  const rejected = await server.fetch(endpoint, { method: 'POST', headers, body: graphic });
  assert.equal(rejected.status, 413);
  assert.match(await rejected.text(), /graphic path limit/);
  const recovered = await server.fetch(endpoint, { method: 'POST', headers, body: '^XA^XZ' });
  assert.equal(recovered.status, 200);
  await recovered.arrayBuffer();
});
