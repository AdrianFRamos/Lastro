#include "unity.h"
#include "lastro_station/event.h"
#include "test_fixture.h"

TEST_CASE("capture rules reject tags that violate the command", "[lastro][contract]")
{
    /* PURPOSE: The Station must never sign a replacement with the same tag or a presence proof with another tag. */
    /* ARRANGE: Committed replace/observe commands whose active tag is A then B. */
    /* ACTION: Encode envelopes with the rule-violating observed tag. */
    /* ASSERT: Encoding fails. */
    /* FAILURE MEANS: the device could attest a reidentification or presence that never happened. */
    lastro_command_payload_t replace = fixture_command(FIXTURE_REPLACE_COMMAND_HEX);
    lastro_command_payload_t observe = fixture_command(FIXTURE_OBSERVE_COMMAND_HEX);
    uint8_t station_id[32];
    uint8_t tag_a[32];
    uint8_t out[LASTRO_ENVELOPE_LEN];
    fixture_decode_hex(FIXTURE_STATION_ID_HEX, station_id, sizeof(station_id));
    fixture_decode_hex(FIXTURE_RFID_A_HASH_HEX, tag_a, sizeof(tag_a));
    TEST_ASSERT_FALSE(lastro_capture_envelope_encode(&replace, station_id, tag_a, out));
    TEST_ASSERT_FALSE(lastro_capture_envelope_encode(&observe, station_id, tag_a, out));
}

TEST_CASE("capture command semantics are validated before any RFID read", "[lastro][contract]")
{
    /* PURPOSE: Inconsistent context must be refused before physical evidence is produced. */
    /* ARRANGE: Mutate the committed bind command (event type, expected tag, version, event id). */
    /* ACTION: lastro_capture_command_valid(). */
    /* ASSERT: The pristine command is valid and every mutation is rejected. */
    /* FAILURE MEANS: the Station could sign against nonexistent or forked state. */
    lastro_command_payload_t base = fixture_command(FIXTURE_BIND_COMMAND_HEX);
    TEST_ASSERT_TRUE(lastro_capture_command_valid(&base));

    lastro_command_payload_t mutated = base;
    mutated.event_type = 4u;
    TEST_ASSERT_FALSE(lastro_capture_command_valid(&mutated));
    mutated = base;
    mutated.expected_rfid_hash[0] = 1u;
    TEST_ASSERT_FALSE(lastro_capture_command_valid(&mutated));
    mutated = base;
    mutated.state_version = 0u;
    TEST_ASSERT_FALSE(lastro_capture_command_valid(&mutated));
    mutated = base;
    mutated.event_id[0] ^= 1u;
    TEST_ASSERT_FALSE(lastro_capture_command_valid(&mutated));
}

TEST_CASE("capture envelope rejects an invalid time window", "[lastro][contract]")
{
    /* PURPOSE: The signed validity window must match the protocol bound. */
    /* ARRANGE: Committed bind command with an inverted and an over-long window. */
    /* ACTION: Encode the envelope. */
    /* ASSERT: Both windows are rejected. */
    /* FAILURE MEANS: evidence could be signed with a window the chain rejects. */
    lastro_command_payload_t command = fixture_command(FIXTURE_BIND_COMMAND_HEX);
    uint8_t station_id[32];
    uint8_t tag_a[32];
    uint8_t out[LASTRO_ENVELOPE_LEN];
    fixture_decode_hex(FIXTURE_STATION_ID_HEX, station_id, sizeof(station_id));
    fixture_decode_hex(FIXTURE_RFID_A_HASH_HEX, tag_a, sizeof(tag_a));
    command.expires_at = command.observed_at - 1;
    TEST_ASSERT_FALSE(lastro_capture_envelope_encode(&command, station_id, tag_a, out));
    command.expires_at = command.observed_at + LASTRO_MAX_EVENT_AGE_SECONDS + 1;
    TEST_ASSERT_FALSE(lastro_capture_envelope_encode(&command, station_id, tag_a, out));
}
