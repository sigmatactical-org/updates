#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
cargo build --release
mkdir -p build/image
cp -f target/release/sigma-updates build/image/sigma-updates
rm -rf build/image/packages
cp -a packages build/image/packages
echo "Staged: $ROOT/build/image/"
echo "Local image: docker build -f Dockerfile build/image -t sigma-updates:local"
