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
  const handler = createHandler({ version: 'test', render: (...args) => {
    calls.push(args);
    return { status: 200, labels: 2, warning_count: 1, take_body: () => new Uint8Array([137, 80, 78, 71]), free: () => freed++, ...overrides };
  } });
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
