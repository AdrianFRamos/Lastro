#include "lastro_station/event.h"

#include <string.h>
#include "mbedtls/sha256.h"

static const uint8_t CAPTURE_EVENT_DOMAIN[] = "LASTRO_V2_CAPTURE_EVENT\0";
static const uint8_t PAYLOAD_DOMAIN[] = "LASTRO_V2_PAYLOAD\0";
static const uint8_t EVENT_DOMAIN[] = "LASTRO_V2_EVENT\0";
static const uint8_t ZERO32[32] = {0};

static bool all_zero_32(const uint8_t value[32])
{
    uint8_t aggregate = 0;
    for (size_t i = 0; i < 32; ++i) aggregate |= value[i];
    return aggregate == 0;
}

static void write_le(uint8_t *out, uint64_t value, size_t len)
{
    for (size_t i = 0; i < len; ++i) out[i] = (uint8_t)(value >> (8u * i));
}

/* SHA-256(domain_without_trailing_literal_nul || a || b); every domain ends in an explicit \0. */
static bool domain_hash(
    const uint8_t *domain, size_t domain_len,
    const uint8_t *a, size_t a_len,
    const uint8_t *b, size_t b_len,
    uint8_t out[32])
{
    mbedtls_sha256_context context;
    int status;
    mbedtls_sha256_init(&context);
    status = mbedtls_sha256_starts(&context, 0);
    if (status == 0) status = mbedtls_sha256_update(&context, domain, domain_len);
    if (status == 0 && a_len > 0) status = mbedtls_sha256_update(&context, a, a_len);
    if (status == 0 && b_len > 0) status = mbedtls_sha256_update(&context, b, b_len);
    if (status == 0) status = mbedtls_sha256_finish(&context, out);
    mbedtls_sha256_free(&context);
    return status == 0;
}

bool lastro_capture_event_id(const uint8_t capture_id[16], uint8_t out[32])
{
    if (capture_id == NULL || out == NULL) return false;
    return domain_hash(CAPTURE_EVENT_DOMAIN, sizeof(CAPTURE_EVENT_DOMAIN) - 1u, capture_id, 16u, NULL, 0u, out);
}

bool lastro_capture_command_valid(const lastro_command_payload_t *command)
{
    if (command == NULL) return false;
    uint8_t zero_capture[16] = {0};
    if (memcmp(command->capture_id, zero_capture, sizeof(zero_capture)) == 0) return false;

    const bool expects_tag = !all_zero_32(command->expected_rfid_hash);
    bool type_ok;
    switch (command->event_type) {
        case LASTRO_EVENT_IDENTIFIER_BOUND: type_ok = !expects_tag; break;
        case LASTRO_EVENT_IDENTIFIER_REPLACED:
        case LASTRO_EVENT_OBSERVATION_RECORDED: type_ok = expects_tag; break;
        default: type_ok = false; break;
    }
    /* The predecessor is not tied to the version (custody transfers advance state_version
     * without an event); the program enforces the exact pair against AssetState. */
    if (!type_ok || command->state_version == 0u) return false;

    uint8_t expected_event_id[32];
    if (!lastro_capture_event_id(command->capture_id, expected_event_id)) return false;
    return memcmp(expected_event_id, command->event_id, 32) == 0;
}

bool lastro_capture_envelope_encode(
    const lastro_command_payload_t *command,
    const uint8_t station_id[32],
    const uint8_t observed_rfid_hash[32],
    uint8_t out[LASTRO_ENVELOPE_LEN])
{
    if (station_id == NULL || observed_rfid_hash == NULL || out == NULL) return false;
    if (!lastro_capture_command_valid(command)) return false;
    if (command->observed_at < 0 || command->expires_at < command->observed_at
        || command->expires_at - command->observed_at > LASTRO_MAX_EVENT_AGE_SECONDS) {
        return false;
    }

    const uint8_t *old_hash;
    const bool same_tag = memcmp(observed_rfid_hash, command->expected_rfid_hash, 32) == 0;
    switch (command->event_type) {
        case LASTRO_EVENT_IDENTIFIER_BOUND: old_hash = ZERO32; break;
        case LASTRO_EVENT_IDENTIFIER_REPLACED:
            if (same_tag) return false;
            old_hash = command->expected_rfid_hash;
            break;
        case LASTRO_EVENT_OBSERVATION_RECORDED:
            if (!same_tag) return false;
            old_hash = observed_rfid_hash;
            break;
        default: return false;
    }

    uint8_t payload_hash[32];
    if (!domain_hash(PAYLOAD_DOMAIN, sizeof(PAYLOAD_DOMAIN) - 1u, old_hash, 32u, observed_rfid_hash, 32u, payload_hash)) {
        return false;
    }

    memset(out, 0, LASTRO_ENVELOPE_LEN);
    write_le(out + 0, LASTRO_SCHEMA_VERSION, 2u);
    write_le(out + 2, command->event_type, 2u);
    memcpy(out + 4, command->deployment_id, 32);
    memcpy(out + 36, command->asset_id, 32);
    memcpy(out + 68, command->event_id, 32);
    write_le(out + 100, command->state_version, 8u);
    memcpy(out + 108, command->previous_event_hash, 32);
    memcpy(out + 140, payload_hash, 32);
    memcpy(out + 172, station_id, 32);
    write_le(out + 204, (uint64_t)command->observed_at, 8u);
    write_le(out + 212, (uint64_t)command->expires_at, 8u);
    return true;
}

bool lastro_envelope_event_hash(const uint8_t envelope[LASTRO_ENVELOPE_LEN], uint8_t out_hash[32])
{
    if (envelope == NULL || out_hash == NULL) return false;
    return domain_hash(EVENT_DOMAIN, sizeof(EVENT_DOMAIN) - 1u, envelope, LASTRO_ENVELOPE_LEN, NULL, 0u, out_hash);
}
