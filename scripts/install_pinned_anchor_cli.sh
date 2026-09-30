#!/usr/bin/env bash
set -euo pipefail

ANCHOR_VERSION="1.2.0"
ANCHOR_ASSET="anchor-1.2.0-x86_64-unknown-linux-gnu"
ANCHOR_SHA256="0c9c41a3292c281cc6eadb78d6e1c8224d8324a34b0736a89d640fd314db05b7"
ANCHOR_URL="https://github.com/otter-sec/anchor/releases/download/v${ANCHOR_VERSION}/${ANCHOR_ASSET}"

if [[ "$(uname -s)" != "Linux" || "$(uname -m)" != "x86_64" ]]; then
  echo "Pinned CI Anchor installer supports Linux x86_64 only" >&2
  exit 1
fi

install_root="${1:-${RUNNER_TEMP:-/tmp}/lastro-anchor-${ANCHOR_VERSION}}"
download_path="${install_root}.download"
rm -rf "$install_root" "$download_path"
mkdir -p "$install_root/bin"

curl --fail --location --silent --show-error \
  --retry 4 --retry-all-errors --retry-delay 2 \
  "$ANCHOR_URL" -o "$download_path"

printf '%s  %s\n' "$ANCHOR_SHA256" "$download_path" | sha256sum --check --status || {
  echo "Pinned Anchor binary checksum mismatch" >&2
  exit 1
}

mv "$download_path" "$install_root/anchor-${ANCHOR_VERSION}"
chmod +x "$install_root/anchor-${ANCHOR_VERSION}"

cat >"$install_root/bin/anchor" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
export AVM_ACTIVE=1
export CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS=fallback
exec "$(dirname "$0")/../anchor-1.2.0" "$@"
SH
chmod +x "$install_root/bin/anchor"

version="$("$install_root/bin/anchor" --version)"
[[ "$version" == "anchor-cli 1.2.0" ]] || {
  echo "Expected anchor-cli 1.2.0, got: $version" >&2
  exit 1
}

printf '%s\n' "$install_root/bin"
