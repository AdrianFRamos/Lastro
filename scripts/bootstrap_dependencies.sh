#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Lockfiles are resolver outputs. Never hand-write or copy stale Cargo/npm locks.
# Run this on a connected development machine with the pinned toolchain. The committed root
# Cargo.lock is verified in locked mode; only the missing nested Cargo/npm resolver snapshots are generated.
command -v cargo >/dev/null || { echo "cargo not found" >&2; exit 1; }
command -v npm >/dev/null || { echo "npm not found" >&2; exit 1; }

RUST_VERSION="$(rustc --version)"
NODE_VERSION="$(node --version)"
NPM_VERSION="$(npm --version)"
[[ "$RUST_VERSION" =~ ^rustc\ 1\.98\.1([[:space:]]|$) ]] || { echo "Expected rustc 1.98.1, got: $RUST_VERSION" >&2; exit 1; }
[[ "$NODE_VERSION" == "v24.21.0" ]] || { echo "Expected Node v24.21.0, got: $NODE_VERSION" >&2; exit 1; }
[[ "$NPM_VERSION" == "11.19.0" ]] || { echo "Expected npm 11.19.0, got: $NPM_VERSION" >&2; exit 1; }
printf '%s\n' "$RUST_VERSION" "$NODE_VERSION" "npm $NPM_VERSION"

cargo metadata --locked --format-version 1 >/dev/null
cargo generate-lockfile --manifest-path chain/Cargo.toml
npm install --package-lock-only --ignore-scripts
python3 scripts/generate_manifest.py

echo "Verified root Cargo.lock, generated chain/Cargo.lock and package-lock.json, then refreshed MANIFEST.sha256."
echo "Review the resolver outputs and manifest with git diff before commit."
