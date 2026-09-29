#!/usr/bin/env bash
set -euo pipefail

# Staging E2E runner. It never signs or submits a Solana transaction itself.
# The canonical asset/transformation identifiers must be supplied from a real
# staging deployment where Agent + program + RPC are already initialized.
BASE_URL="${LASTRO_E2E_BASE_URL:-http://127.0.0.1:8080}"
OPERATOR_TOKEN="${LASTRO_E2E_OPERATOR_TOKEN:?set LASTRO_E2E_OPERATOR_TOKEN}"
DEPLOYMENT_ID="${LASTRO_E2E_DEPLOYMENT_ID_HEX:?set LASTRO_E2E_DEPLOYMENT_ID_HEX}"
CANONICAL_ASSET_ID="${LASTRO_E2E_ASSET_ID:-}"
CANONICAL_TRANSFORMATION_ID="${LASTRO_E2E_TRANSFORMATION_ID:-}"
CANONICAL_CARCASS_ASSET_ID="${LASTRO_E2E_CARCASS_ASSET_ID:-}"
RUN_ID="$(python3 - <<'PY'
import uuid
print(uuid.uuid4())
PY
)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

hex32() { openssl rand -hex 32; }
json_post() {
  local path="$1" payload="$2" output="$3"
  curl --fail-with-body --silent --show-error \
    --request POST "$BASE_URL$path" \
    --header "Authorization: Bearer $OPERATOR_TOKEN" \
    --header 'Content-Type: application/json' \
    --data-binary "$payload" >"$output"
}
json_get() {
  local path="$1" output="$2"
  curl --fail-with-body --silent --show-error \
    --request GET "$BASE_URL$path" \
    --header "Authorization: Bearer $OPERATOR_TOKEN" >"$output"
}
assert_json() {
  local file="$1" expression="$2"
  python3 - "$file" "$expression" <<'PY'
import json, sys
path, expression = sys.argv[1:]
with open(path, encoding='utf-8') as handle:
    value = json.load(handle)
if expression == 'object' and not isinstance(value, dict):
    raise SystemExit('expected JSON object')
if expression == 'array' and not isinstance(value, list):
    raise SystemExit('expected JSON array')
print(json.dumps(value, sort_keys=True))
PY
}

printf '%s\n' '[1/9] health'
curl --fail-with-body --silent --show-error "$BASE_URL/api/health" >/dev/null

PRODUCER_ID="$(hex32)"
FACILITY_ID="$(hex32)"
CARRIER_ID="$(hex32)"
PRODUCER_WALLET="$(hex32)"
CARRIER_WALLET="$(hex32)"
FACILITY_CREDENTIAL="$(hex32)"

printf '%s\n' '[2/9] register producer and carrier parties'
json_post /api/v2/parties "$(cat <<JSON
{"partyId":"$PRODUCER_ID","legalName":"Lastro E2E Producer","taxIdHash":null,"wallet":"$PRODUCER_WALLET","role":1}
JSON
)" "$TMP_DIR/producer.json"
json_post /api/v2/parties "$(cat <<JSON
{"partyId":"$CARRIER_ID","legalName":"Lastro E2E Carrier","taxIdHash":null,"wallet":"$CARRIER_WALLET","role":2}
JSON
)" "$TMP_DIR/carrier.json"
assert_json "$TMP_DIR/producer.json" object >/dev/null
assert_json "$TMP_DIR/carrier.json" object >/dev/null

printf '%s\n' '[3/9] register farm/facility'
json_post /api/v2/facilities "$(cat <<JSON
{"facilityId":"$FACILITY_ID","ownerPartyId":"$PRODUCER_ID","facilityType":1,"displayName":"Lastro E2E Farm","credentialHash":"$FACILITY_CREDENTIAL","validFrom":0,"validUntil":4102444800}
JSON
)" "$TMP_DIR/facility.json"
assert_json "$TMP_DIR/facility.json" object >/dev/null

if [[ -z "$CANONICAL_ASSET_ID" ]]; then
  cat <<'MSG'
Registry E2E passed. Canonical-dependent stages were not run because
LASTRO_E2E_ASSET_ID was not supplied. Set it from a real finalized v2 AssetState
and rerun; the runner deliberately refuses to fabricate an asset or transaction.
MSG
  exit 0
fi

LOT_ID="$(hex32)"
printf '%s\n' '[4/9] register lot against canonical v2 asset'
json_post /api/v2/lots "$(cat <<JSON
{"lotId":"$LOT_ID","facilityId":"$FACILITY_ID","ownerPartyId":"$PRODUCER_ID","externalReference":"E2E-$LOT_ID","headCount":1,"liveWeightGrams":1000,"assets":[{"assetId":"$CANONICAL_ASSET_ID","quantity":1,"weightGrams":1000,"role":"ANIMAL"}]}
JSON
)" "$TMP_DIR/lot.json"
assert_json "$TMP_DIR/lot.json" object >/dev/null

printf '%s\n' '[5/9] open and read recall snapshot'
RECALL_ID="$(hex32)"
json_post /api/v2/recalls "$(cat <<JSON
{"recallId":"$RECALL_ID","openedByPartyId":"$PRODUCER_ID","scopeType":"LOT","scopeId":"$LOT_ID","reason":"E2E traceability recall drill"}
JSON
)" "$TMP_DIR/recall.json"
assert_json "$TMP_DIR/recall.json" object >/dev/null
json_get "/api/v2/recalls/$RECALL_ID" "$TMP_DIR/recall-get.json"
assert_json "$TMP_DIR/recall-get.json" object >/dev/null

printf '%s\n' '[6/9] create and deliver shipment'
SHIPMENT_ID="$(hex32)"
json_post /api/v2/shipments "$(cat <<JSON
{"shipmentId":"$SHIPMENT_ID","originFacilityId":"$FACILITY_ID","destinationFacilityId":"$FACILITY_ID","carrierPartyId":"$CARRIER_ID","createdByPartyId":"$PRODUCER_ID","plannedDeparture":"2026-09-28T12:00:00Z","notes":"E2E shipment","items":[{"assetId":"$CANONICAL_ASSET_ID","quantity":1,"weightGrams":1000}]}
JSON
)" "$TMP_DIR/shipment.json"
assert_json "$TMP_DIR/shipment.json" object >/dev/null
json_post "/api/v2/shipments/$SHIPMENT_ID/status" '{"status":"DISPATCHED"}' "$TMP_DIR/shipment-dispatched.json"
json_post "/api/v2/shipments/$SHIPMENT_ID/status" '{"status":"IN_TRANSIT"}' "$TMP_DIR/shipment-transit.json"
json_post "/api/v2/shipments/$SHIPMENT_ID/status" '{"status":"DELIVERED"}' "$TMP_DIR/shipment-delivered.json"

if [[ -n "$CANONICAL_TRANSFORMATION_ID" && -n "$CANONICAL_CARCASS_ASSET_ID" ]]; then
  printf '%s\n' '[7/9] register processing operation linked to finalized transformation'
  OPERATION_ID="$(hex32)"
  json_post /api/v2/processing "$(cat <<JSON
{"operationId":"$OPERATION_ID","facilityId":"$FACILITY_ID","lotId":"$LOT_ID","transformationId":"$CANONICAL_TRANSFORMATION_ID","operatorPartyId":"$PRODUCER_ID","operationKind":"SLAUGHTER","notes":"E2E operation","items":[{"assetId":"$CANONICAL_ASSET_ID","direction":"INPUT","quantity":1,"weightGrams":1000},{"assetId":"$CANONICAL_CARCASS_ASSET_ID","direction":"OUTPUT","quantity":1,"weightGrams":900},{"assetId":null,"direction":"LOSS","quantity":1,"weightGrams":100}]}
JSON
)" "$TMP_DIR/processing.json"
  assert_json "$TMP_DIR/processing.json" object >/dev/null
else
  printf '%s\n' '[7/9] processing skipped: supply LASTRO_E2E_TRANSFORMATION_ID and LASTRO_E2E_CARCASS_ASSET_ID'
fi

printf '%s\n' '[8/9] plan controlled v1 migration'
json_post /api/v2/migrations/v1/plan "$(cat <<JSON
{"runId":"$RUN_ID","requestedByPartyId":"$PRODUCER_ID"}
JSON
)" "$TMP_DIR/migration.json"
assert_json "$TMP_DIR/migration.json" object >/dev/null
json_get "/api/v2/migrations/$RUN_ID" "$TMP_DIR/migration-get.json"
assert_json "$TMP_DIR/migration-get.json" object >/dev/null

printf '%s\n' '[9/9] audit trail'
json_get '/api/v2/audit?limit=100' "$TMP_DIR/audit.json"
assert_json "$TMP_DIR/audit.json" array >/dev/null

printf '%s\n' 'LASTRO_V2_STAGING_E2E=PASS'
