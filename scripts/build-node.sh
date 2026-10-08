#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# Match zpl-wasm/Cargo.toml and the browser build. Node deployment reference:
# https://wasm-bindgen.github.io/wasm-bindgen/reference/deployment.html#nodejs
if ! node_bindgen_path=$(command -v wasm-bindgen); then
  echo 'wasm-bindgen executable not found on PATH.' >&2
  echo 'Install the package: cargo install wasm-bindgen-cli --version 0.2.128 --locked' >&2
  exit 1
fi
node_bindgen_version=$("$node_bindgen_path" --version)
if [[ "$node_bindgen_version" != "wasm-bindgen 0.2.128" ]]; then
  echo "Expected wasm-bindgen 0.2.128; found $node_bindgen_version at $node_bindgen_path." >&2
  echo 'Install the matching CLI: cargo install wasm-bindgen-cli --version 0.2.128 --locked' >&2
  echo 'Put its bin directory before the Nix/system version on PATH when running npm.' >&2
  exit 1
fi
cargo build --locked --release --target wasm32-unknown-unknown -p zpl-wasm
node_target=$(cargo metadata --locked --no-deps --format-version 1 | node -e 'let s="";process.stdin.on("data",d=>s+=d);process.stdin.on("end",()=>console.log(JSON.parse(s).target_directory))')
mkdir -p zpl-node/pkg
"$node_bindgen_path" "$node_target/wasm32-unknown-unknown/release/zpl_wasm.wasm" \
  --target nodejs --out-dir zpl-node/pkg --out-name zpl_wasm
cp LICENSE zpl-node/LICENSE
