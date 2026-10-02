import { readFile } from 'node:fs/promises';
import { performance } from 'node:perf_hooks';
import { initSync, render_png } from '../pkg/zpl_render_api.js';

const binary = await readFile(new URL('../pkg/zpl_render_api_bg.wasm', import.meta.url));
const wasm = initSync({ module: binary });
const encoder = new TextEncoder();
const shipping = '^XA^FO40,40^A0N,32,32^FDShipping label^FS^FO40,100^BY2^BCN,80,N,N,N^FD1234567890^FS^FO40,240^BQN,2,4^FDLA,SHIP-1234567890^FS^XZ';
const dense = '^XA' + Array.from({ length: 48 }, (_, i) => `^FO${20 + i % 3 * 250},${20 + Math.floor(i / 3) * 65}^A0N,32,24^FDField ${i}: ABC123^FS`).join('') + '^XZ';
// 50 * 500 five-segment rectangles exceeds the 100,000 document budget,
// independently of text/barcode path optimizations. Input stays below 1 MiB.
const excessiveBatch = ('^XA' + '^FO2,2^GB10,10,10^FS'.repeat(500) + '^XZ').repeat(50);
const cases = [
  { name: 'shipping-203', input: shipping, width: 812, height: 1218, dpi: 203, status: 200 },
  { name: 'dense-203', input: dense, width: 812, height: 1218, dpi: 203, status: 200 },
  { name: 'shipping-609', input: shipping, width: 2436, height: 3654, dpi: 609, status: 200 },
  { name: 'maximum-canvas', input: '^XA^FO0,0^GB4096,2441,2^FS^XZ', width: 4096, height: 2441, dpi: 609, status: 200 },
  { name: '50-labels', input: '^XA^FO2,2^GB100,80,2^FS^XZ'.repeat(50), width: 812, height: 1218, dpi: 203, status: 200 },
  { name: '50-shipping-labels', input: shipping.repeat(50), width: 812, height: 1218, dpi: 203, status: 200 },
  { name: 'batch-scene-limit', input: excessiveBatch, width: 812, height: 1218, dpi: 203, status: 413 },
  { name: 'graphic-limit', input: '~DGR:STRESS.GRF,25000,125,' + 'AA'.repeat(25_000) + '^XA^XGR:STRESS.GRF,1,1^FS^XZ', width: 812, height: 1218, dpi: 203, status: 413 },
];
console.log(JSON.stringify({ runtime: process.version, wasmBytes: binary.byteLength, note: 'Local Node Wasm wall time, not Cloudflare CPU billing. Memory is Wasm linear-memory high water, excluding the host heap.' }));
for (const item of cases) {
  const input = encoder.encode(item.input);
  const times = [];
  let pngBytes = 0;
  for (let i = 0; i < 13; i++) {
    const start = performance.now();
    const result = render_png(input, item.width, item.height, item.dpi, 0);
    try {
      const body = result.take_body();
      if (result.status !== item.status) {
        const detail = result.status === 200 ? `${body.length} PNG bytes` : new TextDecoder().decode(body);
        throw new Error(`${item.name}: expected ${item.status}, got ${result.status}: ${detail}`);
      }
      pngBytes = item.status === 200 ? body.length : 0;
    } finally { result.free(); }
    if (i >= 3) times.push(performance.now() - start);
  }
  times.sort((a, b) => a - b);
  console.log(JSON.stringify({ fixture: item.name, status: item.status, medianMs: Number(times[5].toFixed(2)), maxMs: Number(times.at(-1).toFixed(2)), pngBytes, wasmMemoryMiB: wasm.memory.buffer.byteLength / 2 ** 20 }));
}
