#!/usr/bin/env bash
# Builds the web version into dist/: ninsei_bg.wasm, ninsei.js, index.html.
# Needs: rustup target add wasm32-unknown-unknown
#        cargo install wasm-bindgen-cli --version <same as wasm-bindgen in Cargo.lock> --locked
# No wasm-opt on purpose: older binaryen (e.g. Ubuntu's 108) mangles the reference-types
# tables current Rust emits, and the page dies with a WebAssembly.Table.set() TypeError.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release --target wasm32-unknown-unknown -p ninsei
rm -rf dist
wasm-bindgen --target web --no-typescript --out-dir dist \
  target/wasm32-unknown-unknown/release/ninsei.wasm
cp web/index.html dist/
touch dist/.nojekyll
ls -lh dist
