#pragma once

#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "unity.h"
#include "lastro_station/event.h"

#define FIXTURE_ORIGIN_EVENT_HEX "4c53545201010000d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0111111111111111111111111111111111111111111111111111111111111111156c266d8ab41a37e7a3bb80b33f6249c05bdb965c68bbeb4775303c69594df77010000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008a604528f19062cc9ada87a5e225a5c967e2cf148c85d86daf02d04212ebf6e10000000000000000000000000000000000000000000000000000000000000000a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1"
#define FIXTURE_TRANSFER_EVENT_HEX "4c53545201020000d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0111111111111111111111111111111111111111111111111111111111111111156c266d8ab41a37e7a3bb80b33f6249c05bdb965c68bbeb4775303c69594df770200000000000000010000005845dc20fd6b266ec98399f0aa93c736ec9aa778bf038af5291e5df81334b5318a604528f19062cc9ada87a5e225a5c967e2cf148c85d86daf02d04212ebf6e18a604528f19062cc9ada87a5e225a5c967e2cf148c85d86daf02d04212ebf6e1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2"
#define FIXTURE_REIDENTIFY_EVENT_HEX "4c53545201030000d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0111111111111111111111111111111111111111111111111111111111111111156c266d8ab41a37e7a3bb80b33f6249c05bdb965c68bbeb4775303c69594df77030000000000000002000000a709b5d24fb31448ff34c1522368a0b5ad3dddfb3993964c1a7d509caa4c801e8a604528f19062cc9ada87a5e225a5c967e2cf148c85d86daf02d04212ebf6e11c5bb72358bd6303c86a4969bef05eed30f9a6b3daac8ad4207bc27f4f40f978b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2"
#define FIXTURE_ORIGIN_EVENT_HASH_HEX "5845dc20fd6b266ec98399f0aa93c736ec9aa778bf038af5291e5df81334b531"
#define FIXTURE_TRANSFER_EVENT_HASH_HEX "a709b5d24fb31448ff34c1522368a0b5ad3dddfb3993964c1a7d509caa4c801e"
#define FIXTURE_RFID_A_HEX "8000130000000001"
#define FIXTURE_RFID_A_HASH_HEX "8a604528f19062cc9ada87a5e225a5c967e2cf148c85d86daf02d04212ebf6e1"
#define FIXTURE_RFID_B_HEX "8000130000000002"
#define FIXTURE_RFID_B_HASH_HEX "1c5bb72358bd6303c86a4969bef05eed30f9a6b3daac8ad4207bc27f4f40f978"
#define FIXTURE_STATION_ID_HEX "56c266d8ab41a37e7a3bb80b33f6249c05bdb965c68bbeb4775303c69594df77"
#define FIXTURE_STATION_PUBKEY_HEX "036b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
#define FIXTURE_ORIGIN_SIGNATURE_HEX "09579de39fbd8108dea2fc89262240c45b2043d85e6e16854d9bed737e6426e109be548b3eb9d3ba3252aeba61af230a18115aeaf0b74734df88c90955f9184f"
#define FIXTURE_CAPTURE_ID_HEX "00112233445566778899aabbccddeeff"
#define FIXTURE_COMMAND_HEX "00112233445566778899aabbccddeeff01000000d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d01111111111111111111111111111111111111111111111111111111111111111010000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1"
#define FIXTURE_EVENT_READY_HEX "00112233445566778899aabbccddeeff4c53545201010000d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0d0111111111111111111111111111111111111111111111111111111111111111156c266d8ab41a37e7a3bb80b33f6249c05bdb965c68bbeb4775303c69594df77010000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000008a604528f19062cc9ada87a5e225a5c967e2cf148c85d86daf02d04212ebf6e10000000000000000000000000000000000000000000000000000000000000000a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a18000130000000001036b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c29609579de39fbd8108dea2fc89262240c45b2043d85e6e16854d9bed737e6426e109be548b3eb9d3ba3252aeba61af230a18115aeaf0b74734df88c90955f9184f"
#define FIXTURE_ACK_HEX "00112233445566778899aabbccddeeff5845dc20fd6b266ec98399f0aa93c736ec9aa778bf038af5291e5df81334b531"
#define FIXTURE_ERROR_HEX "00112233445566778899aabbccddeeff02000000"

static int fixture_hex_nibble(char value)
{
    if (value >= '0' && value <= '9') return value - '0';
    if (value >= 'a' && value <= 'f') return value - 'a' + 10;
    return -1;
}

static void fixture_decode_hex(const char *hex, uint8_t *out, size_t out_len)
{
    TEST_ASSERT_NOT_NULL(hex);
    TEST_ASSERT_NOT_NULL(out);
    TEST_ASSERT_EQUAL_UINT32((uint32_t)(out_len * 2u), (uint32_t)strlen(hex));
    for (size_t i = 0; i < out_len; ++i) {
        const int high = fixture_hex_nibble(hex[i * 2u]);
        const int low = fixture_hex_nibble(hex[i * 2u + 1u]);
        TEST_ASSERT_GREATER_OR_EQUAL_INT(0, high);
        TEST_ASSERT_GREATER_OR_EQUAL_INT(0, low);
        out[i] = (uint8_t)((high << 4) | low);
    }
}

static void fixture_fill32(uint8_t out[32], uint8_t value)
{
    memset(out, value, 32u);
}

static lastro_station_event_fields_t fixture_origin_fields(void)
{
    lastro_station_event_fields_t fields = {0};
    fields.action = LASTRO_ACTION_ORIGIN;
    fixture_fill32(fields.deployment_id, 0xd0u);
    fixture_fill32(fields.animal_id, 0x11u);
    fixture_decode_hex(FIXTURE_STATION_ID_HEX, fields.station_id, 32u);
    fields.event_sequence = 1u;
    fields.identity_revision = 1u;
    fixture_decode_hex(FIXTURE_RFID_A_HASH_HEX, fields.new_rfid_hash, 32u);
    fixture_fill32(fields.to_custodian, 0xa1u);
    return fields;
}

static lastro_station_event_fields_t fixture_transfer_fields(void)
{
    lastro_station_event_fields_t fields = {0};
    fields.action = LASTRO_ACTION_TRANSFER;
    fixture_fill32(fields.deployment_id, 0xd0u);
    fixture_fill32(fields.animal_id, 0x11u);
    fixture_decode_hex(FIXTURE_STATION_ID_HEX, fields.station_id, 32u);
    fields.event_sequence = 2u;
    fields.identity_revision = 1u;
    fixture_decode_hex(FIXTURE_ORIGIN_EVENT_HASH_HEX, fields.previous_event_hash, 32u);
    fixture_decode_hex(FIXTURE_RFID_A_HASH_HEX, fields.old_rfid_hash, 32u);
    fixture_decode_hex(FIXTURE_RFID_A_HASH_HEX, fields.new_rfid_hash, 32u);
    fixture_fill32(fields.from_custodian, 0xa1u);
    fixture_fill32(fields.to_custodian, 0xb2u);
    return fields;
}

static lastro_station_event_fields_t fixture_reidentify_fields(void)
{
    lastro_station_event_fields_t fields = {0};
    fields.action = LASTRO_ACTION_REIDENTIFY;
    fixture_fill32(fields.deployment_id, 0xd0u);
    fixture_fill32(fields.animal_id, 0x11u);
    fixture_decode_hex(FIXTURE_STATION_ID_HEX, fields.station_id, 32u);
    fields.event_sequence = 3u;
    fields.identity_revision = 2u;
    fixture_decode_hex(FIXTURE_TRANSFER_EVENT_HASH_HEX, fields.previous_event_hash, 32u);
    fixture_decode_hex(FIXTURE_RFID_A_HASH_HEX, fields.old_rfid_hash, 32u);
    fixture_decode_hex(FIXTURE_RFID_B_HASH_HEX, fields.new_rfid_hash, 32u);
    fixture_fill32(fields.from_custodian, 0xb2u);
    fixture_fill32(fields.to_custodian, 0xb2u);
    return fields;
}
