// Labelary's POST contract: https://labelary.com/service.html sections 1–3, 6.6.
// HTTP negotiation: https://www.rfc-editor.org/rfc/rfc9110.html#section-12.5.1
export const MAX_INPUT_BYTES = 1_048_576;
const MAX_MULTIPART_BYTES = MAX_INPUT_BYTES + 65_536;
const MAX_DIMENSION = 4096;
const MAX_PIXELS = 10_000_000;
// Labelary truncates dpmm * 25.4 to integer DPI, then truncates inch dimensions.
// Verified against its public endpoint with synthetic boxes on 2026-10-02.
const DENSITIES = new Map([['6dpmm', 152], ['8dpmm', 203], ['12dpmm', 304], ['24dpmm', 609]]);
const CORS = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'POST, OPTIONS',
  'Access-Control-Allow-Headers': 'Content-Type, Accept',
  'Access-Control-Expose-Headers': 'X-Total-Count, X-Render-Warnings, X-Renderer-Version, Retry-After',
};

class HttpError extends Error {
  constructor(status, message) { super(message); this.status = status; }
}

function response(body, status, headers = {}) {
  return new Response(body, { status, headers: {
    ...CORS,
    'Content-Type': 'text/plain; charset=utf-8',
    'Cache-Control': 'no-store',
    'X-Content-Type-Options': 'nosniff',
    ...headers,
  } });
}

function parsePath(pathname) {
  const match = /^\/v1\/printers\/([^/]+)\/labels\/([^/]+)x([^/]+)\/(\d+)\/?$/.exec(pathname);
  if (!match) throw new HttpError(404, 'Unknown endpoint. Use POST /v1/printers/8dpmm/labels/4x6/0/');
  const [, density, widthText, heightText, indexText] = match;
  const dpi = DENSITIES.get(density);
  const decimal = /^(?:\d+(?:\.\d*)?|\.\d+)$/;
  const dimensions = [widthText, heightText].map(value => {
    if (value.length > 16 || !decimal.test(value)) return NaN;
    return Number(value);
  });
  const index = Number(indexText);
  if (!dpi || dimensions.some(value => !Number.isFinite(value) || value <= 0 || value > 15)
      || !Number.isSafeInteger(index) || index > 0xffffffff) {
    throw new HttpError(400, 'Invalid density, dimensions, or label index');
  }
  const [width, height] = dimensions.map(value => Math.floor(value * dpi));
  if (!width || !height || width > MAX_DIMENSION || height > MAX_DIMENSION || width * height > MAX_PIXELS) {
    throw new HttpError(400, 'Label exceeds 4096 dots per side or 10 million pixels');
  }
  return { width, height, dpi, index };
}

function acceptsPng(accept) {
  if (!accept?.trim()) return true;
  let bestSpecificity = -1;
  let quality = 0;
  for (const item of accept.split(',')) {
    const [type, ...parameters] = item.trim().toLowerCase().split(';').map(p => p.trim());
    const specificity = type === 'image/png' ? 2 : type === 'image/*' ? 1 : type === '*/*' ? 0 : -1;
    if (specificity < 0) continue;
    const weights = parameters.filter(p => p.startsWith('q='));
    const q = weights.length ? Number(weights[0].slice(2)) : 1;
    if (weights.length > 1 || !Number.isFinite(q) || q < 0 || q > 1) continue;
    if (specificity > bestSpecificity) { bestSpecificity = specificity; quality = q; }
    else if (specificity === bestSpecificity) quality = Math.max(quality, q);
  }
  return quality > 0;
}

async function readBounded(request, maxBytes) {
  const declared = request.headers.get('Content-Length');
  if (declared !== null && (!/^\d+$/.test(declared) || Number(declared) > maxBytes)) {
    throw new HttpError(413, 'Request body exceeds the size limit');
  }
  if (!request.body) return new Uint8Array();
  const reader = request.body.getReader();
  let timer;
  const deadline = new Promise((_, reject) => {
    timer = setTimeout(() => {
      reject(new HttpError(408, 'Request body timed out'));
      void reader.cancel().catch(() => {});
    }, 15_000);
  });
  try {
    // A fixed buffer also bounds overhead from arbitrarily small stream chunks.
    const bytes = new Uint8Array(maxBytes);
    let size = 0;
    while (true) {
      const { done, value } = await Promise.race([reader.read(), deadline]);
      if (done) break;
      if (size + value.byteLength > maxBytes) {
        void reader.cancel().catch(() => {});
        throw new HttpError(413, 'Request body exceeds the size limit');
      }
      bytes.set(value, size);
      size += value.byteLength;
    }
    return bytes.subarray(0, size);
  } finally {
    clearTimeout(timer);
    reader.releaseLock();
  }
}

async function inputBytes(request) {
  const contentType = request.headers.get('Content-Type') ?? 'application/octet-stream';
  const mediaType = contentType.split(';', 1)[0].trim().toLowerCase();
  if (mediaType === 'multipart/form-data') {
    const bytes = await readBounded(request, MAX_MULTIPART_BYTES);
    let form;
    try { form = await new Response(bytes, { headers: { 'Content-Type': contentType } }).formData(); }
    catch { throw new HttpError(400, 'Invalid multipart form'); }
    if ([...form.keys()].length !== 1 || !form.has('file')) {
      throw new HttpError(400, 'Multipart requests must contain exactly one file field');
    }
    const file = form.get('file');
    const input = typeof file === 'string' ? new TextEncoder().encode(file) : new Uint8Array(await file.arrayBuffer());
    if (input.byteLength > MAX_INPUT_BYTES) throw new HttpError(413, 'ZPL exceeds the 1 MiB request limit');
    return input;
  }
  if (!['application/x-www-form-urlencoded', 'application/octet-stream', 'text/plain'].includes(mediaType)) {
    throw new HttpError(415, 'Use a raw ZPL body or multipart/form-data with a file field');
  }
  // Labelary uses raw bytes even for x-www-form-urlencoded: preserve +, %, &, NUL, and binary graphics.
  return readBounded(request, MAX_INPUT_BYTES);
}

export function createHandler({ render, version }) {
  // Limit buffered request bodies sharing one isolate with Wasm's memory.
  let inFlight = 0;
  return async function fetch(request, env) {
    let admitted = false;
    try {
      const url = new URL(request.url);
      if (url.pathname === '/health' && request.method === 'GET') {
        return response(JSON.stringify({ status: 'ok', renderer: version }), 200, { 'Content-Type': 'application/json' });
      }
      const { width, height, dpi, index } = parsePath(url.pathname);
      if (request.method === 'OPTIONS') return response(null, 204, { 'Access-Control-Max-Age': '86400' });
      if (request.method !== 'POST') return response('Use POST with ZPL in the request body', 405, { Allow: 'POST, OPTIONS' });
      if (!acceptsPng(request.headers.get('Accept'))) return response('Only image/png is supported', 406);
      if (request.headers.has('X-Rotation') || request.headers.has('X-Linter')) {
        return response('X-Rotation and X-Linter are not supported', 400);
      }
      // Rate limits are deliberately required: missing deployment bindings fail closed.
      if (!env?.CLIENT_RATE_LIMITER || !env?.SERVICE_RATE_LIMITER) {
        return response('Rate limiting is unavailable', 503, { 'Retry-After': '10' });
      }
      // Cloudflare supplies this header. Never persist or log client addresses.
      const key = request.headers.get('CF-Connecting-IP') ?? 'local';
      if (!(await env.CLIENT_RATE_LIMITER.limit({ key })).success) {
        return response('Rate limit exceeded', 429, { 'Retry-After': '10' });
      }
      if (!(await env.SERVICE_RATE_LIMITER.limit({ key: 'render' })).success) {
        return response('Service is busy', 429, { 'Retry-After': '60' });
      }
      if (inFlight >= 4) return response('Service is busy', 503, { 'Retry-After': '1' });
      inFlight++;
      admitted = true;
      const input = await inputBytes(request);
      if (!input.length) return response('ZPL request body is empty', 400);
      let result;
      try {
        result = render(input, width, height, dpi, index);
        return response(result.take_body(), result.status, {
          'Content-Type': result.status === 200 ? 'image/png' : 'text/plain; charset=utf-8',
          'X-Total-Count': String(result.labels),
          'X-Render-Warnings': String(result.warning_count),
          'X-Renderer-Version': version,
        });
      } finally { result?.free(); }
    } catch (error) {
      if (error instanceof HttpError) return response(error.message, error.status);
      // Renderer errors can contain submitted ZPL. Never log arbitrary exceptions.
      return response('Rendering failed', 500);
    } finally { if (admitted) inFlight--; }
  };
}
