#!/usr/bin/env bash
set -euo pipefail

SOLANA_VERSION="4.2.0"
SOLANA_ARCHIVE="solana-release-x86_64-unknown-linux-gnu.tar.bz2"
SOLANA_SHA256="1f5eb13bcf3694dbd3cf634602aee5edcf8eab519acac75778391c979c3002b0"
SOLANA_URL="https://github.com/anza-xyz/agave/releases/download/v${SOLANA_VERSION}/${SOLANA_ARCHIVE}"

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "x86_64" ]]; then
  echo "Pinned CI Solana installer supports Linux x86_64 only" >&2
  exit 1
fi

install_root="${1:-${RUNNER_TEMP:-/tmp}/lastro-solana-${SOLANA_VERSION}}"
archive_path="${install_root}.tar.bz2"
rm -rf "$install_root" "$archive_path"
mkdir -p "$install_root"

curl --fail --location --silent --show-error \
  --retry 4 --retry-all-errors --retry-delay 2 \
  "$SOLANA_URL" -o "$archive_path"

printf '%s  %s\n' "$SOLANA_SHA256" "$archive_path" | sha256sum --check --status || {
  echo "Pinned Solana archive checksum mismatch" >&2
  exit 1
}

tar -xjf "$archive_path" -C "$install_root"
rm -f "$archive_path"

bin_dir="$install_root/solana-release/bin"
for command_name in solana solana-keygen solana-test-validator cargo-build-sbf; do
  [[ -x "$bin_dir/$command_name" ]] || {
    echo "Pinned Solana archive is missing executable: $command_name" >&2
    exit 1
  }
done

version="$("$bin_dir/solana" --version)"
[[ "$version" =~ ^solana-cli\ 4\.2\.0([[:space:]]|$) ]] || {
  echo "Expected solana-cli 4.2.0, got: $version" >&2
  exit 1
}

printf '%s\n' "$bin_dir"
