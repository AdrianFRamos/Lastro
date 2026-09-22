/* Reader-independent Station runtime that binds Agent serial frames to the Station state machine. */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "lastro_station/rfid.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef bool (*lastro_runtime_write_fn)(const uint8_t *bytes, size_t len, void *context);

/** Initialize transport/state and bind the byte writer used for Station -> Agent frames. */
bool lastro_runtime_init(lastro_runtime_write_fn write_bytes, void *write_context);

/** Feed arbitrary bytes received from the Agent. Only COMMAND and ACK are accepted inbound. */
bool lastro_runtime_feed_agent(const uint8_t *bytes, size_t len);

/** Supply one RFID already canonicalized by the selected physical reader adapter. */
bool lastro_runtime_observe_rfid(const lastro_canonical_rfid_t *rfid);

/** Advance Station work and retry any Station -> Agent frame whose previous write failed. */
void lastro_runtime_poll(void);

/** True while an encoded Station -> Agent frame is retained for write retry. */
bool lastro_runtime_has_pending_output(void);

#ifdef __cplusplus
}
#endif
