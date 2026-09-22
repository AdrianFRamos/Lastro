#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LIB="$ROOT/chain/programs/lastro/src/lib.rs"
KEYPAIR="$ROOT/chain/target/deploy/lastro-keypair.json"
MARKER='// LASTRO_PROGRAM_ID_BOOTSTRAP_MARKER'
MODE="${1:-}"

case "$MODE" in
  "")
    EPHEMERAL_CI=0
    ;;
  --ephemeral-ci)
    [[ "${CI:-}" == "true" ]] || { echo "--ephemeral-ci requires CI=true" >&2; exit 1; }
    EPHEMERAL_CI=1
    ;;
  *)
    echo "usage: $0 [--ephemeral-ci]" >&2
    exit 2
    ;;
esac

command -v solana-keygen >/dev/null || { echo "solana-keygen is required" >&2; exit 1; }
command -v anchor >/dev/null || { echo "anchor is required" >&2; exit 1; }

if [[ -e "$KEYPAIR" ]]; then
  echo "program keypair already exists at $KEYPAIR; refusing to overwrite it" >&2
  exit 1
fi

if [[ "$EPHEMERAL_CI" -eq 0 ]]; then
  if grep -Eq '^[[:space:]]*declare_id!\(' "$LIB"; then
    echo "lib.rs already contains declare_id!; refusing to generate a second program identity." >&2
    exit 1
  fi
  if ! grep -qF "$MARKER" "$LIB"; then
    echo "bootstrap marker missing from $LIB" >&2
    exit 1
  fi
else
  if ! grep -qF "$MARKER" "$LIB" && ! grep -Eq '^[[:space:]]*declare_id!\(' "$LIB"; then
    echo "lib.rs contains neither the bootstrap marker nor a declared program identity" >&2
    exit 1
  fi
fi

mkdir -p "$(dirname "$KEYPAIR")"
# The normal mode creates the real deployment identity and must be backed up securely.
# The CI mode creates a disposable identity only inside an ephemeral runner checkout.
solana-keygen new --outfile "$KEYPAIR" --no-bip39-passphrase --force
PROGRAM_ID="$(solana-keygen pubkey "$KEYPAIR")"

python3 - "$LIB" "$PROGRAM_ID" "$EPHEMERAL_CI" <<'PY2'
from pathlib import Path
import re
import sys

path = Path(sys.argv[1])
program_id = sys.argv[2]
ephemeral_ci = sys.argv[3] == "1"
marker = "// LASTRO_PROGRAM_ID_BOOTSTRAP_MARKER"
declare = re.compile(r'(?m)^[ \t]*declare_id!\("[1-9A-HJ-NP-Za-km-z]{32,44}"\);[ \t]*$')
source = path.read_text(encoding="utf-8")
replacement = f'declare_id!("{program_id}");'

if marker in source:
    updated = source.replace(marker, replacement, 1)
elif ephemeral_ci:
    matches = list(declare.finditer(source))
    if len(matches) != 1:
        raise SystemExit("expected exactly one declare_id! for ephemeral CI replacement")
    updated = declare.sub(replacement, source, count=1)
else:
    raise SystemExit("program id marker missing")

path.write_text(updated, encoding="utf-8")
PY2

(
  cd "$ROOT/chain"
  anchor keys sync
)

printf '\nProgram ID generated from %s\n' "$KEYPAIR"
printf 'LASTRO_PROGRAM_ID=%s\n' "$PROGRAM_ID"
if [[ "$EPHEMERAL_CI" -eq 1 ]]; then
  printf '%s\n' 'Ephemeral CI identity: never commit the rewritten declare_id! or generated keypair.'
else
  printf '%s\n' 'Back up the keypair securely; do not commit or upload it.'
  printf '%s\n' 'Commit only the public declare_id! change after review.'
fi
