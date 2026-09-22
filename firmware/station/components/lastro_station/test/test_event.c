#include "unity.h"
#include "lastro_station/event.h"
#include "test_fixture.h"

static void assert_event_fixture(lastro_station_event_fields_t fields, const char *expected_hex)
{
    uint8_t actual[LASTRO_STATION_EVENT_LEN];
    uint8_t expected[LASTRO_STATION_EVENT_LEN];
    fixture_decode_hex(expected_hex, expected, sizeof(expected));
    TEST_ASSERT_TRUE(lastro_event_encode(&fields, actual));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(expected, actual, sizeof(actual));
}

TEST_CASE("event_origin_matches_fixture_byte_for_byte", "[lastro][contract]")
{
    /* PURPOSE: Keep C ORIGIN encoding byte-identical to the shared cross-language fixture. */
    /* ARRANGE: Build semantic fields from the committed frozen vector values. */
    /* ACTION: Encode one StationEvent. */
    /* ASSERT: All 276 bytes equal the repository fixture. */
    /* FAILURE MEANS: firmware and the shared protocol disagree on ORIGIN wire encoding. */
    assert_event_fixture(fixture_origin_fields(), FIXTURE_ORIGIN_EVENT_HEX);
}

TEST_CASE("event_transfer_matches_fixture_byte_for_byte", "[lastro][contract]")
{
    /* PURPOSE: Keep C TRANSFER encoding byte-identical to the shared cross-language fixture. */
    /* ARRANGE: Build semantic fields from the committed frozen vector values. */
    /* ACTION: Encode one StationEvent. */
    /* ASSERT: All 276 bytes equal the repository fixture. */
    /* FAILURE MEANS: firmware and the shared protocol disagree on TRANSFER wire encoding. */
    assert_event_fixture(fixture_transfer_fields(), FIXTURE_TRANSFER_EVENT_HEX);
}

TEST_CASE("event_reidentify_matches_fixture_byte_for_byte", "[lastro][contract]")
{
    /* PURPOSE: Keep C REIDENTIFY encoding byte-identical to the shared cross-language fixture. */
    /* ARRANGE: Build semantic fields from the committed frozen vector values. */
    /* ACTION: Encode one StationEvent. */
    /* ASSERT: All 276 bytes equal the repository fixture. */
    /* FAILURE MEANS: firmware and the shared protocol disagree on REIDENTIFY wire encoding. */
    assert_event_fixture(fixture_reidentify_fields(), FIXTURE_REIDENTIFY_EVENT_HEX);
}

TEST_CASE("event_integers_are_little_endian", "[lastro][contract]")
{
    /* PURPOSE: Make integer encoding independent of CPU endianness. */
    /* ARRANGE: Use asymmetric sequence/revision values in an otherwise valid TRANSFER. */
    /* ACTION: Encode StationEvent. */
    /* ASSERT: sequence and revision bytes use explicit little-endian order at frozen offsets. */
    /* FAILURE MEANS: signed bytes can differ across architectures or protocol layers. */
    lastro_station_event_fields_t fields = fixture_transfer_fields();
    uint8_t encoded[LASTRO_STATION_EVENT_LEN];
    const uint8_t expected_sequence[8] = {1, 2, 3, 4, 5, 6, 7, 8};
    const uint8_t expected_revision[4] = {1, 2, 3, 4};
    fields.event_sequence = UINT64_C(0x0807060504030201);
    fields.identity_revision = UINT32_C(0x04030201);

    TEST_ASSERT_TRUE(lastro_event_encode(&fields, encoded));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(expected_sequence, encoded + 104u, sizeof(expected_sequence));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(expected_revision, encoded + 112u, sizeof(expected_revision));
}

TEST_CASE("event_reserved_is_always_zero", "[lastro][contract]")
{
    /* PURPOSE: Preserve one canonical encoding for signed StationEvent bytes. */
    /* ARRANGE: Build a valid ORIGIN. */
    /* ACTION: Encode StationEvent. */
    /* ASSERT: Both reserved header bytes are zero. */
    /* FAILURE MEANS: multiple encodings could represent the same logical event. */
    lastro_station_event_fields_t fields = fixture_origin_fields();
    uint8_t encoded[LASTRO_STATION_EVENT_LEN];
    TEST_ASSERT_TRUE(lastro_event_encode(&fields, encoded));
    TEST_ASSERT_EQUAL_UINT8(0u, encoded[6]);
    TEST_ASSERT_EQUAL_UINT8(0u, encoded[7]);
}

TEST_CASE("event_rejects_invalid_origin_semantics", "[lastro][contract]")
{
    /* PURPOSE: Prevent the Station from signing an impossible ORIGIN. */
    /* ARRANGE: Make previous_event_hash nonzero in an otherwise valid ORIGIN. */
    /* ACTION: Encode StationEvent. */
    /* ASSERT: Encoding fails before signing. */
    /* FAILURE MEANS: firmware can sign an ORIGIN that canonical protocol rules reject. */
    lastro_station_event_fields_t fields = fixture_origin_fields();
    uint8_t encoded[LASTRO_STATION_EVENT_LEN];
    fields.previous_event_hash[0] = 1u;
    TEST_ASSERT_FALSE(lastro_event_encode(&fields, encoded));
}

TEST_CASE("event_rejects_invalid_transfer_semantics", "[lastro][contract]")
{
    /* PURPOSE: TRANSFER must preserve RFID and identity revision. */
    /* ARRANGE: Change new RFID hash in an otherwise valid TRANSFER. */
    /* ACTION: Encode StationEvent. */
    /* ASSERT: Encoding fails before signing. */
    /* FAILURE MEANS: firmware can sign a TRANSFER that changes physical identity. */
    lastro_station_event_fields_t fields = fixture_transfer_fields();
    uint8_t encoded[LASTRO_STATION_EVENT_LEN];
    fields.new_rfid_hash[0] ^= 1u;
    TEST_ASSERT_FALSE(lastro_event_encode(&fields, encoded));
}

TEST_CASE("event_rejects_invalid_reidentify_semantics", "[lastro][contract]")
{
    /* PURPOSE: REIDENTIFY must change RFID while preserving custody. */
    /* ARRANGE: Change to_custodian in an otherwise valid REIDENTIFY. */
    /* ACTION: Encode StationEvent. */
    /* ASSERT: Encoding fails before signing. */
    /* FAILURE MEANS: firmware can sign REIDENTIFY as an implicit custody transfer. */
    lastro_station_event_fields_t fields = fixture_reidentify_fields();
    uint8_t encoded[LASTRO_STATION_EVENT_LEN];
    fields.to_custodian[0] ^= 1u;
    TEST_ASSERT_FALSE(lastro_event_encode(&fields, encoded));
}

TEST_CASE("event_sha256_matches_rust_fixture", "[lastro][contract]")
{
    /* PURPOSE: Keep event hashing interoperable across C, Rust, and the verifier. */
    /* ARRANGE: Decode the exact committed ORIGIN StationEvent fixture. */
    /* ACTION: SHA-256 the exact 276 bytes. */
    /* ASSERT: Hash equals the committed shared event hash. */
    /* FAILURE MEANS: event_hash/predecessor binding can diverge across stack layers. */
    uint8_t event[LASTRO_STATION_EVENT_LEN];
    uint8_t actual[32];
    uint8_t expected[32];
    fixture_decode_hex(FIXTURE_ORIGIN_EVENT_HEX, event, sizeof(event));
    fixture_decode_hex(FIXTURE_ORIGIN_EVENT_HASH_HEX, expected, sizeof(expected));
    TEST_ASSERT_TRUE(lastro_event_sha256(event, actual));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(expected, actual, sizeof(actual));
}
