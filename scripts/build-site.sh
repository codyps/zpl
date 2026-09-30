#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Keep this CLI version aligned with zpl-wasm/Cargo.toml.
# https://wasm-bindgen.github.io/wasm-bindgen/reference/deployment.html
if [[ "$(wasm-bindgen --version)" != "wasm-bindgen 0.2.128" ]]; then
  echo 'Install wasm-bindgen-cli 0.2.128 (cargo install wasm-bindgen-cli --version 0.2.128 --locked).' >&2
  exit 1
fi
cargo build --locked --release --target wasm32-unknown-unknown -p zpl-wasm
site_target=$(cargo metadata --no-deps --format-version 1 | node -e 'let s="";process.stdin.on("data",d=>s+=d);process.stdin.on("end",()=>console.log(JSON.parse(s).target_directory))')
mkdir -p _site/pkg
wasm-bindgen "$site_target/wasm32-unknown-unknown/release/zpl_wasm.wasm" --target web --out-dir _site/pkg --out-name zpl_wasm
# Explicit allowlist: never publish the checkout, specifications, or captures.
cp site/index.html site/style.css site/app.mjs site/worker.mjs site/png-metadata.mjs site/svg-metadata.mjs _site/
mkdir -p _site/fonts
cp site/fonts/PatrickHand-Regular.ttf site/fonts/OFL.txt _site/fonts/
cp LICENSE _site/LICENSE
