/*
 * Lastro v2 capture envelope firmware contract: explicit offsets only, never C struct wire
 * copies. Layout and rules mirror crates/lastro-protocol/src/v2/capture.rs and are pinned by
 * test-vectors/v2-capture.json.
 */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "lastro_station/transport.h"

#ifdef __cplusplus
extern "C" {
#endif

#define LASTRO_ENVELOPE_LEN 220u
#define LASTRO_SCHEMA_VERSION 1u
#define LASTRO_MAX_EVENT_AGE_SECONDS 86400
#define LASTRO_EVENT_OBSERVATION_RECORDED 2u
#define LASTRO_EVENT_IDENTIFIER_BOUND 18u
#define LASTRO_EVENT_IDENTIFIER_REPLACED 19u

/** event_id = SHA-256("LASTRO_V2_CAPTURE_EVENT\0" || capture_id). */
bool lastro_capture_event_id(const uint8_t capture_id[16], uint8_t out[32]);

/** Context checks possible before the RFID is read (event type, predecessor, event id). */
bool lastro_capture_command_valid(const lastro_command_payload_t *command);

/**
 * Apply the Station rule for the observed RFID hash and encode the 220-byte envelope:
 * IDENTIFIER_BOUND binds an untagged asset, IDENTIFIER_REPLACED requires a different tag,
 * OBSERVATION_RECORDED requires the active tag. Returns false when the rule is violated.
 */
bool lastro_capture_envelope_encode(
    const lastro_command_payload_t *command,
    const uint8_t station_id[32],
    const uint8_t observed_rfid_hash[32],
    uint8_t out[LASTRO_ENVELOPE_LEN]);

/** event_hash = SHA-256("LASTRO_V2_EVENT\0" || envelope); bound to the Agent ACK. */
bool lastro_envelope_event_hash(const uint8_t envelope[LASTRO_ENVELOPE_LEN], uint8_t out_hash[32]);

#ifdef __cplusplus
}
#endif
