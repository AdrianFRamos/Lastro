/* StationEvent firmware contract: explicit offsets only, never C struct wire copies. */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define LASTRO_STATION_EVENT_LEN 276u
#define LASTRO_EVENT_MAGIC_0 'L'
#define LASTRO_EVENT_MAGIC_1 'S'
#define LASTRO_EVENT_MAGIC_2 'T'
#define LASTRO_EVENT_MAGIC_3 'R'
#define LASTRO_EVENT_VERSION 1u

typedef enum {
    LASTRO_ACTION_ORIGIN = 1,
    LASTRO_ACTION_TRANSFER = 2,
    LASTRO_ACTION_REIDENTIFY = 3,
} lastro_action_t;

typedef struct {
    lastro_action_t action;
    uint8_t deployment_id[32];
    uint8_t animal_id[32];
    uint8_t station_id[32];
    uint64_t event_sequence;
    uint32_t identity_revision;
    uint8_t previous_event_hash[32];
    uint8_t old_rfid_hash[32];
    uint8_t new_rfid_hash[32];
    uint8_t from_custodian[32];
    uint8_t to_custodian[32];
} lastro_station_event_fields_t;

/** Encode one canonical 276-byte StationEvent after action-specific semantic validation. */
bool lastro_event_encode(
    const lastro_station_event_fields_t *fields,
    uint8_t out[LASTRO_STATION_EVENT_LEN]);

/** SHA-256 over the exact 276 bytes that are signed and transported. */
bool lastro_event_sha256(
    const uint8_t event_bytes[LASTRO_STATION_EVENT_LEN],
    uint8_t out_hash[32]);

#ifdef __cplusplus
}
#endif
