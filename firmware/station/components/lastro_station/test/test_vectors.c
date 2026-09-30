#include "unity.h"
#include "lastro_station/event.h"
#include "test_fixture.h"

static void assert_vector(const char *command_hex, const char *rfid_hash_hex, const char *envelope_hex, const char *event_hash_hex)
{
    lastro_command_payload_t command = fixture_command(command_hex);
    uint8_t station_id[32];
    uint8_t rfid_hash[32];
    uint8_t expected[LASTRO_ENVELOPE_LEN];
    uint8_t actual[LASTRO_ENVELOPE_LEN];
    uint8_t expected_hash[32];
    uint8_t actual_hash[32];
    fixture_decode_hex(FIXTURE_STATION_ID_HEX, station_id, sizeof(station_id));
    fixture_decode_hex(rfid_hash_hex, rfid_hash, sizeof(rfid_hash));
    fixture_decode_hex(envelope_hex, expected, sizeof(expected));
    fixture_decode_hex(event_hash_hex, expected_hash, sizeof(expected_hash));
    TEST_ASSERT_TRUE(lastro_capture_envelope_encode(&command, station_id, rfid_hash, actual));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(expected, actual, sizeof(actual));
    TEST_ASSERT_TRUE(lastro_envelope_event_hash(actual, actual_hash));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(expected_hash, actual_hash, sizeof(actual_hash));
}

TEST_CASE("c_bind_envelope_equals_repository_fixture", "[lastro][contract]")
{
    /* PURPOSE: Preserve cross-language IDENTIFIER_BOUND interoperability. */
    /* ARRANGE: Decode the committed bind COMMAND and tag A hash. */
    /* ACTION: Encode the capture envelope and its event hash. */
    /* ASSERT: Bytes and hash equal the Rust vectors exactly. */
    /* FAILURE MEANS: C signs envelopes the chain and API would reject. */
    assert_vector(FIXTURE_BIND_COMMAND_HEX, FIXTURE_RFID_A_HASH_HEX, FIXTURE_BIND_ENVELOPE_HEX, FIXTURE_BIND_EVENT_HASH_HEX);
}

TEST_CASE("c_replace_envelope_equals_repository_fixture", "[lastro][contract]")
{
    /* PURPOSE: Preserve cross-language IDENTIFIER_REPLACED interoperability. */
    /* ARRANGE: Decode the committed replace COMMAND and tag B hash. */
    /* ACTION: Encode the capture envelope and its event hash. */
    /* ASSERT: Bytes and hash equal the Rust vectors exactly. */
    /* FAILURE MEANS: reidentification evidence from C would not verify. */
    assert_vector(FIXTURE_REPLACE_COMMAND_HEX, FIXTURE_RFID_B_HASH_HEX, FIXTURE_REPLACE_ENVELOPE_HEX, FIXTURE_REPLACE_EVENT_HASH_HEX);
}

TEST_CASE("c_observe_envelope_equals_repository_fixture", "[lastro][contract]")
{
    /* PURPOSE: Preserve cross-language OBSERVATION_RECORDED interoperability. */
    /* ARRANGE: Decode the committed observe COMMAND and tag B hash. */
    /* ACTION: Encode the capture envelope and its event hash. */
    /* ASSERT: Bytes and hash equal the Rust vectors exactly. */
    /* FAILURE MEANS: presence evidence from C would not verify. */
    assert_vector(FIXTURE_OBSERVE_COMMAND_HEX, FIXTURE_RFID_B_HASH_HEX, FIXTURE_OBSERVE_ENVELOPE_HEX, FIXTURE_OBSERVE_EVENT_HASH_HEX);
}
