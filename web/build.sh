#!/bin/sh
# Compiles the game to web/www/stardust.wasm. web/www is then a complete static site.
# Needs rustup, because Homebrew's own Rust can't target WebAssembly:
#   rustup toolchain install stable --target wasm32-unknown-unknown
set -eu
cd "$(dirname "$0")/.."

# Name both tools explicitly, so a Homebrew rustc earlier on the PATH can't be used.
RUSTC="$(rustup which --toolchain stable rustc)"
export RUSTC
"$(rustup which --toolchain stable cargo)" build --release --locked -p stardust-web --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/stardust_web.wasm web/www/stardust.wasm
echo "Built web/www/stardust.wasm"
