#!/usr/bin/env bash
set -e
cargo +stable build --manifest-path boot/Cargo.toml --release
cargo +stable build --manifest-path kernel/Cargo.toml --release
echo "✅ Boot + kernel built."
