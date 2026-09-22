/*
 * Reader-independent Station orchestration state machine.
 *
 * COMMAND is the only trigger for a capture. A valid command moves the Station to WAIT_RFID.
 * A reader adapter may then provide exactly one canonical RFID observed from physical hardware.
 * The Station constructs/signs one StationEvent and remains bound to that capture/event until a
 * matching ACK arrives. No API accepts a future new_rfid_hash from the backend.
 */
#pragma once

#include <stdbool.h>
#include <stdint.h>

#include "lastro_station/rfid.h"
#include "lastro_station/transport.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef enum {
    LASTRO_STATION_IDLE = 0,
    LASTRO_STATION_WAIT_RFID,
    LASTRO_STATION_BUILD_EVENT,
    LASTRO_STATION_SIGN,
    LASTRO_STATION_WAIT_ACK,
} lastro_station_state_t;

/** Initialize signer identity and leave the Station idle without starting a capture. */
bool lastro_station_init(void);

/**
 * Accept one decoded Agent command. While busy, the command is rejected with BUSY and the
 * current capture remains unchanged. Invalid idle commands are rejected with INVALID_COMMAND.
 */
bool lastro_station_submit_command(
    const lastro_command_payload_t *command,
    lastro_error_payload_t *out_error);

/**
 * Supply one canonical RFID that the configured physical reader adapter has actually observed.
 * The Station accepts it only while WAIT_RFID. This API is not a raw-reader parser.
 */
bool lastro_station_observe_rfid(const lastro_canonical_rfid_t *rfid);

/** Advance non-blocking BUILD_EVENT/SIGN work. Call repeatedly from the firmware loop. */
void lastro_station_poll(void);

/** Retrieve the newly signed evidence once. The signed payload remains bound internally to ACK. */
bool lastro_station_take_event_ready(lastro_event_ready_payload_t *out);

/** Retrieve one terminal Station error generated while processing the current capture. */
bool lastro_station_take_error(lastro_error_payload_t *out);

/** Accept only an ACK whose capture_id and event_hash match the signed event in WAIT_ACK. */
bool lastro_station_submit_ack(const lastro_ack_payload_t *ack);

lastro_station_state_t lastro_station_state(void);

#ifdef __cplusplus
}
#endif
