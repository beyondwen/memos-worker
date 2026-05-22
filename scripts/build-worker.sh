#!/usr/bin/env sh
set -eu

if [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1090
  . "$HOME/.cargo/env"
fi
if [ -d "$HOME/.cargo/bin" ]; then
  PATH="$HOME/.cargo/bin:$PATH"
  export PATH
fi
if command -v rustup >/dev/null 2>&1; then
  RUSTUP_CARGO="$(rustup which cargo 2>/dev/null || true)"
  if [ -n "$RUSTUP_CARGO" ]; then
    RUSTUP_BIN="$(dirname "$RUSTUP_CARGO")"
    PATH="$RUSTUP_BIN:$PATH"
    export PATH
  fi
fi

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
