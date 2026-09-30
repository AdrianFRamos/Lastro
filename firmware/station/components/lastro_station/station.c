#include "lastro_station/station.h"

#include <string.h>

#include "lastro_station/event.h"
#include "lastro_station/signer.h"
#include "mbedtls/sha256.h"

static const uint8_t STATION_DOMAIN[] = "LASTRO_STATION\0";
#define LASTRO_RFID_WAIT_TIMEOUT_MS 180000u

static lastro_station_state_t g_state = LASTRO_STATION_IDLE;
static lastro_command_payload_t g_command;
static lastro_canonical_rfid_t g_observed_rfid;
static lastro_event_ready_payload_t g_event_ready;
static lastro_error_payload_t g_error;
static uint8_t g_station_pubkey[LASTRO_P256_PUBKEY_COMPRESSED_LEN];
static uint8_t g_station_id[32];
static uint8_t g_event_hash[32];
static bool g_has_observed_rfid;
static bool g_event_ready_available;
static bool g_error_available;
static uint32_t g_rfid_wait_elapsed_ms;

static bool equal_bytes(const uint8_t *left, const uint8_t *right, size_t len)
{
    return memcmp(left, right, len) == 0;
}

static bool same_command(const lastro_command_payload_t *left, const lastro_command_payload_t *right)
{
    return left->event_type == right->event_type
        && left->state_version == right->state_version
        && left->observed_at == right->observed_at
        && left->expires_at == right->expires_at
        && equal_bytes(left->capture_id, right->capture_id, sizeof(left->capture_id))
        && equal_bytes(left->deployment_id, right->deployment_id, sizeof(left->deployment_id))
        && equal_bytes(left->asset_id, right->asset_id, sizeof(left->asset_id))
        && equal_bytes(left->event_id, right->event_id, sizeof(left->event_id))
        && equal_bytes(left->previous_event_hash, right->previous_event_hash, sizeof(left->previous_event_hash))
        && equal_bytes(left->expected_rfid_hash, right->expected_rfid_hash, sizeof(left->expected_rfid_hash));
}

static bool derive_station_id(const uint8_t public_key[LASTRO_P256_PUBKEY_COMPRESSED_LEN], uint8_t out[32])
{
    mbedtls_sha256_context context;
    int status;

    mbedtls_sha256_init(&context);
    status = mbedtls_sha256_starts(&context, 0);
    if (status == 0) status = mbedtls_sha256_update(&context, STATION_DOMAIN, sizeof(STATION_DOMAIN) - 1u);
    if (status == 0) status = mbedtls_sha256_update(&context, public_key, LASTRO_P256_PUBKEY_COMPRESSED_LEN);
    if (status == 0) status = mbedtls_sha256_finish(&context, out);
    mbedtls_sha256_free(&context);
    return status == 0;
}

static void clear_capture(void)
{
    g_rfid_wait_elapsed_ms = 0u;
    memset(&g_command, 0, sizeof(g_command));
    memset(&g_observed_rfid, 0, sizeof(g_observed_rfid));
    memset(&g_event_ready, 0, sizeof(g_event_ready));
    memset(g_event_hash, 0, sizeof(g_event_hash));
    g_has_observed_rfid = false;
    g_event_ready_available = false;
    g_state = LASTRO_STATION_IDLE;
}

static void queue_error_for_capture(const uint8_t capture_id[16], lastro_station_error_code_t code)
{
    memcpy(g_error.capture_id, capture_id, sizeof(g_error.capture_id));
    g_error.code = code;
    g_error_available = true;
}

static void fail_current_capture(lastro_station_error_code_t code)
{
    uint8_t capture_id[16];
    memcpy(capture_id, g_command.capture_id, sizeof(capture_id));
    clear_capture();
    queue_error_for_capture(capture_id, code);
}

bool lastro_station_init(void)
{
    memset(&g_error, 0, sizeof(g_error));
    g_error_available = false;
    clear_capture();
    if (!lastro_signer_public_key(g_station_pubkey)) return false;
    if (!derive_station_id(g_station_pubkey, g_station_id)) return false;
    return true;
}

bool lastro_station_submit_command(
    const lastro_command_payload_t *command,
    lastro_error_payload_t *out_error)
{
    if (command == NULL) return false;

    if (g_state != LASTRO_STATION_IDLE) {
        if (same_command(command, &g_command)) {
            if (g_state == LASTRO_STATION_WAIT_ACK) {
                g_event_ready_available = true;
            }
            return true;
        }
        lastro_error_payload_t error = {0};
        memcpy(error.capture_id, command->capture_id, sizeof(error.capture_id));
        error.code = LASTRO_STATION_ERROR_BUSY;
        if (out_error != NULL) *out_error = error;
        return false;
    }
    if (!lastro_capture_command_valid(command)) {
        lastro_error_payload_t error = {0};
        memcpy(error.capture_id, command->capture_id, sizeof(error.capture_id));
        error.code = LASTRO_STATION_ERROR_INVALID_COMMAND;
        if (out_error != NULL) *out_error = error;
        return false;
    }

    g_command = *command;
    memset(&g_observed_rfid, 0, sizeof(g_observed_rfid));
    memset(&g_event_ready, 0, sizeof(g_event_ready));
    memset(g_event_hash, 0, sizeof(g_event_hash));
    g_has_observed_rfid = false;
    g_event_ready_available = false;
    g_state = LASTRO_STATION_WAIT_RFID;
    g_rfid_wait_elapsed_ms = 0u;
    return true;
}

void lastro_station_elapse(uint32_t elapsed_ms)
{
    if (g_state != LASTRO_STATION_WAIT_RFID) return;
    if (elapsed_ms >= LASTRO_RFID_WAIT_TIMEOUT_MS - g_rfid_wait_elapsed_ms) {
        fail_current_capture(LASTRO_STATION_ERROR_RFID_TIMEOUT);
    } else {
        g_rfid_wait_elapsed_ms += elapsed_ms;
    }
}

bool lastro_station_observe_rfid(const lastro_canonical_rfid_t *rfid)
{
    if (rfid == NULL || g_state != LASTRO_STATION_WAIT_RFID || g_has_observed_rfid) return false;
    g_observed_rfid = *rfid;
    g_has_observed_rfid = true;
    g_state = LASTRO_STATION_BUILD_EVENT;
    return true;
}

void lastro_station_poll(void)
{
    if (g_state == LASTRO_STATION_BUILD_EVENT) {
        if (!g_has_observed_rfid) {
            fail_current_capture(LASTRO_STATION_ERROR_RFID_READ_FAILED);
            return;
        }

        uint8_t observed_hash[32];
        if (!lastro_rfid_hash(&g_observed_rfid, observed_hash)) {
            fail_current_capture(LASTRO_STATION_ERROR_RFID_READ_FAILED);
            return;
        }
        memcpy(g_event_ready.capture_id, g_command.capture_id, sizeof(g_event_ready.capture_id));
        memcpy(g_event_ready.observed_rfid, g_observed_rfid.bytes, sizeof(g_event_ready.observed_rfid));
        memcpy(g_event_ready.station_pubkey33, g_station_pubkey, sizeof(g_event_ready.station_pubkey33));
        /* The envelope encoder enforces the bind/replace/observe RFID rule. */
        if (!lastro_capture_envelope_encode(&g_command, g_station_id, observed_hash, g_event_ready.event_bytes)) {
            fail_current_capture(LASTRO_STATION_ERROR_INVALID_EVENT_CONTEXT);
            return;
        }
        g_state = LASTRO_STATION_SIGN;
    }

    if (g_state == LASTRO_STATION_SIGN) {
        if (!lastro_signer_sign_event(
                g_event_ready.event_bytes,
                sizeof(g_event_ready.event_bytes),
                g_event_ready.station_signature64)) {
            fail_current_capture(LASTRO_STATION_ERROR_SIGNING_FAILED);
            return;
        }
        if (!lastro_envelope_event_hash(g_event_ready.event_bytes, g_event_hash)) {
            fail_current_capture(LASTRO_STATION_ERROR_SIGNING_FAILED);
            return;
        }
        g_event_ready_available = true;
        g_state = LASTRO_STATION_WAIT_ACK;
    }
}

bool lastro_station_take_event_ready(lastro_event_ready_payload_t *out)
{
    if (out == NULL || !g_event_ready_available || g_state != LASTRO_STATION_WAIT_ACK) return false;
    *out = g_event_ready;
    g_event_ready_available = false;
    return true;
}

bool lastro_station_take_error(lastro_error_payload_t *out)
{
    if (out == NULL || !g_error_available) return false;
    *out = g_error;
    g_error_available = false;
    memset(&g_error, 0, sizeof(g_error));
    return true;
}

bool lastro_station_submit_ack(const lastro_ack_payload_t *ack)
{
    if (ack == NULL || g_state != LASTRO_STATION_WAIT_ACK) return false;
    if (!equal_bytes(ack->capture_id, g_command.capture_id, sizeof(ack->capture_id))) return false;
    if (!equal_bytes(ack->event_hash, g_event_hash, sizeof(ack->event_hash))) return false;
    clear_capture();
    return true;
}

lastro_station_state_t lastro_station_state(void)
{
    return g_state;
}
