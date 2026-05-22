#!/usr/bin/env sh
set -eu

if ! command -v cargo >/dev/null 2>&1; then
  echo "Rust cargo is required to build the worker backend." >&2
  echo "Install Rust, then run: rustup target add wasm32-unknown-unknown" >&2
  exit 127
fi

if command -v rustup >/dev/null 2>&1; then
  rustup target add wasm32-unknown-unknown >/dev/null
fi

if ! command -v worker-build >/dev/null 2>&1; then
  cargo install worker-build
fi

worker-build --release
