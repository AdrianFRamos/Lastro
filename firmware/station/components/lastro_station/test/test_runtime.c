#include "unity.h"
#include "lastro_station/event.h"
#include "lastro_station/rfid.h"
#include "lastro_station/runtime.h"
#include "lastro_station/station.h"
#include "lastro_station/transport.h"
#include "test_fixture.h"

#include <string.h>

#define MAX_FRAME_LEN (LASTRO_SERIAL_HEADER_LEN + LASTRO_EVENT_READY_PAYLOAD_LEN + LASTRO_SERIAL_CRC_LEN)

static uint8_t written_frame[MAX_FRAME_LEN];
static size_t written_len;
static bool writer_should_fail;

static bool capture_writer(const uint8_t *bytes, size_t len, void *context)
{
    (void)context;
    if (writer_should_fail) return false;
    TEST_ASSERT_LESS_OR_EQUAL_UINT32(sizeof(written_frame), len);
    memcpy(written_frame, bytes, len);
    written_len = len;
    return true;
}

static void clear_writer(void)
{
    memset(written_frame, 0, sizeof(written_frame));
    written_len = 0u;
    writer_should_fail = false;
}

static size_t frame_from_fixture(
    lastro_message_type_t type,
    const char *fixture_hex,
    uint8_t *out,
    size_t out_len,
    size_t payload_len)
{
    uint8_t payload[LASTRO_EVENT_READY_PAYLOAD_LEN];
    size_t written = 0u;
    TEST_ASSERT_LESS_OR_EQUAL_UINT32(sizeof(payload), payload_len);
    fixture_decode_hex(fixture_hex, payload, payload_len);
    lastro_frame_view_t frame = {type, payload, (uint32_t)payload_len};
    TEST_ASSERT_TRUE(lastro_transport_encode(&frame, out, out_len, &written));
    return written;
}

TEST_CASE("runtime consumes Agent COMMAND and emits signed EVENT_READY after physical observation", "[lastro][runtime][contract]")
{
#if !CONFIG_LASTRO_STATION_USE_TEST_VECTOR_KEY
    TEST_IGNORE_MESSAGE("Enable LASTRO_STATION_USE_TEST_VECTOR_KEY for deterministic runtime signing.");
#else
    /* PURPOSE: Complete the reader-independent Station serial path from Agent command to signed evidence. */
    /* ARRANGE: Initialize the runtime with the frozen test key and feed the committed ORIGIN COMMAND frame. */
    /* ACTION: Supply the physically observed canonical RFID and poll the Station runtime. */
    /* ASSERT: The runtime emits exactly one EVENT_READY frame containing the command capture and observed RFID. */
    /* FAILURE MEANS: app_main would need to reimplement protocol orchestration or could bypass the Station state machine. */
    clear_writer();
    TEST_ASSERT_TRUE(lastro_runtime_init(capture_writer, NULL));

    uint8_t command_frame[MAX_FRAME_LEN];
    const size_t command_len = frame_from_fixture(
        LASTRO_MSG_COMMAND,
        FIXTURE_COMMAND_HEX,
        command_frame,
        sizeof(command_frame),
        LASTRO_COMMAND_PAYLOAD_LEN);
    TEST_ASSERT_TRUE(lastro_runtime_feed_agent(command_frame, command_len));
    TEST_ASSERT_EQUAL(LASTRO_STATION_WAIT_RFID, lastro_station_state());

    lastro_canonical_rfid_t observed;
    fixture_decode_hex(FIXTURE_RFID_A_HEX, observed.bytes, sizeof(observed.bytes));
    TEST_ASSERT_TRUE(lastro_runtime_observe_rfid(&observed));
    lastro_runtime_poll();
    TEST_ASSERT_GREATER_THAN_UINT32(0u, written_len);

    lastro_transport_reset();
    TEST_ASSERT_TRUE(lastro_transport_feed(written_frame, written_len));
    lastro_frame_view_t emitted;
    TEST_ASSERT_TRUE(lastro_transport_take(&emitted));
    TEST_ASSERT_EQUAL(LASTRO_MSG_EVENT_READY, emitted.type);
    TEST_ASSERT_EQUAL_UINT32(LASTRO_EVENT_READY_PAYLOAD_LEN, emitted.payload_len);
    TEST_ASSERT_EQUAL_UINT8_ARRAY(observed.bytes, emitted.payload + 292u, sizeof(observed.bytes));
#endif
}

TEST_CASE("runtime retries an EVENT_READY frame after a transient Station-to-Agent write failure", "[lastro][runtime][contract]")
{
#if !CONFIG_LASTRO_STATION_USE_TEST_VECTOR_KEY
    TEST_IGNORE_MESSAGE("Enable LASTRO_STATION_USE_TEST_VECTOR_KEY for deterministic runtime signing.");
#else
    /* PURPOSE: A transient UART write error must not discard the only signed evidence held by the Station. */
    /* ARRANGE: Accept ORIGIN and one physical RFID, then force the first writer call to fail. */
    /* ACTION: Poll once with failure and again after restoring the writer. */
    /* ASSERT: The runtime retains pending output after failure and emits it on the next poll without rebuilding/resigning. */
    /* FAILURE MEANS: serial write failure could strand WAIT_ACK with evidence that can never reach the Agent. */
    clear_writer();
    TEST_ASSERT_TRUE(lastro_runtime_init(capture_writer, NULL));
    uint8_t command_frame[MAX_FRAME_LEN];
    const size_t command_len = frame_from_fixture(
        LASTRO_MSG_COMMAND,
        FIXTURE_COMMAND_HEX,
        command_frame,
        sizeof(command_frame),
        LASTRO_COMMAND_PAYLOAD_LEN);
    TEST_ASSERT_TRUE(lastro_runtime_feed_agent(command_frame, command_len));
    lastro_canonical_rfid_t observed;
    fixture_decode_hex(FIXTURE_RFID_A_HEX, observed.bytes, sizeof(observed.bytes));
    TEST_ASSERT_TRUE(lastro_runtime_observe_rfid(&observed));

    writer_should_fail = true;
    lastro_runtime_poll();
    TEST_ASSERT_TRUE(lastro_runtime_has_pending_output());
    TEST_ASSERT_EQUAL_UINT32(0u, written_len);
    TEST_ASSERT_EQUAL(LASTRO_STATION_WAIT_ACK, lastro_station_state());

    writer_should_fail = false;
    lastro_runtime_poll();
    TEST_ASSERT_FALSE(lastro_runtime_has_pending_output());
    TEST_ASSERT_GREATER_THAN_UINT32(0u, written_len);
    TEST_ASSERT_EQUAL(LASTRO_STATION_WAIT_ACK, lastro_station_state());
#endif
}

TEST_CASE("runtime rejects inbound Station-only message types", "[lastro][runtime][contract]")
{
    /* PURPOSE: Keep serial directionality explicit so the Agent cannot inject Station evidence or errors. */
    /* ARRANGE: Initialize the runtime and construct a validly framed ERROR payload as inbound Agent bytes. */
    /* ACTION: Feed the frame through the production runtime decoder. */
    /* ASSERT: The runtime rejects it and Station state remains IDLE. */
    /* FAILURE MEANS: Agent-controlled bytes could enter a message direction reserved for Station output. */
    clear_writer();
    TEST_ASSERT_TRUE(lastro_runtime_init(capture_writer, NULL));
    uint8_t frame[MAX_FRAME_LEN];
    const size_t frame_len = frame_from_fixture(
        LASTRO_MSG_ERROR,
        FIXTURE_ERROR_HEX,
        frame,
        sizeof(frame),
        LASTRO_ERROR_PAYLOAD_LEN);
    TEST_ASSERT_FALSE(lastro_runtime_feed_agent(frame, frame_len));
    TEST_ASSERT_EQUAL(LASTRO_STATION_IDLE, lastro_station_state());
}
