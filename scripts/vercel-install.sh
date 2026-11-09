#!/usr/bin/env bash
# Non-interactive, idempotent: Vercel's build image has no Rust toolchain.
set -euo pipefail

export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v cargo >/dev/null 2>&1; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
  export PATH="$HOME/.cargo/bin:$PATH"
fi
if command -v rustup >/dev/null 2>&1; then
  rustup target add wasm32-unknown-unknown
else
  echo "rustup not found; assuming a toolchain that already provides wasm32-unknown-unknown" >&2
  if ! rustc --print target-list | grep -qx wasm32-unknown-unknown; then
    echo "error: rustc does not know wasm32-unknown-unknown" >&2
    exit 1
  fi
fi
npm ci
