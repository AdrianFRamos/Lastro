#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LIB="$ROOT/chain/programs/lastro-v2/src/lib.rs"
KEYPAIR="$ROOT/chain/target/deploy/lastro-v2-keypair.json"
MODE="${1:-verify}"

case "$MODE" in
  verify|--verify) ;;
  --generate|--ephemeral-ci) ;;
  *) echo "usage: $0 [verify|--generate|--ephemeral-ci]" >&2; exit 2 ;;
esac

command -v solana-keygen >/dev/null || { echo "solana-keygen is required" >&2; exit 1; }
[[ -f "$LIB" ]] || { echo "missing $LIB" >&2; exit 1; }

if [[ "$MODE" == "--ephemeral-ci" ]]; then
  [[ "${CI:-}" == "true" ]] || { echo "--ephemeral-ci requires CI=true" >&2; exit 1; }
  mkdir -p "$(dirname "$KEYPAIR")"
  rm -f "$KEYPAIR"
  solana-keygen new --outfile "$KEYPAIR" --no-bip39-passphrase --force >/dev/null
  PROGRAM_ID="$(solana-keygen pubkey "$KEYPAIR")"
  python3 - "$LIB" "$PROGRAM_ID" <<'PY'
from pathlib import Path
import re
import sys

path = Path(sys.argv[1])
program_id = sys.argv[2]
source = path.read_text(encoding="utf-8")
pattern = re.compile(r'(?m)^[ \t]*declare_id!\("[1-9A-HJ-NP-Za-km-z]{32,44}"\);[ \t]*$')
updated, count = pattern.subn(f'declare_id!("{program_id}");', source, count=1)
if count != 1:
    raise SystemExit("expected exactly one v2 declare_id! in CI mode")
path.write_text(updated, encoding="utf-8")
PY
  printf 'LASTRO_V2_PROGRAM_ID=%s\n' "$PROGRAM_ID"
  echo 'Ephemeral CI identity generated; do not commit the rewritten source or keypair.'
  exit 0
fi

if [[ "$MODE" == "--generate" ]]; then
  grep -qF 'LASTRO_V2_PROGRAM_ID_BOOTSTRAP_MARKER' "$LIB" || {
    echo "--generate requires the explicit bootstrap marker in lib.rs" >&2
    exit 1
  }
  mkdir -p "$(dirname "$KEYPAIR")"
  [[ ! -e "$KEYPAIR" ]] || { echo "refusing to overwrite existing keypair: $KEYPAIR" >&2; exit 1; }
  solana-keygen new --outfile "$KEYPAIR" --no-bip39-passphrase >/dev/null
  PROGRAM_ID="$(solana-keygen pubkey "$KEYPAIR")"
  python3 - "$LIB" "$PROGRAM_ID" <<'PY'
from pathlib import Path
import sys

path = Path(sys.argv[1])
program_id = sys.argv[2]
marker = '// LASTRO_V2_PROGRAM_ID_BOOTSTRAP_MARKER'
source = path.read_text(encoding='utf-8')
if marker not in source:
    raise SystemExit('bootstrap marker missing')
path.write_text(source.replace(marker, f'declare_id!("{program_id}");', 1), encoding='utf-8')
PY
  (cd "$ROOT/chain" && anchor keys sync >/dev/null)
  printf 'LASTRO_V2_PROGRAM_ID=%s\n' "$PROGRAM_ID"
  echo 'Back up the keypair securely; never commit or upload it.'
  exit 0
fi

[[ -f "$KEYPAIR" ]] || {
  echo "missing local v2 keypair: $KEYPAIR" >&2
  echo 'Restore the deployment keypair or run --generate only from a source containing the explicit marker.' >&2
  exit 1
}
DECLARED_ID="$(sed -nE 's/^[[:space:]]*declare_id!\("([1-9A-HJ-NP-Za-km-z]{32,44})"\);[[:space:]]*$/\1/p' "$LIB")"
[[ -n "$DECLARED_ID" ]] || { echo "no v2 declare_id! found in $LIB" >&2; exit 1; }
KEYPAIR_ID="$(solana-keygen pubkey "$KEYPAIR")"
[[ "$DECLARED_ID" == "$KEYPAIR_ID" ]] || {
  echo "v2 program ID mismatch: source=$DECLARED_ID keypair=$KEYPAIR_ID" >&2
  exit 1
}
printf 'LASTRO_V2_PROGRAM_ID=%s\n' "$DECLARED_ID"
echo 'Verified source program ID and local keypair match.'
