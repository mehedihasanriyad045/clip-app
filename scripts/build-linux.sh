#!/bin/sh
# Builds the Linux bundles inside a container, for copying to an offline machine.
#
# Run from the repo root on Windows:
#   docker run --rm -v "D:/Learning/Tauri/clipapp:/src:ro" -v "D:/Learning/Tauri/clipapp/linux-dist:/out" ubuntu:22.04 sh /src/scripts/build-linux.sh
#
# Built on Ubuntu 22.04, so the result needs glibc 2.35 or newer: Ubuntu 22.04+
# or Debian 12+. It will not run on Ubuntu 20.04.
set -eu

export DEBIAN_FRONTEND=noninteractive

echo "--- system dependencies"
apt-get update -qq
apt-get install -y -qq --no-install-recommends \
  curl ca-certificates build-essential file pkg-config \
  libssl-dev libgtk-3-dev libwebkit2gtk-4.1-dev \
  libayatana-appindicator3-dev librsvg2-dev \
  patchelf desktop-file-utils xdg-utils

echo "--- node"
curl -fsSL https://deb.nodesource.com/setup_22.x | bash - >/dev/null
apt-get install -y -qq nodejs

echo "--- rust"
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --no-modify-path
. "$HOME/.cargo/env"

# The source is mounted read-only and copied in, so the Windows node_modules and
# target/ are left untouched: both hold platform-specific binaries.
#
# target/ and node_modules are excluded DURING the copy, not deleted afterwards:
# a Windows target/ can be several gigabytes, and copying it across the bind
# mount only to delete it wastes more time than the whole build.
echo "--- copying source (excluding build artifacts)"
mkdir -p /build
tar -cf - -C /src   --exclude=./src-tauri/target   --exclude=./node_modules   --exclude=./dist   --exclude=./linux-dist   --exclude=./.git   . | tar -xf - -C /build
cd /build
du -sh /build

echo "--- frontend"
# Container networking drops connections often enough to lose a long build, so
# be patient and retry rather than failing the whole run.
npm config set fetch-retries 5
npm config set fetch-retry-mintimeout 20000
npm config set fetch-retry-maxtimeout 180000
npm config set fetch-timeout 600000

attempt=1
until npm install --no-audit --no-fund; do
  attempt=$((attempt + 1))
  if [ "$attempt" -gt 4 ]; then
    echo "npm install failed after 4 attempts" >&2
    exit 1
  fi
  echo "--- npm install failed, retry $attempt in 10s"
  sleep 10
done

# linuxdeploy normally needs FUSE, which a container does not have; this makes it
# extract itself instead.
export APPIMAGE_EXTRACT_AND_RUN=1
export CARGO_NET_RETRY=10
export CARGO_HTTP_TIMEOUT=120

echo "--- tauri build"
npm run tauri build -- --bundles deb,appimage

echo "--- collecting artifacts"
mkdir -p /out
find src-tauri/target/release/bundle -maxdepth 2 -type f \
  \( -name '*.deb' -o -name '*.AppImage' \) -exec cp -v {} /out/ \;

chmod a+rx /out/*.AppImage 2>/dev/null || true
ls -l /out
echo "--- done"
