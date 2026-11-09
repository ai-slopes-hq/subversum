#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

# 1. Rust engine -> wasm
cargo build --release --target wasm32-unknown-unknown -p versum-wasm
mkdir -p web/public
cp target/wasm32-unknown-unknown/release/versum_wasm.wasm web/public/engine.wasm

# 2. New app -> dist/next (vite empties dist/next only)
npx vite build

# 3. Legacy app -> dist root (sw.js, index.html, icons, ...)
if [ -e legacy/next ]; then
  echo "error: legacy/next exists and would collide with dist/next" >&2
  exit 1
fi
cp -R legacy/. dist/
find dist -maxdepth 1 -name '*.test.mjs' -delete
