#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Match the existing browser adapter's pinned bindings version.
# https://wasm-bindgen.github.io/wasm-bindgen/reference/deployment.html
if [[ "$(wasm-bindgen --version)" != "wasm-bindgen 0.2.128" ]]; then
  echo 'Install wasm-bindgen-cli 0.2.128 (cargo install wasm-bindgen-cli --version 0.2.128 --locked).' >&2
  exit 1
fi
cargo build --locked --release --target wasm32-unknown-unknown -p zpl-render-api
worker_target=$(cargo metadata --no-deps --format-version 1 | node -e 'let s="";process.stdin.on("data",d=>s+=d);process.stdin.on("end",()=>console.log(JSON.parse(s).target_directory))')
mkdir -p zpl-render-api/pkg
wasm-bindgen "$worker_target/wasm32-unknown-unknown/release/zpl_render_api.wasm" \
  --target web --out-dir zpl-render-api/pkg --out-name zpl_render_api
