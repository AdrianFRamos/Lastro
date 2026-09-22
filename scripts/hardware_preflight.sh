#!/usr/bin/env bash
set -euo pipefail

: "${LASTRO_AGENT_SERIAL_PORT:?Set LASTRO_AGENT_SERIAL_PORT to the real Station device path}"
test -e "$LASTRO_AGENT_SERIAL_PORT" || { echo "serial device not found: $LASTRO_AGENT_SERIAL_PORT" >&2; exit 1; }

check_hex() {
  local name="$1" expected="$2" value="${!1:-}"
  if [[ -n "$value" && ! "$value" =~ ^[0-9A-Fa-f]{$expected}$ ]]; then
    echo "$name must contain exactly $expected hexadecimal characters" >&2
    exit 1
  fi
}

check_hex LASTRO_DEPLOYMENT_ID_HEX 64
check_hex LASTRO_HARDWARE_CUSTODIAN_HEX 64
check_hex LASTRO_HARDWARE_TAG_A_RFID_HEX 16
check_hex LASTRO_HARDWARE_TAG_B_RFID_HEX 16

if [[ "${LASTRO_EFUSE_TEST:-0}" == "1" ]]; then
  : "${LASTRO_EFUSE_SUMMARY_JSON:?Set LASTRO_EFUSE_SUMMARY_JSON to an espefuse --format json artifact}"
  : "${LASTRO_EFUSE_BLOCK_FIELD:?Set the exact selected key-block field name from the summary}"
  : "${LASTRO_EFUSE_PURPOSE_FIELD:?Set the exact selected key-purpose field name from the summary}"
  test -f "$LASTRO_EFUSE_SUMMARY_JSON" || { echo "eFuse summary not found: $LASTRO_EFUSE_SUMMARY_JSON" >&2; exit 1; }
fi

cat <<EOF2
Hardware preflight passed the machine-visible checks only.
Before G1, docs/HARDWARE.md must contain the real reader manufacturer/model/revision/manual,
electrical interface, real frames from at least two tags, integrity rule, and timeout behavior.
The Rust Agent must be stopped while pytest owns $LASTRO_AGENT_SERIAL_PORT.
No command in this preflight writes or burns eFuse state.
EOF2
