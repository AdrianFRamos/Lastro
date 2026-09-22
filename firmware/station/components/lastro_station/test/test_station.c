#include "unity.h"
#include "lastro_station/event.h"
#include "lastro_station/rfid.h"
#include "lastro_station/station.h"
#include "lastro_station/transport.h"

#include <string.h>

static lastro_command_payload_t origin_command(void)
{
    lastro_command_payload_t command = {0};
    memset(command.capture_id, 0x41, sizeof(command.capture_id));
    command.action = LASTRO_ACTION_ORIGIN;
    memset(command.deployment_id, 0xd0, sizeof(command.deployment_id));
    memset(command.animal_id, 0x11, sizeof(command.animal_id));
    command.event_sequence = 1;
    command.identity_revision = 1;
    memset(command.to_custodian, 0xa1, sizeof(command.to_custodian));
    return command;
}

static lastro_canonical_rfid_t rfid_a(void)
{
    lastro_canonical_rfid_t value = {{0x80, 0x00, 0x13, 0x00, 0x00, 0x00, 0x00, 0x01}};
    return value;
}

TEST_CASE("station boots idle", "[lastro][contract]")
{
    /* PURPOSE: initialization must never start a capture spontaneously.
     * ASSERT: successful initialization leaves state exactly IDLE.
     * FAILURE: boot could consume nonexistent context or create unsolicited evidence. */
    TEST_ASSERT_TRUE(lastro_station_init());
    TEST_ASSERT_EQUAL(LASTRO_STATION_IDLE, lastro_station_state());
}

TEST_CASE("station command moves to wait RFID", "[lastro][contract]")
{
    /* PURPOSE: capture starts only from an explicit valid Agent command.
     * ASSERT: one valid ORIGIN command moves IDLE to WAIT_RFID.
     * FAILURE: the Station could ignore or mis-handle explicit capture context. */
    TEST_ASSERT_TRUE(lastro_station_init());
    lastro_command_payload_t command = origin_command();
    lastro_error_payload_t error = {0};
    TEST_ASSERT_TRUE(lastro_station_submit_command(&command, &error));
    TEST_ASSERT_EQUAL(LASTRO_STATION_WAIT_RFID, lastro_station_state());
}

TEST_CASE("station does not sign without valid RFID", "[lastro][contract]")
{
    /* PURPOSE: backend context alone must never create physical evidence.
     * ASSERT: without an observed RFID, polling produces no EVENT_READY and remains WAIT_RFID.
     * FAILURE: the device could sign evidence without a physical observation. */
    TEST_ASSERT_TRUE(lastro_station_init());
    lastro_command_payload_t command = origin_command();
    TEST_ASSERT_TRUE(lastro_station_submit_command(&command, NULL));
    lastro_station_poll();
    lastro_event_ready_payload_t event_ready = {0};
    TEST_ASSERT_FALSE(lastro_station_take_event_ready(&event_ready));
    TEST_ASSERT_EQUAL(LASTRO_STATION_WAIT_RFID, lastro_station_state());
}

TEST_CASE("station rejects second command while busy", "[lastro][contract]")
{
    /* PURPOSE: exactly one capture context may own the next physical RFID observation.
     * ASSERT: command B receives BUSY while command A remains WAIT_RFID.
     * FAILURE: two captures could race for one physical RFID read. */
    TEST_ASSERT_TRUE(lastro_station_init());
    lastro_command_payload_t first = origin_command();
    lastro_command_payload_t second = origin_command();
    memset(second.capture_id, 0x42, sizeof(second.capture_id));
    TEST_ASSERT_TRUE(lastro_station_submit_command(&first, NULL));
    lastro_error_payload_t error = {0};
    TEST_ASSERT_FALSE(lastro_station_submit_command(&second, &error));
    TEST_ASSERT_EQUAL(LASTRO_STATION_ERROR_BUSY, error.code);
    TEST_ASSERT_EQUAL_UINT8_ARRAY(second.capture_id, error.capture_id, sizeof(error.capture_id));
    TEST_ASSERT_EQUAL(LASTRO_STATION_WAIT_RFID, lastro_station_state());
}

TEST_CASE("station new RFID hash comes from observed RFID", "[lastro][contract]")
{
    /* PURPOSE: the backend must not inject the new physical identifier.
     * ASSERT: StationEvent new_rfid_hash equals SHA-256(domain || observed canonical RFID).
     * FAILURE: capture context could determine the new RFID instead of physical observation. */
    TEST_ASSERT_TRUE(lastro_station_init());
    lastro_command_payload_t command = origin_command();
    lastro_canonical_rfid_t observed = rfid_a();
    uint8_t expected_hash[32];
    TEST_ASSERT_TRUE(lastro_rfid_hash(&observed, expected_hash));
    TEST_ASSERT_TRUE(lastro_station_submit_command(&command, NULL));
    TEST_ASSERT_TRUE(lastro_station_observe_rfid(&observed));
    lastro_station_poll();
    lastro_event_ready_payload_t event_ready = {0};
    TEST_ASSERT_TRUE(lastro_station_take_event_ready(&event_ready));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(expected_hash, event_ready.event_bytes + 180, 32);
    TEST_ASSERT_EQUAL_UINT8_ARRAY(observed.bytes, event_ready.observed_rfid, 8);
}

TEST_CASE("station ACK returns to idle only for matching event", "[lastro][contract]")
{
    /* PURPOSE: ACK must terminate only the exact capture/event that was durably accepted by the Agent.
     * ASSERT: wrong hash is rejected; exact capture_id + event_hash returns to IDLE.
     * FAILURE: stale or unrelated ACK could discard unacknowledged evidence. */
    TEST_ASSERT_TRUE(lastro_station_init());
    lastro_command_payload_t command = origin_command();
    lastro_canonical_rfid_t observed = rfid_a();
    TEST_ASSERT_TRUE(lastro_station_submit_command(&command, NULL));
    TEST_ASSERT_TRUE(lastro_station_observe_rfid(&observed));
    lastro_station_poll();
    lastro_event_ready_payload_t event_ready = {0};
    TEST_ASSERT_TRUE(lastro_station_take_event_ready(&event_ready));

    lastro_ack_payload_t ack = {0};
    memcpy(ack.capture_id, command.capture_id, sizeof(ack.capture_id));
    TEST_ASSERT_TRUE(lastro_event_sha256(event_ready.event_bytes, ack.event_hash));
    ack.event_hash[0] ^= 1;
    TEST_ASSERT_FALSE(lastro_station_submit_ack(&ack));
    TEST_ASSERT_EQUAL(LASTRO_STATION_WAIT_ACK, lastro_station_state());
    ack.event_hash[0] ^= 1;
    TEST_ASSERT_TRUE(lastro_station_submit_ack(&ack));
    TEST_ASSERT_EQUAL(LASTRO_STATION_IDLE, lastro_station_state());
}

TEST_CASE("station invalid transfer observation returns to safe state", "[lastro][contract]")
{
    /* PURPOSE: TRANSFER may use only the current RFID physically observed at capture time.
     * ASSERT: a different observed RFID produces INVALID_EVENT_CONTEXT, no EVENT_READY, and IDLE.
     * FAILURE: a transfer could be signed without observing the canonical current RFID. */
    TEST_ASSERT_TRUE(lastro_station_init());
    lastro_command_payload_t command = origin_command();
    command.action = LASTRO_ACTION_TRANSFER;
    command.event_sequence = 2;
    memset(command.previous_event_hash, 0x51, sizeof(command.previous_event_hash));
    memset(command.expected_old_rfid_hash, 0x52, sizeof(command.expected_old_rfid_hash));
    memset(command.from_custodian, 0xa1, sizeof(command.from_custodian));
    memset(command.to_custodian, 0xb2, sizeof(command.to_custodian));
    lastro_canonical_rfid_t observed = rfid_a();
    TEST_ASSERT_TRUE(lastro_station_submit_command(&command, NULL));
    TEST_ASSERT_TRUE(lastro_station_observe_rfid(&observed));
    lastro_station_poll();
    lastro_error_payload_t error = {0};
    TEST_ASSERT_TRUE(lastro_station_take_error(&error));
    TEST_ASSERT_EQUAL(LASTRO_STATION_ERROR_INVALID_EVENT_CONTEXT, error.code);
    TEST_ASSERT_EQUAL(LASTRO_STATION_IDLE, lastro_station_state());
    lastro_event_ready_payload_t event_ready = {0};
    TEST_ASSERT_FALSE(lastro_station_take_event_ready(&event_ready));
}

TEST_CASE("station accepts an identical command replay while waiting for RFID", "[lastro][contract][recovery]")
{
    /* PURPOSE: Recover an Agent restart without replacing the immutable capture context or consuming another physical observation. */
    /* ARRANGE: Submit one valid ORIGIN command and leave the Station in WAIT_RFID. */
    /* ACTION: Submit the exact same command again. */
    /* ASSERT: The replay is accepted idempotently and state remains WAIT_RFID with no BUSY error. */
    /* FAILURE MEANS: API command redelivery after Agent restart can wedge an otherwise recoverable capture. */
    TEST_ASSERT_TRUE(lastro_station_init());
    lastro_command_payload_t command = origin_command();
    TEST_ASSERT_TRUE(lastro_station_submit_command(&command, NULL));
    lastro_error_payload_t error = {0};
    TEST_ASSERT_TRUE(lastro_station_submit_command(&command, &error));
    TEST_ASSERT_EQUAL(LASTRO_STATION_WAIT_RFID, lastro_station_state());
    TEST_ASSERT_EQUAL_UINT16(0u, error.code);
}

TEST_CASE("station replays identical signed evidence when the same command returns before ACK", "[lastro][contract][recovery]")
{
    /* PURPOSE: Close the crash window where EVENT_READY was sent but the Agent died before its durable LOCAL commit/ACK. */
    /* ARRANGE: Complete one ORIGIN through signing, take EVENT_READY once, and remain in WAIT_ACK. */
    /* ACTION: Submit the exact same immutable command again and take EVENT_READY a second time. */
    /* ASSERT: The second payload is byte-identical to the original signed evidence and no new RFID/signing context is introduced. */
    /* FAILURE MEANS: Agent restart before ACK requires manual Station reset or can produce different evidence for one capture. */
    TEST_ASSERT_TRUE(lastro_station_init());
    lastro_command_payload_t command = origin_command();
    lastro_canonical_rfid_t observed = rfid_a();
    TEST_ASSERT_TRUE(lastro_station_submit_command(&command, NULL));
    TEST_ASSERT_TRUE(lastro_station_observe_rfid(&observed));
    lastro_station_poll();
    lastro_event_ready_payload_t first = {0};
    lastro_event_ready_payload_t replay = {0};
    TEST_ASSERT_TRUE(lastro_station_take_event_ready(&first));
    TEST_ASSERT_EQUAL(LASTRO_STATION_WAIT_ACK, lastro_station_state());

    TEST_ASSERT_TRUE(lastro_station_submit_command(&command, NULL));
    TEST_ASSERT_TRUE(lastro_station_take_event_ready(&replay));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(&first, &replay, sizeof(first));
    TEST_ASSERT_EQUAL(LASTRO_STATION_WAIT_ACK, lastro_station_state());
}
