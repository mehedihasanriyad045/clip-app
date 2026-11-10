#!/bin/sh
# Compile-checks the Linux build without bundling, for fast iteration.
#
#   docker run --rm -v "D:/Learning/Tauri/clipapp:/src:ro" \
#     -v clipapp-cargo:/root/.cargo -v clipapp-target:/target \
#     -v clipapp-apt:/var/cache/apt/archives \
#     ubuntu:22.04 sh /src/scripts/check-linux.sh
#
# The named volumes persist the apt cache, crate registry and build output, so a
# second run takes a fraction of the first.
set -eu

export DEBIAN_FRONTEND=noninteractive
export CARGO_TARGET_DIR=/target
export CARGO_NET_RETRY=10

echo "--- system dependencies"
apt-get update -qq
apt-get install -y -qq --no-install-recommends \
  curl ca-certificates build-essential file pkg-config \
  libssl-dev libgtk-3-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev

echo "--- rust"
if [ ! -x "$HOME/.cargo/bin/cargo" ]; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs |
    sh -s -- -y --profile minimal --no-modify-path
fi
. "$HOME/.cargo/env"

echo "--- copying source"
rm -rf /build
mkdir -p /build
tar -cf - -C /src --exclude=./src-tauri/target --exclude=./node_modules \
  --exclude=./dist --exclude=./linux-dist --exclude=./.git . | tar -xf - -C /build

# cargo check needs the frontend directory to exist, but not a real build.
mkdir -p /build/dist
echo '<!doctype html><title>placeholder</title>' > /build/dist/index.html

echo "--- cargo check"
cd /build/src-tauri
cargo check --all-targets 2>&1 | tail -60

echo "--- cargo test"
cargo test --lib 2>&1 | tail -20

echo "--- done"
