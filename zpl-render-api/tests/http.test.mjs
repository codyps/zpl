import assert from 'node:assert/strict';
import test from 'node:test';
import { createHandler, MAX_INPUT_BYTES } from '../worker/http.mjs';

const endpoint = 'https://example.test/v1/printers/8dpmm/labels/4x6/0/';
const encoder = new TextEncoder();
const permit = { limit: async () => ({ success: true }) };
const env = { CLIENT_RATE_LIMITER: permit, SERVICE_RATE_LIMITER: permit };
function fixture(overrides = {}) {
  const calls = [];
  let freed = 0;
  const renderer = (...args) => {
    calls.push(args);
    return { status: 200, labels: 2, warning_count: 1, take_body: () => new Uint8Array([137, 80, 78, 71]), free: () => freed++, ...overrides };
  };
  const handler = createHandler({ version: 'test', render: renderer, renderLabelZoom: renderer });
  return { handler, calls, get freed() { return freed; } };
}
function post(body = '^XA^XZ', headers = {}, url = endpoint) {
  return new Request(url, { method: 'POST', body, headers });
}

test('raw form content remains bytes, including binary and form metacharacters', async () => {
  const f = fixture();
  const bytes = new Uint8Array([...encoder.encode('^FDa+b%20&c=1'), 0, 0xff, 0x80]);
  const response = await f.handler(post(bytes, { 'Content-Type': 'application/x-www-form-urlencoded' }), env);
  assert.equal(response.status, 200);
  assert.deepEqual(f.calls[0], [bytes, 812, 1218, 203, 0]);
  assert.equal(response.headers.get('Content-Type'), 'image/png');
  assert.equal(response.headers.get('X-Total-Count'), '2');
  assert.equal(response.headers.get('Cache-Control'), 'no-store');
  assert.equal(response.headers.get('Access-Control-Allow-Origin'), '*');
  assert.equal(f.freed, 1);
});

test('multipart file uploads preserve binary; duplicate and missing fields fail', async () => {
  const f = fixture();
  const bytes = new Uint8Array([94, 88, 65, 0xff, 0, 128]);
  const form = new FormData();
  form.set('file', new Blob([bytes]), 'label.zpl');
  assert.equal((await f.handler(post(form), env)).status, 200);
  assert.deepEqual(f.calls[0][0], bytes);
  form.append('file', 'second');
  assert.equal((await f.handler(post(form), env)).status, 400);
  assert.equal((await f.handler(post(new FormData()), env)).status, 400);
  assert.equal((await f.handler(post('bad', { 'Content-Type': 'multipart/form-data; boundary=none' }), env)).status, 400);
  assert.equal(f.calls.length, 1);
});

test('density, fractional dimensions, and zero-based indices reach the renderer', async () => {
  const f = fixture();
  for (const [density, dpi, width, height] of [[6, 152, 228, 76], [8, 203, 304, 101], [12, 304, 456, 152], [24, 609, 913, 304]]) {
    const url = `https://example.test/v1/printers/${density}dpmm/labels/1.5x.5/2`;
    assert.equal((await f.handler(post('^XA^XZ', {}, url), env)).status, 200);
    assert.deepEqual(f.calls.at(-1).slice(1), [width, height, dpi, 2]);
  }
  // Live Labelary canvas measurement, 2026-10-02; deliberately differs from rounding.
  await f.handler(post('^XA^XZ', {}, 'https://example.test/v1/printers/8dpmm/labels/1.234x2.345/0'), env);
  assert.deepEqual(f.calls.at(-1).slice(1), [250, 476, 203, 0]);
});

test('slow bodies time out and cancel their stream', async context => {
  context.mock.timers.enable({ apis: ['setTimeout'] });
  const f = fixture();
  let cancelled = false;
  const body = new ReadableStream({ cancel() { cancelled = true; } });
  const pending = f.handler(new Request(endpoint, { method: 'POST', body, duplex: 'half' }), env);
  await new Promise(resolve => setImmediate(resolve));
  context.mock.timers.tick(15_000);
  assert.equal((await pending).status, 408);
  assert.equal(cancelled, true);
  assert.equal(f.calls.length, 0);
});

test('rejects malformed routes, impossible canvases, and invalid density before rendering', async () => {
  const f = fixture();
  for (const path of ['9dpmm/labels/4x6/0', '8dpmm/labels/0x6/0', '8dpmm/labels/NaNx6/0', '8dpmm/labels/1e2x6/0', '24dpmm/labels/15x15/0', '8dpmm/labels/4x6/4294967296']) {
    const response = await f.handler(post('^XA^XZ', {}, `https://example.test/v1/printers/${path}`), env);
    assert.equal(response.status, 400, path);
  }
  assert.equal((await f.handler(post('^XA^XZ', {}, endpoint + 'source'), env)).status, 404);
  assert.equal(f.calls.length, 0);
});

test('Accept respects quality and specificity and reports unsupported PDF', async () => {
  const f = fixture();
  for (const accept of ['application/pdf', 'image/png;q=0, */*;q=1', 'image/*;q=0', 'image/png;q=NaN']) {
    assert.equal((await f.handler(post('^XA^XZ', { Accept: accept }), env)).status, 406, accept);
  }
  for (const accept of ['image/png', '*/*', 'image/*', 'application/pdf, image/png;q=0.5']) {
    assert.equal((await f.handler(post('^XA^XZ', { Accept: accept }), env)).status, 200, accept);
  }
});

test('method handling, preflight, and health need no render or rate bindings', async () => {
  const f = fixture();
  const preflight = await f.handler(new Request(endpoint, { method: 'OPTIONS' }), {});
  assert.equal(preflight.status, 204);
  assert.match(preflight.headers.get('Access-Control-Allow-Methods'), /POST/);
  const get = await f.handler(new Request(endpoint), {});
  assert.equal(get.status, 405);
  assert.equal(get.headers.get('Allow'), 'POST, OPTIONS');
  const health = await f.handler(new Request('https://example.test/health'), {});
  assert.deepEqual(await health.json(), { status: 'ok', renderer: 'test' });
  assert.equal(f.calls.length, 0);
});

test('rate limits are enforced before consuming the body; missing bindings fail closed', async () => {
  const f = fixture();
  const reject = { limit: async ({ key }) => { assert.equal(key, '192.0.2.1'); return { success: false }; } };
  const request = post('^XA^XZ', { 'CF-Connecting-IP': '192.0.2.1' });
  const response = await f.handler(request, { ...env, CLIENT_RATE_LIMITER: reject });
  assert.equal(response.status, 429);
  assert.equal(response.headers.get('Retry-After'), '10');
  assert.equal(request.bodyUsed, false);
  assert.equal((await f.handler(post(), {})).status, 503);
  const busy = await f.handler(post(), { ...env, SERVICE_RATE_LIMITER: { limit: async () => ({ success: false }) } });
  assert.equal(busy.status, 429);
  assert.equal(busy.headers.get('Retry-After'), '60');
  assert.equal(f.calls.length, 0);
});

test('enforces actual streamed bytes even without an honest Content-Length', async () => {
  const f = fixture();
  let cancelled = false;
  const stream = new ReadableStream({
    start(controller) { controller.enqueue(new Uint8Array(MAX_INPUT_BYTES)); controller.enqueue(new Uint8Array(1)); },
    cancel() { cancelled = true; },
  });
  const request = new Request(endpoint, { method: 'POST', body: stream, duplex: 'half', headers: { 'Content-Length': '1' } });
  assert.equal((await f.handler(request, env)).status, 413);
  assert.equal(cancelled, true);
  assert.equal((await f.handler(post('small', { 'Content-Length': String(MAX_INPUT_BYTES + 1) }), env)).status, 413);
  const form = new FormData();
  form.set('file', new Blob([new Uint8Array(MAX_INPUT_BYTES + 1)]), 'large.zpl');
  assert.equal((await f.handler(post(form), env)).status, 413);
  assert.equal(f.calls.length, 0);
});

test('rejects empty and unsupported request bodies', async () => {
  const f = fixture();
  assert.equal((await f.handler(post(''), env)).status, 400);
  assert.equal((await f.handler(post('{}', { 'Content-Type': 'application/json' }), env)).status, 415);
  assert.equal((await f.handler(post('^XA^XZ', { 'X-Rotation': '90' }), env)).status, 400);
  assert.equal(f.calls.length, 0);
});

test('bounds concurrent body buffering and releases admission slots', async () => {
  const f = fixture();
  const controllers = [];
  const pending = Array.from({ length: 4 }, () => {
    const body = new ReadableStream({ start(controller) { controllers.push(controller); } });
    return f.handler(new Request(endpoint, { method: 'POST', body, duplex: 'half' }), env);
  });
  await new Promise(resolve => setImmediate(resolve));
  const busy = await f.handler(post(), env);
  assert.equal(busy.status, 503);
  assert.equal(busy.headers.get('Retry-After'), '1');
  for (const controller of controllers) { controller.enqueue(encoder.encode('^XA^XZ')); controller.close(); }
  for (const result of await Promise.all(pending)) assert.equal(result.status, 200);
  assert.equal((await f.handler(post(), env)).status, 200);
});

test('preserves renderer error status and frees Wasm output on success and failure', async () => {
  const f = fixture({ status: 404, take_body: () => encoder.encode('No label at this index') });
  const response = await f.handler(post(), env);
  assert.equal(response.status, 404);
  assert.equal(response.headers.get('X-Total-Count'), '2');
  assert.equal(await response.text(), 'No label at this index');
  assert.equal(f.freed, 1);
  const broken = fixture({ take_body() { throw new Error('secret submitted ZPL'); } });
  const failure = await broken.handler(post(), env);
  assert.equal(failure.status, 500);
  assert.equal(await failure.text(), 'Rendering failed');
  assert.equal(broken.freed, 1);
});

const labelzoom = 'https://example.test/api/v2/convert/zpl/to/';

// LabelZoom conversion parameters: JSON and dot notation, with dot values winning.
// https://docs.labelzoom.com/reference/conversion-parameters/
test('LabelZoom PNG/PDF routes preserve bytes, exact DPI and parameter precedence', async () => {
  const f = fixture();
  const bytes = new Uint8Array([...encoder.encode('^XA^FDa+b%20&c=1'), 0, 0xff, 0x80]);
  for (const dpi of [152, 203, 300, 600]) {
    const query = new URLSearchParams({ params: JSON.stringify({ dpi: 203, label: { width: 4, height: 6 } }), dpi: String(dpi), 'label.width': '1.5' });
    const response = await f.handler(post(bytes, { 'Content-Type': 'application/octet-stream' }, `${labelzoom}png/?${query}`), env);
    assert.equal(response.status, 200);
    assert.deepEqual(f.calls.at(-1), [bytes, Math.floor(1.5 * dpi), 6 * dpi, dpi, false]);
    assert.equal(response.headers.get('Content-Type'), 'image/png');
  }
  const pdf = await f.handler(post('^XA^XZ', { Authorization: 'Bearer unused', Accept: 'application/pdf' }, `${labelzoom}pdf`), env);
  assert.equal(pdf.status, 200);
  assert.equal(pdf.headers.get('Content-Type'), 'application/pdf');
  assert.equal(pdf.headers.get('Cache-Control'), 'no-store');
  assert.equal(pdf.headers.get('X-Total-Count'), '2');
  assert.deepEqual(f.calls.at(-1).slice(1), [0, 0, 203, true]);
  const partial = await f.handler(post('^XA^XZ', {}, `${labelzoom}png?label.height=2&rotation=0&scaling=100&watermark=false`), env);
  assert.equal(partial.status, 200);
  assert.deepEqual(f.calls.at(-1).slice(1), [0, 406, 203, false]);
  const fractional = await f.handler(post('^XA^XZ', {}, `${labelzoom}png?label.width=.5&label.height=1.234`), env);
  assert.equal(fractional.status, 200);
  assert.deepEqual(f.calls.at(-1).slice(1), [101, 250, 203, false]);
  assert.equal(f.freed, f.calls.length);
});

test('LabelZoom rejects invalid, excessive and unsupported parameters before reading bodies', async () => {
  const f = fixture();
  const queries = [
    'params={', 'params=null', 'params=[]', 'params=1', 'params={"label":null}',
    'params={"label":{"depth":2}}', 'params={"dpi":"300"}', 'params={"__proto__":{"dpi":600}}',
    'dpi=0', 'dpi=304', 'dpi=1.5', 'dpi=true', 'dpi=null', 'dpi=', 'dpi=203&dpi=300',
    'params={}&params={}', 'label.width=0', 'label.width=-1', 'label.width=0.0001',
    'label.height=16', 'label.width=15&dpi=600', 'label.width=15&label.height=15&dpi=300',
    'label.width=1e999', 'label.width="4"', 'label.width=false', 'label.width=[]',
    'rotation=90', 'scaling=50', 'watermark=true', 'data=[]', 'pdf.pageNumber=0',
    'zpl.commandsToIgnore=["^PQ"]', 'unknown=0', `params=${' '.repeat(8193)}`,
  ];
  for (const query of queries) {
    const request = post('^XA^XZ', {}, `${labelzoom}png?${query}`);
    const response = await f.handler(request, env);
    assert.equal(response.status, 400, query);
    assert.equal(request.bodyUsed, false);
  }
  for (const path of ['zpl/to/jpeg', 'pdf/to/zpl', 'url/to/png']) {
    assert.equal((await f.handler(post('^XA^XZ', {}, `https://example.test/api/v2/convert/${path}`), env)).status, 400);
  }
  assert.equal((await f.handler(post('^XA^XZ', {}, `${labelzoom}png/extra`), env)).status, 404);
  assert.equal(f.calls.length, 0);
});

// Supported formats, "Matching content types"; live OpenAPI convert description.
// https://docs.labelzoom.com/reference/supported-formats/#matching-content-types
// https://api.labelzoom.com/v3/api-docs
test('LabelZoom validates media types and supports SDK preflight with Authorization', async () => {
  const f = fixture();
  for (const format of ['png', 'pdf']) {
    const url = labelzoom + format;
    const type = format === 'pdf' ? 'application/pdf' : 'image/png';
    for (const accept of [type, '*/*', type.split('/')[0] + '/*', `${type};q=0;charset=utf-8`]) {
      assert.equal((await f.handler(post('^XA^XZ', { Accept: accept, 'Content-Type': 'text/x-zpl' }, url), env)).status, 200);
    }
    assert.equal((await f.handler(post('^XA^XZ', { Accept: 'text/html' }, url), env)).status, 406);
    const mismatch = format === 'pdf' ? 'image/png' : 'application/pdf';
    assert.equal((await f.handler(post('^XA^XZ', { Accept: mismatch }, url), env)).status, 400);
    for (const inputType of ['application/json', 'application/pdf', 'application/x-www-form-urlencoded', 'multipart/form-data']) {
      assert.equal((await f.handler(post('^XA^XZ', { 'Content-Type': inputType }, url), env)).status, 400);
    }
    const preflight = await f.handler(new Request(url, { method: 'OPTIONS', headers: { 'Access-Control-Request-Headers': 'authorization,content-type' } }), {});
    assert.equal(preflight.status, 204);
    assert.match(preflight.headers.get('Access-Control-Allow-Headers'), /Authorization/);
    assert.equal((await f.handler(new Request(url), {})).status, 405);
  }
});

test('LabelZoom shares rate, body and rendering failure protections', async () => {
  const f = fixture();
  for (const format of ['png', 'pdf']) {
    const url = labelzoom + format;
    const request = post('^XA^XZ', {}, url);
    assert.equal((await f.handler(request, {})).status, 503);
    assert.equal(request.bodyUsed, false);
    const limited = await f.handler(request, { ...env, SERVICE_RATE_LIMITER: { limit: async () => ({ success: false }) } });
    assert.equal(limited.status, 429);
    assert.equal(request.bodyUsed, false);
    assert.equal((await f.handler(post(new Uint8Array(MAX_INPUT_BYTES + 1), {}, url), env)).status, 413);
    assert.equal((await f.handler(post('', {}, url), env)).status, 400);
    const broken = fixture({ status: 400, take_body: () => encoder.encode('Unsupported ZPL') });
    const failure = await broken.handler(post('^XA^XZ', {}, url), env);
    assert.equal(failure.status, 400);
    assert.equal(failure.headers.get('Content-Type'), 'text/plain; charset=utf-8');
    assert.equal(await failure.text(), 'Unsupported ZPL');
    assert.equal(broken.freed, 1);
  }
  assert.equal(f.calls.length, 0);
});
