#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
: "${LASTRO_PROGRAM_ID:?Set LASTRO_PROGRAM_ID}"
: "${LASTRO_SOLANA_RPC_URL:=https://api.devnet.solana.com}"

DECLARED_PROGRAM_ID="$(
  python3 - "$ROOT/chain/programs/lastro/src/lib.rs" <<'PY'
from pathlib import Path
import re
import sys

source = Path(sys.argv[1]).read_text(encoding="utf-8")
if "LASTRO_PROGRAM_ID_BOOTSTRAP_MARKER" in source:
    raise SystemExit("Lastro source program identity is not bootstrapped; commit the real declare_id! before Devnet validation")
matches = re.findall(r'(?m)^\s*declare_id!\("([1-9A-HJ-NP-Za-km-z]{32,44})"\);\s*$', source)
if len(matches) != 1:
    raise SystemExit("Expected exactly one committed Lastro declare_id! before Devnet validation")
print(matches[0])
PY
)"

if [[ "$DECLARED_PROGRAM_ID" != "$LASTRO_PROGRAM_ID" ]]; then
  echo "LASTRO_PROGRAM_ID does not match the committed declare_id!: $DECLARED_PROGRAM_ID" >&2
  exit 1
fi

solana program show "$LASTRO_PROGRAM_ID" --url "$LASTRO_SOLANA_RPC_URL" >/dev/null
printf 'Program %s is visible on %s and matches the committed declare_id!\n' "$LASTRO_PROGRAM_ID" "$LASTRO_SOLANA_RPC_URL"
# This smoke check proves source/deployment identity and deployment visibility only;
# G4 still requires the complete signed flow.
