#!/usr/bin/env bash
# Non-interactive, idempotent: Vercel's build image has no Rust toolchain.
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
  export PATH="$HOME/.cargo/bin:$PATH"
fi
rustup target add wasm32-unknown-unknown
npm ci
