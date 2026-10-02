# ZPL renderer API

Public Cloudflare Worker serving the local Rust renderer through Labelary-style
POST endpoints. The Rust Wasm adapter emits compressed PNGs; the JavaScript
handler handles HTTP, binary uploads, CORS, and rate limits.

See the [API contract and deployment guide](../docs/worker-api.md) for limits,
hosting costs, build/test commands, and the configured Cloudflare account.

```sh
npm ci
npm run build
npm test
npm run test:worker
npm run dev
```

The build requires the Rust Wasm target and wasm-bindgen CLI 0.2.128. This crate
is a deployment artifact and is not published to crates.io.
