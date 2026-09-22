#!/usr/bin/env bash
set -euo pipefail

SIMULATOR_WIRE_HOST="${LASTRO_HARDWARE_SIM_WIRE_HOST:-hardware-simulator}"
SIMULATOR_WIRE_PORT="${LASTRO_HARDWARE_SIM_WIRE_PORT:-9100}"
SERIAL_LINK="${LASTRO_AGENT_SERIAL_PORT:-/tmp/lastro-station}"
API_HEALTH_URL="${LASTRO_AGENT_API_URL%/}/api/health"

cleanup() {
  if [[ -n "${SOCAT_PID:-}" ]]; then
    kill "$SOCAT_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT INT TERM

for attempt in $(seq 1 60); do
  if curl --fail --silent --max-time 1 "$API_HEALTH_URL" >/dev/null; then
    break
  fi
  if [[ "$attempt" -eq 60 ]]; then
    echo "Lastro API did not become healthy at $API_HEALTH_URL" >&2
    exit 1
  fi
  sleep 1
done

rm -f "$SERIAL_LINK"
socat \
  "PTY,link=${SERIAL_LINK},rawer,echo=0" \
  "TCP:${SIMULATOR_WIRE_HOST}:${SIMULATOR_WIRE_PORT},forever,interval=1,nodelay" &
SOCAT_PID=$!

for attempt in $(seq 1 60); do
  if [[ -e "$SERIAL_LINK" ]]; then
    break
  fi
  if ! kill -0 "$SOCAT_PID" 2>/dev/null; then
    echo "Serial bridge exited before creating $SERIAL_LINK" >&2
    wait "$SOCAT_PID" || true
    exit 1
  fi
  if [[ "$attempt" -eq 60 ]]; then
    echo "Serial bridge did not create $SERIAL_LINK" >&2
    exit 1
  fi
  sleep 0.25
done

exec /usr/local/bin/lastro-agent
