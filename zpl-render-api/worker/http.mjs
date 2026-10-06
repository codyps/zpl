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
  'Access-Control-Allow-Headers': 'Content-Type, Accept, Authorization',
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

// LabelZoom v2: raw ZPL, inches, exact DPI, and dot parameters overriding JSON.
// https://docs.labelzoom.com/reference/conversion-parameters/
// https://api.labelzoom.com/v3/api-docs (convert operation)
function parseLabelZoom(url) {
  const match = /^\/api\/v2\/convert\/([^/]+)\/to\/([^/]+)\/?$/.exec(url.pathname);
  if (!match) throw new HttpError(404, 'Unknown LabelZoom endpoint');
  if (match[1] !== 'zpl' || !['png', 'pdf'].includes(match[2])) {
    throw new HttpError(400, 'Supported conversions are zpl/to/png and zpl/to/pdf');
  }
  if (url.search.length > 8192) throw new HttpError(400, 'Conversion parameters exceed 8 KiB');
  const query = url.searchParams;
  for (const key of new Set(query.keys())) {
    if (query.getAll(key).length !== 1) throw new HttpError(400, 'Duplicate conversion parameter');
  }
  let params = {};
  if (query.has('params')) {
    try { params = JSON.parse(query.get('params')); }
    catch { throw new HttpError(400, 'params must be a JSON object'); }
  }
  const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
  if (!object(params)) throw new HttpError(400, 'params must be a JSON object');
  const values = new Map();
  for (const [key, value] of Object.entries(params)) {
    if (key === 'label') {
      if (!object(value)) throw new HttpError(400, 'label must be an object');
      for (const [axis, size] of Object.entries(value)) values.set(`label.${axis}`, size);
    } else values.set(key, value);
  }
  for (const [key, value] of query) {
    if (key === 'params') continue;
    // Query numbers may use .5 or 4.; never coerce null, booleans or empty strings.
    const number = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:e[+-]?\d+)?$/i.test(value);
    try { values.set(key, number ? Number(value) : JSON.parse(value)); }
    catch { throw new HttpError(400, 'Conversion parameters must be JSON scalar values'); }
  }
  for (const key of values.keys()) {
    if (!['dpi', 'label.width', 'label.height', 'rotation', 'scaling', 'watermark'].includes(key)) {
      throw new HttpError(400, 'Unsupported conversion parameter');
    }
  }
  // Accept explicit neutral settings; reject transformations rather than ignore them.
  for (const [key, neutral] of [['rotation', 0], ['scaling', 100], ['watermark', false]]) {
    if (values.has(key) && values.get(key) !== neutral) {
      throw new HttpError(400, `${key} is not supported except at its default value`);
    }
  }
  const dpi = values.has('dpi') ? values.get('dpi') : 203;
  if (![152, 203, 300, 600].includes(dpi)) throw new HttpError(400, 'dpi must be 152, 203, 300, or 600');
  const [width, height] = ['width', 'height'].map(axis => {
    if (!values.has(`label.${axis}`)) return 0; // Native PW/LL; renderer fallback is 4x6 inches.
    const size = values.get(`label.${axis}`);
    if (typeof size !== 'number' || !Number.isFinite(size) || size <= 0 || size > 15) {
      throw new HttpError(400, 'Label dimensions must be positive numbers no larger than 15 inches');
    }
    const dots = Math.floor(size * dpi);
    if (!dots || dots > MAX_DIMENSION) throw new HttpError(400, 'Label exceeds 4096 dots per side');
    return dots;
  });
  if (width * height > MAX_PIXELS) throw new HttpError(400, 'Label exceeds 10 million pixels');
  return { width, height, dpi, format: match[2], labelzoom: true };
}

function validateLabelZoomHeaders(request, format) {
  // The live OpenAPI contract accepts octet-stream and any text/* for ZPL.
  // Preserve raw bytes, including inline binary graphics; no charset transcoding.
  const type = (request.headers.get('Content-Type') ?? 'application/octet-stream').split(';', 1)[0].trim().toLowerCase();
  if (type !== 'application/octet-stream' && !type.startsWith('text/')) {
    throw new HttpError(400, 'ZPL requires a text/* or application/octet-stream Content-Type');
  }
  const accept = request.headers.get('Accept');
  if (!accept?.trim()) return;
  const types = accept.split(',').map(item => item.split(';', 1)[0].trim().toLowerCase());
  const target = format === 'pdf' ? 'application/pdf' : 'image/png';
  // LabelZoom ignores media-type parameters, including q (Supported formats,
  // "Matching content types"). Labelary keeps RFC quality negotiation above.
  if (types.some(type => [target, `${target.split('/')[0]}/*`, '*/*'].includes(type))) return;
  const supported = types.some(type => ['application/pdf', 'image/png', 'application/*', 'image/*'].includes(type));
  throw new HttpError(supported ? 400 : 406, `Accept must allow ${target}`);
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

export function createHandler({ render, renderLabelZoom, version }) {
  // Limit buffered request bodies sharing one isolate with Wasm's memory.
  let inFlight = 0;
  return async function fetch(request, env) {
    let admitted = false;
    try {
      const url = new URL(request.url);
      if (url.pathname === '/health' && request.method === 'GET') {
        return response(JSON.stringify({ status: 'ok', renderer: version }), 200, { 'Content-Type': 'application/json' });
      }
      const route = url.pathname.startsWith('/api/v2/convert/') ? parseLabelZoom(url) : parsePath(url.pathname);
      const { width, height, dpi, index, labelzoom } = route;
      const contentType = route.format === 'pdf' ? 'application/pdf' : 'image/png';
      if (request.method === 'OPTIONS') return response(null, 204, { 'Access-Control-Max-Age': '86400' });
      if (request.method !== 'POST') return response('Use POST with ZPL in the request body', 405, { Allow: 'POST, OPTIONS' });
      if (labelzoom) validateLabelZoomHeaders(request, route.format);
      else if (!acceptsPng(request.headers.get('Accept'))) return response('Only image/png is supported', 406);
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
      const input = labelzoom ? await readBounded(request, MAX_INPUT_BYTES) : await inputBytes(request);
      if (!input.length) return response('ZPL request body is empty', 400);
      let result;
      try {
        result = labelzoom
          ? renderLabelZoom(input, width, height, dpi, route.format === 'pdf')
          : render(input, width, height, dpi, index);
        return response(result.take_body(), result.status, {
          'Content-Type': result.status === 200 ? contentType : 'text/plain; charset=utf-8',
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
