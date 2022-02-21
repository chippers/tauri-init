#!/usr/bin/env sh
set -xe

# build the project with cargo
cargo +nightly build --release \
  -Z build-std=std,panic_abort \
  -Z build-std-features=panic_immediate_abort \
  --target aarch64-apple-darwin

# simply decrease the size a bit for faster up/downloads
strip target/aarch64-apple-darwin/release/tauri-init
