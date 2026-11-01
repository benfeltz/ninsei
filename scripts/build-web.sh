#!/usr/bin/env bash
# Builds the web version into dist/: ninsei_bg.wasm, ninsei.js, index.html.
# Needs: rustup target add wasm32-unknown-unknown
#        cargo install wasm-bindgen-cli --version <same as wasm-bindgen in Cargo.lock> --locked
# Optional: wasm-opt (binaryen) on PATH shrinks the .wasm further.
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --release --target wasm32-unknown-unknown -p ninsei
rm -rf dist
wasm-bindgen --target web --no-typescript --out-dir dist \
  target/wasm32-unknown-unknown/release/ninsei.wasm
if command -v wasm-opt >/dev/null; then
  wasm-opt -O2 --enable-bulk-memory --enable-nontrapping-float-to-int \
    dist/ninsei_bg.wasm -o dist/ninsei_bg.wasm
fi
cp web/index.html dist/
touch dist/.nojekyll
ls -lh dist
