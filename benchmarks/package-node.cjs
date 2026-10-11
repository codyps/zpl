// Public package API, including binding overhead and PNG encoding.
// https://nodejs.org/api/process.html#processhrtimebigint
'use strict';
const fs = require('node:fs');
const [packagePath, mode, fixture, width, height, milliseconds, output] = process.argv.slice(2);
const started = process.hrtime.bigint();
const api = require(packagePath);
if (typeof api.render !== 'function') throw new Error('Missing public render API');
if (mode === 'initialize') {
  console.log(JSON.stringify({iterations: 1, ns: Number(process.hrtime.bigint() - started), checksum: 1}));
} else {
  const input = fs.readFileSync(fixture);
  const options = {format: 'png', profile: 'zd621', dpi: 203, width: Number(width), height: Number(height)};
  const render = () => api.render(input, options);
  const first = render();
  if (first.labels !== 1 || first.width !== options.width || first.height !== options.height || !first.data.length) {
    throw new Error('Invalid package output');
  }
  fs.writeFileSync(output, first.data);
  let warmup = process.hrtime.bigint();
  while (process.hrtime.bigint() - warmup < 250_000_000n) render();
  const duration = BigInt(milliseconds) * 1_000_000n;
  let iterations = 0, checksum = 0;
  const start = process.hrtime.bigint();
  let elapsed;
  do {
    for (let i = 0; i < 8; i++) checksum += render().data.length;
    iterations += 8;
    elapsed = process.hrtime.bigint() - start;
  } while (elapsed < duration);
  console.log(JSON.stringify({iterations, ns: Number(elapsed), checksum}));
}
