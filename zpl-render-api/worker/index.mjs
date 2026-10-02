// Workers imports Wasm as a compiled module; no runtime network fetch is needed.
// https://developers.cloudflare.com/workers/runtime-apis/webassembly/javascript/
import module from '../pkg/zpl_render_api_bg.wasm';
import { initSync, library_version, render_png } from '../pkg/zpl_render_api.js';
import { createHandler } from './http.mjs';

initSync({ module });
export default { fetch: createHandler({ render: render_png, version: library_version() }) };
