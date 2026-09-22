#include "lastro_station/event.h"

#include <string.h>
#include "mbedtls/sha256.h"

#define OFFSET_DEPLOYMENT_ID 8u
#define OFFSET_ANIMAL_ID 40u
#define OFFSET_STATION_ID 72u
#define OFFSET_EVENT_SEQUENCE 104u
#define OFFSET_IDENTITY_REVISION 112u
#define OFFSET_PREVIOUS_EVENT_HASH 116u
#define OFFSET_OLD_RFID_HASH 148u
#define OFFSET_NEW_RFID_HASH 180u
#define OFFSET_FROM_CUSTODIAN 212u
#define OFFSET_TO_CUSTODIAN 244u

static bool all_zero_32(const uint8_t value[32])
{
    uint8_t aggregate = 0;
    for (size_t i = 0; i < 32; ++i) {
        aggregate |= value[i];
    }
    return aggregate == 0;
}

static bool equal_32(const uint8_t left[32], const uint8_t right[32])
{
    return memcmp(left, right, 32) == 0;
}

static void write_u64_le(uint8_t *out, uint64_t value)
{
    for (size_t i = 0; i < 8; ++i) {
        out[i] = (uint8_t)(value >> (8u * i));
    }
}

static void write_u32_le(uint8_t *out, uint32_t value)
{
    for (size_t i = 0; i < 4; ++i) {
        out[i] = (uint8_t)(value >> (8u * i));
    }
}

static bool valid_semantics(const lastro_station_event_fields_t *fields)
{
    if (fields->action == LASTRO_ACTION_ORIGIN) {
        return fields->event_sequence == 1u
            && fields->identity_revision == 1u
            && all_zero_32(fields->previous_event_hash)
            && all_zero_32(fields->old_rfid_hash)
            && !all_zero_32(fields->new_rfid_hash)
            && all_zero_32(fields->from_custodian)
            && !all_zero_32(fields->to_custodian);
    }

    if (fields->action == LASTRO_ACTION_TRANSFER) {
        return fields->event_sequence >= 2u
            && fields->identity_revision >= 1u
            && !all_zero_32(fields->previous_event_hash)
            && !all_zero_32(fields->old_rfid_hash)
            && equal_32(fields->old_rfid_hash, fields->new_rfid_hash)
            && !all_zero_32(fields->from_custodian)
            && !all_zero_32(fields->to_custodian)
            && !equal_32(fields->from_custodian, fields->to_custodian);
    }

    if (fields->action == LASTRO_ACTION_REIDENTIFY) {
        return fields->event_sequence >= 2u
            && fields->identity_revision >= 2u
            && !all_zero_32(fields->previous_event_hash)
            && !all_zero_32(fields->old_rfid_hash)
            && !all_zero_32(fields->new_rfid_hash)
            && !equal_32(fields->old_rfid_hash, fields->new_rfid_hash)
            && !all_zero_32(fields->from_custodian)
            && equal_32(fields->from_custodian, fields->to_custodian);
    }

    return false;
}

bool lastro_event_encode(
    const lastro_station_event_fields_t *fields,
    uint8_t out[LASTRO_STATION_EVENT_LEN])
{
    if (fields == NULL || out == NULL || !valid_semantics(fields)) {
        return false;
    }

    memset(out, 0, LASTRO_STATION_EVENT_LEN);
    out[0] = LASTRO_EVENT_MAGIC_0;
    out[1] = LASTRO_EVENT_MAGIC_1;
    out[2] = LASTRO_EVENT_MAGIC_2;
    out[3] = LASTRO_EVENT_MAGIC_3;
    out[4] = LASTRO_EVENT_VERSION;
    out[5] = (uint8_t)fields->action;
    memcpy(out + OFFSET_DEPLOYMENT_ID, fields->deployment_id, 32);
    memcpy(out + OFFSET_ANIMAL_ID, fields->animal_id, 32);
    memcpy(out + OFFSET_STATION_ID, fields->station_id, 32);
    write_u64_le(out + OFFSET_EVENT_SEQUENCE, fields->event_sequence);
    write_u32_le(out + OFFSET_IDENTITY_REVISION, fields->identity_revision);
    memcpy(out + OFFSET_PREVIOUS_EVENT_HASH, fields->previous_event_hash, 32);
    memcpy(out + OFFSET_OLD_RFID_HASH, fields->old_rfid_hash, 32);
    memcpy(out + OFFSET_NEW_RFID_HASH, fields->new_rfid_hash, 32);
    memcpy(out + OFFSET_FROM_CUSTODIAN, fields->from_custodian, 32);
    memcpy(out + OFFSET_TO_CUSTODIAN, fields->to_custodian, 32);
    return true;
}

bool lastro_event_sha256(
    const uint8_t event_bytes[LASTRO_STATION_EVENT_LEN],
    uint8_t out_hash[32])
{
    if (event_bytes == NULL || out_hash == NULL) {
        return false;
    }
    return mbedtls_sha256(event_bytes, LASTRO_STATION_EVENT_LEN, out_hash, 0) == 0;
}
