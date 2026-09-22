/* Binary serial framing and fixed-payload contract between ESP32-C5 and the Rust Agent. */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define LASTRO_SERIAL_MAGIC "LSTR"
#define LASTRO_SERIAL_VERSION 1u
#define LASTRO_SERIAL_MAX_PAYLOAD 1024u
#define LASTRO_COMMAND_PAYLOAD_LEN 224u
#define LASTRO_EVENT_READY_PAYLOAD_LEN 397u
#define LASTRO_ACK_PAYLOAD_LEN 48u
#define LASTRO_ERROR_PAYLOAD_LEN 20u
#define LASTRO_SERIAL_HEADER_LEN 12u
#define LASTRO_SERIAL_CRC_LEN 4u

typedef enum {
    LASTRO_MSG_COMMAND = 1,
    LASTRO_MSG_EVENT_READY = 2,
    LASTRO_MSG_ACK = 3,
    LASTRO_MSG_ERROR = 4,
} lastro_message_type_t;

typedef enum {
    LASTRO_STATION_ERROR_INVALID_COMMAND = 1,
    LASTRO_STATION_ERROR_RFID_READ_FAILED = 2,
    LASTRO_STATION_ERROR_INVALID_EVENT_CONTEXT = 3,
    LASTRO_STATION_ERROR_SIGNING_FAILED = 4,
    LASTRO_STATION_ERROR_BUSY = 5,
} lastro_station_error_code_t;

typedef struct {
    lastro_message_type_t type;
    const uint8_t *payload;
    uint32_t payload_len;
} lastro_frame_view_t;

typedef struct {
    uint8_t capture_id[16];
    uint8_t action;
    uint8_t deployment_id[32];
    uint8_t animal_id[32];
    uint64_t event_sequence;
    uint32_t identity_revision;
    uint8_t previous_event_hash[32];
    uint8_t expected_old_rfid_hash[32];
    uint8_t from_custodian[32];
    uint8_t to_custodian[32];
} lastro_command_payload_t;

typedef struct {
    uint8_t capture_id[16];
    uint8_t event_bytes[276];
    uint8_t observed_rfid[8];
    uint8_t station_pubkey33[33];
    uint8_t station_signature64[64];
} lastro_event_ready_payload_t;

typedef struct {
    uint8_t capture_id[16];
    uint8_t event_hash[32];
} lastro_ack_payload_t;

typedef struct {
    uint8_t capture_id[16];
    lastro_station_error_code_t code;
} lastro_error_payload_t;

uint32_t lastro_crc32c(const uint8_t *bytes, size_t len);

/** Frame = magic | version | type | reserved | payload_len | payload | CRC32C. */
bool lastro_transport_encode(const lastro_frame_view_t *frame, uint8_t *out, size_t out_len, size_t *written);

/** Reset incremental decoder state. Call during runtime initialization or serial reconnect. */
void lastro_transport_reset(void);

/** Incrementally feed arbitrary serial chunks; corrupted frames are never exposed. */
bool lastro_transport_feed(const uint8_t *bytes, size_t len);

/** Return the next decoded frame once. The payload pointer remains valid until the next feed/take. */
bool lastro_transport_take(lastro_frame_view_t *out);

bool lastro_command_decode(const uint8_t payload[LASTRO_COMMAND_PAYLOAD_LEN], lastro_command_payload_t *out);
bool lastro_event_ready_encode(const lastro_event_ready_payload_t *value, uint8_t out[LASTRO_EVENT_READY_PAYLOAD_LEN]);
bool lastro_ack_decode(const uint8_t payload[LASTRO_ACK_PAYLOAD_LEN], lastro_ack_payload_t *out);
bool lastro_error_encode(const lastro_error_payload_t *value, uint8_t out[LASTRO_ERROR_PAYLOAD_LEN]);

#ifdef __cplusplus
}
#endif
