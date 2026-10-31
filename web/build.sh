#!/bin/sh
# Compiles the game and generates web/www/index.html. web/www is then a complete static site.
# Needs rustup, because Homebrew's own Rust can't target WebAssembly:
#   rustup toolchain install stable --target wasm32-unknown-unknown
set -eu
cd "$(dirname "$0")/.."

# Name both tools explicitly, so a Homebrew rustc earlier on the PATH can't be used.
RUSTC="$(rustup which --toolchain stable rustc)"
export RUSTC
"$(rustup which --toolchain stable cargo)" build --release --locked -p stardust-web --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/stardust_web.wasm web/www/stardust.wasm

# Cloudflare's domain settings let browsers cache .js and .css for 4 hours, but not
# HTML. Giving each release its own asset URLs stops a browser mixing old code with a
# new game file, because the always-fresh page links to the current assets.
version="$(cat web/www/stardust.wasm web/www/app.js web/www/style.css web/index.template.html | cksum | cut -d ' ' -f 1)"
sed "s/__VERSION__/$version/g" web/index.template.html > web/www/index.html
echo "Built web/www (release $version)"
