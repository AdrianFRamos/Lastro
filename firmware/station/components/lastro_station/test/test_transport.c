#include "unity.h"
#include "lastro_station/transport.h"
#include "test_fixture.h"

#define EVENT_READY_FRAME_MAX (LASTRO_SERIAL_HEADER_LEN + LASTRO_EVENT_READY_PAYLOAD_LEN + LASTRO_SERIAL_CRC_LEN)
#define COMMAND_FRAME_MAX (LASTRO_SERIAL_HEADER_LEN + LASTRO_COMMAND_PAYLOAD_LEN + LASTRO_SERIAL_CRC_LEN)

static void take_and_assert_frame(lastro_message_type_t type, const uint8_t *payload, size_t payload_len)
{
    lastro_frame_view_t decoded;
    TEST_ASSERT_TRUE(lastro_transport_take(&decoded));
    TEST_ASSERT_EQUAL_INT(type, decoded.type);
    TEST_ASSERT_EQUAL_UINT32(payload_len, decoded.payload_len);
    TEST_ASSERT_EQUAL_UINT8_ARRAY(payload, decoded.payload, payload_len);
    TEST_ASSERT_FALSE(lastro_transport_take(&decoded));
}

TEST_CASE("command_payload_fixture_matches_224_byte_wire_contract", "[lastro][transport][payload]")
{
    /* PURPOSE: Freeze COMMAND offsets and endianness against the shared Rust fixture. */
    /* ARRANGE: Decode the committed 224-byte serial-command fixture. */
    /* ACTION: lastro_command_decode(). */
    /* ASSERT: Every fixed field matches the exact protocol vector. */
    /* FAILURE MEANS: ESP32-C5 interprets Agent COMMAND bytes differently from Rust. */
    uint8_t payload[LASTRO_COMMAND_PAYLOAD_LEN];
    uint8_t capture[16];
    uint8_t origin_rfid[32];
    lastro_command_payload_t command;
    fixture_decode_hex(FIXTURE_COMMAND_HEX, payload, sizeof(payload));
    fixture_decode_hex(FIXTURE_CAPTURE_ID_HEX, capture, sizeof(capture));
    fixture_decode_hex(FIXTURE_RFID_A_HASH_HEX, origin_rfid, sizeof(origin_rfid));

    TEST_ASSERT_TRUE(lastro_command_decode(payload, &command));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(capture, command.capture_id, sizeof(capture));
    TEST_ASSERT_EQUAL_UINT8(LASTRO_ACTION_ORIGIN, command.action);
    for (size_t i = 0; i < 32u; ++i) {
        TEST_ASSERT_EQUAL_UINT8(0xd0u, command.deployment_id[i]);
        TEST_ASSERT_EQUAL_UINT8(0x11u, command.animal_id[i]);
        TEST_ASSERT_EQUAL_UINT8(0u, command.previous_event_hash[i]);
        TEST_ASSERT_EQUAL_UINT8(0u, command.expected_old_rfid_hash[i]);
        TEST_ASSERT_EQUAL_UINT8(0u, command.from_custodian[i]);
        TEST_ASSERT_EQUAL_UINT8(0xa1u, command.to_custodian[i]);
    }
    TEST_ASSERT_EQUAL_UINT64(1u, command.event_sequence);
    TEST_ASSERT_EQUAL_UINT32(1u, command.identity_revision);
    (void)origin_rfid;
}

TEST_CASE("event_ready_payload_fixture_matches_397_byte_wire_contract", "[lastro][transport][payload]")
{
    /* PURPOSE: Prove EVENT_READY transports Station cryptographic evidence without modification. */
    /* ARRANGE: Construct origin evidence from committed capture/event/RFID/key/signature vectors. */
    /* ACTION: Encode lastro_event_ready_payload_t. */
    /* ASSERT: Output equals serial-event-ready byte-for-byte and fixed offsets stay exact. */
    /* FAILURE MEANS: physical evidence changes before the Agent receives it. */
    lastro_event_ready_payload_t value;
    uint8_t actual[LASTRO_EVENT_READY_PAYLOAD_LEN];
    uint8_t expected[LASTRO_EVENT_READY_PAYLOAD_LEN];
    fixture_decode_hex(FIXTURE_CAPTURE_ID_HEX, value.capture_id, sizeof(value.capture_id));
    fixture_decode_hex(FIXTURE_ORIGIN_EVENT_HEX, value.event_bytes, sizeof(value.event_bytes));
    fixture_decode_hex(FIXTURE_RFID_A_HEX, value.observed_rfid, sizeof(value.observed_rfid));
    fixture_decode_hex(FIXTURE_STATION_PUBKEY_HEX, value.station_pubkey33, sizeof(value.station_pubkey33));
    fixture_decode_hex(FIXTURE_ORIGIN_SIGNATURE_HEX, value.station_signature64, sizeof(value.station_signature64));
    fixture_decode_hex(FIXTURE_EVENT_READY_HEX, expected, sizeof(expected));

    TEST_ASSERT_TRUE(lastro_event_ready_encode(&value, actual));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(expected, actual, sizeof(actual));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(value.event_bytes, actual + 16u, sizeof(value.event_bytes));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(value.station_signature64, actual + 333u, sizeof(value.station_signature64));
}

TEST_CASE("ack_and_error_payload_offsets_are_exact", "[lastro][transport][payload]")
{
    /* PURPOSE: Freeze ACK/ERROR layouts so lifecycle messages cannot bind to the wrong capture. */
    /* ARRANGE: Use committed ACK and RFID_READ_FAILED error fixtures. */
    /* ACTION: Decode ACK and encode ERROR. */
    /* ASSERT: ACK is capture+hash; ERROR is capture+u16 little-endian code+zero reserved. */
    /* FAILURE MEANS: lifecycle acknowledgements/errors can be associated with wrong evidence. */
    uint8_t ack_bytes[LASTRO_ACK_PAYLOAD_LEN];
    uint8_t expected_capture[16];
    uint8_t expected_hash[32];
    uint8_t expected_error[LASTRO_ERROR_PAYLOAD_LEN];
    uint8_t actual_error[LASTRO_ERROR_PAYLOAD_LEN];
    lastro_ack_payload_t ack;
    lastro_error_payload_t error = {0};
    fixture_decode_hex(FIXTURE_ACK_HEX, ack_bytes, sizeof(ack_bytes));
    fixture_decode_hex(FIXTURE_CAPTURE_ID_HEX, expected_capture, sizeof(expected_capture));
    fixture_decode_hex(FIXTURE_ORIGIN_EVENT_HASH_HEX, expected_hash, sizeof(expected_hash));
    fixture_decode_hex(FIXTURE_ERROR_HEX, expected_error, sizeof(expected_error));

    TEST_ASSERT_TRUE(lastro_ack_decode(ack_bytes, &ack));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(expected_capture, ack.capture_id, sizeof(expected_capture));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(expected_hash, ack.event_hash, sizeof(expected_hash));

    memcpy(error.capture_id, expected_capture, sizeof(error.capture_id));
    error.code = LASTRO_STATION_ERROR_RFID_READ_FAILED;
    TEST_ASSERT_TRUE(lastro_error_encode(&error, actual_error));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(expected_error, actual_error, sizeof(actual_error));
}

TEST_CASE("crc32c_castagnoli_known_vector_is_e3069283", "[lastro][transport][crc]")
{
    /* PURPOSE: Prove firmware uses CRC32C/Castagnoli compatible with the Rust Agent. */
    /* ARRANGE: ASCII bytes \"123456789\". */
    /* ACTION: Run the frame CRC helper. */
    /* ASSERT: Numeric CRC32C equals 0xe3069283. */
    /* FAILURE MEANS: firmware and Agent use incompatible frame integrity algorithms. */
    static const uint8_t input[] = "123456789";
    TEST_ASSERT_EQUAL_HEX32(UINT32_C(0xe3069283), lastro_crc32c(input, sizeof(input) - 1u));
}

TEST_CASE("transport_roundtrip_command_and_event_ready", "[lastro][transport][frame]")
{
    /* PURPOSE: Prove outer framing symmetry for both critical payloads. */
    /* ARRANGE: Decode committed COMMAND and EVENT_READY payload fixtures. */
    /* ACTION: Frame-encode each payload, feed it, and take the decoded frame. */
    /* ASSERT: Type and payload remain byte-identical with valid header/CRC. */
    /* FAILURE MEANS: outer serial framing is not symmetric even with correct payload bytes. */
    uint8_t command[LASTRO_COMMAND_PAYLOAD_LEN];
    uint8_t event_ready[LASTRO_EVENT_READY_PAYLOAD_LEN];
    uint8_t command_frame[COMMAND_FRAME_MAX];
    uint8_t event_frame[EVENT_READY_FRAME_MAX];
    size_t command_len = 0u;
    size_t event_len = 0u;
    fixture_decode_hex(FIXTURE_COMMAND_HEX, command, sizeof(command));
    fixture_decode_hex(FIXTURE_EVENT_READY_HEX, event_ready, sizeof(event_ready));
    lastro_frame_view_t command_view = {LASTRO_MSG_COMMAND, command, sizeof(command)};
    lastro_frame_view_t event_view = {LASTRO_MSG_EVENT_READY, event_ready, sizeof(event_ready)};

    TEST_ASSERT_TRUE(lastro_transport_encode(&command_view, command_frame, sizeof(command_frame), &command_len));
    TEST_ASSERT_TRUE(lastro_transport_feed(command_frame, command_len));
    take_and_assert_frame(LASTRO_MSG_COMMAND, command, sizeof(command));

    TEST_ASSERT_TRUE(lastro_transport_encode(&event_view, event_frame, sizeof(event_frame), &event_len));
    TEST_ASSERT_TRUE(lastro_transport_feed(event_frame, event_len));
    take_and_assert_frame(LASTRO_MSG_EVENT_READY, event_ready, sizeof(event_ready));
}

TEST_CASE("transport_handles_every_single_byte_chunking", "[lastro][transport][frame]")
{
    /* PURPOSE: Make protocol behavior independent of UART read chunk size. */
    /* ARRANGE: Encode one valid EVENT_READY frame. */
    /* ACTION: Feed exactly one byte per call through the complete frame. */
    /* ASSERT: No frame is exposed early and exactly one complete frame is available at the end. */
    /* FAILURE MEANS: UART chunk size changes transport behavior. */
    uint8_t payload[LASTRO_EVENT_READY_PAYLOAD_LEN];
    uint8_t frame[EVENT_READY_FRAME_MAX];
    size_t frame_len = 0u;
    lastro_frame_view_t frame_view;
    fixture_decode_hex(FIXTURE_EVENT_READY_HEX, payload, sizeof(payload));
    lastro_frame_view_t input = {LASTRO_MSG_EVENT_READY, payload, sizeof(payload)};
    TEST_ASSERT_TRUE(lastro_transport_encode(&input, frame, sizeof(frame), &frame_len));

    for (size_t i = 0; i < frame_len; ++i) {
        TEST_ASSERT_TRUE(lastro_transport_feed(frame + i, 1u));
        if (i + 1u < frame_len) TEST_ASSERT_FALSE(lastro_transport_take(&frame_view));
    }
    take_and_assert_frame(LASTRO_MSG_EVENT_READY, payload, sizeof(payload));
}

TEST_CASE("transport_bad_crc_is_rejected_and_next_good_frame_recovers", "[lastro][transport][frame]")
{
    /* PURPOSE: Reject corruption and resynchronize without exposing the bad frame. */
    /* ARRANGE: Corrupt one payload bit without updating CRC, then append a valid frame. */
    /* ACTION: Feed the corrupted frame followed by the valid frame. */
    /* ASSERT: Bad frame is never exposed and the next valid frame is emitted exactly once. */
    /* FAILURE MEANS: serial corruption can escape validation or permanently desynchronize transport. */
    uint8_t payload[LASTRO_COMMAND_PAYLOAD_LEN];
    uint8_t bad[COMMAND_FRAME_MAX];
    uint8_t good[COMMAND_FRAME_MAX];
    size_t bad_len = 0u;
    size_t good_len = 0u;
    lastro_frame_view_t frame_view;
    fixture_decode_hex(FIXTURE_COMMAND_HEX, payload, sizeof(payload));
    lastro_frame_view_t input = {LASTRO_MSG_COMMAND, payload, sizeof(payload)};
    TEST_ASSERT_TRUE(lastro_transport_encode(&input, bad, sizeof(bad), &bad_len));
    TEST_ASSERT_TRUE(lastro_transport_encode(&input, good, sizeof(good), &good_len));
    bad[LASTRO_SERIAL_HEADER_LEN + 3u] ^= 1u;

    TEST_ASSERT_TRUE(lastro_transport_feed(bad, bad_len));
    TEST_ASSERT_FALSE(lastro_transport_take(&frame_view));
    TEST_ASSERT_TRUE(lastro_transport_feed(good, good_len));
    take_and_assert_frame(LASTRO_MSG_COMMAND, payload, sizeof(payload));
}

TEST_CASE("transport_oversized_payload_is_rejected_before_copy", "[lastro][transport][safety]")
{
    /* PURPOSE: Reject declared payload sizes above the fixed transport bound before payload copy. */
    /* ARRANGE: Build a syntactically recognizable header declaring LASTRO_SERIAL_MAX_PAYLOAD+1, then a valid COMMAND frame. */
    /* ACTION: Feed the oversized header and then the valid frame. */
    /* ASSERT: No oversized frame is exposed and decoder recovers for the following valid frame. */
    /* FAILURE MEANS: serial input can drive unsafe copy/allocation behavior or permanent desynchronization. */
    uint8_t oversized[LASTRO_SERIAL_HEADER_LEN] = {'L', 'S', 'T', 'R', LASTRO_SERIAL_VERSION, LASTRO_MSG_COMMAND, 0, 0};
    const uint32_t too_large = LASTRO_SERIAL_MAX_PAYLOAD + 1u;
    oversized[8] = (uint8_t)too_large;
    oversized[9] = (uint8_t)(too_large >> 8u);
    oversized[10] = (uint8_t)(too_large >> 16u);
    oversized[11] = (uint8_t)(too_large >> 24u);
    lastro_frame_view_t decoded;
    TEST_ASSERT_TRUE(lastro_transport_feed(oversized, sizeof(oversized)));
    TEST_ASSERT_FALSE(lastro_transport_take(&decoded));

    uint8_t payload[LASTRO_COMMAND_PAYLOAD_LEN];
    uint8_t frame[COMMAND_FRAME_MAX];
    size_t frame_len = 0u;
    fixture_decode_hex(FIXTURE_COMMAND_HEX, payload, sizeof(payload));
    lastro_frame_view_t input = {LASTRO_MSG_COMMAND, payload, sizeof(payload)};
    TEST_ASSERT_TRUE(lastro_transport_encode(&input, frame, sizeof(frame), &frame_len));
    TEST_ASSERT_TRUE(lastro_transport_feed(frame, frame_len));
    take_and_assert_frame(LASTRO_MSG_COMMAND, payload, sizeof(payload));
}

TEST_CASE("transport exposes back-to-back frames from one UART chunk in order", "[lastro][transport][frame]")
{
    /* PURPOSE: Preserve every complete frame when one UART read contains multiple Agent messages. */
    /* ARRANGE: Encode two valid COMMAND frames and concatenate them into one input chunk. */
    /* ACTION: Feed the combined chunk once and take frames twice without another feed call. */
    /* ASSERT: Both frames are returned exactly once, in order, with byte-identical payloads. */
    /* FAILURE MEANS: a complete ACK/COMMAND already buffered behind another frame can be stranded until unrelated bytes arrive. */
    uint8_t payload[LASTRO_COMMAND_PAYLOAD_LEN];
    uint8_t first[COMMAND_FRAME_MAX];
    uint8_t second[COMMAND_FRAME_MAX];
    uint8_t combined[COMMAND_FRAME_MAX * 2u];
    size_t first_len = 0u;
    size_t second_len = 0u;
    fixture_decode_hex(FIXTURE_COMMAND_HEX, payload, sizeof(payload));
    lastro_frame_view_t input = {LASTRO_MSG_COMMAND, payload, sizeof(payload)};

    lastro_transport_reset();
    TEST_ASSERT_TRUE(lastro_transport_encode(&input, first, sizeof(first), &first_len));
    TEST_ASSERT_TRUE(lastro_transport_encode(&input, second, sizeof(second), &second_len));
    memcpy(combined, first, first_len);
    memcpy(combined + first_len, second, second_len);

    TEST_ASSERT_TRUE(lastro_transport_feed(combined, first_len + second_len));
    take_and_assert_frame(LASTRO_MSG_COMMAND, payload, sizeof(payload));
    take_and_assert_frame(LASTRO_MSG_COMMAND, payload, sizeof(payload));
}
