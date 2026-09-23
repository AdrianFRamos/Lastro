#include "lastro_station/transport.h"
#include "lastro_station/event.h"

#include <string.h>

#define FRAME_BUFFER_LEN (LASTRO_SERIAL_HEADER_LEN + LASTRO_SERIAL_MAX_PAYLOAD + LASTRO_SERIAL_CRC_LEN)

static uint8_t g_rx[FRAME_BUFFER_LEN];
static size_t g_rx_len = 0;
static uint8_t g_ready_payload[LASTRO_SERIAL_MAX_PAYLOAD];
static lastro_frame_view_t g_ready;
static bool g_has_ready = false;

static uint16_t read_u16_le(const uint8_t *bytes)
{
    return (uint16_t)bytes[0] | ((uint16_t)bytes[1] << 8u);
}

static uint32_t read_u32_le(const uint8_t *bytes)
{
    return (uint32_t)bytes[0]
        | ((uint32_t)bytes[1] << 8u)
        | ((uint32_t)bytes[2] << 16u)
        | ((uint32_t)bytes[3] << 24u);
}

static uint64_t read_u64_le(const uint8_t *bytes)
{
    uint64_t value = 0;
    for (size_t i = 0; i < 8; ++i) {
        value |= (uint64_t)bytes[i] << (8u * i);
    }
    return value;
}

static void write_u16_le(uint8_t *bytes, uint16_t value)
{
    bytes[0] = (uint8_t)value;
    bytes[1] = (uint8_t)(value >> 8u);
}

static void write_u32_le(uint8_t *bytes, uint32_t value)
{
    bytes[0] = (uint8_t)value;
    bytes[1] = (uint8_t)(value >> 8u);
    bytes[2] = (uint8_t)(value >> 16u);
    bytes[3] = (uint8_t)(value >> 24u);
}

static bool valid_type(uint8_t type)
{
    return type >= LASTRO_MSG_COMMAND && type <= LASTRO_MSG_ERROR;
}

static uint32_t expected_payload_len(uint8_t type)
{
    switch (type) {
        case LASTRO_MSG_COMMAND: return LASTRO_COMMAND_PAYLOAD_LEN;
        case LASTRO_MSG_EVENT_READY: return LASTRO_EVENT_READY_PAYLOAD_LEN;
        case LASTRO_MSG_ACK: return LASTRO_ACK_PAYLOAD_LEN;
        case LASTRO_MSG_ERROR: return LASTRO_ERROR_PAYLOAD_LEN;
        default: return UINT32_MAX;
    }
}

uint32_t lastro_crc32c(const uint8_t *bytes, size_t len)
{
    uint32_t crc = 0xffffffffu;
    if (bytes == NULL && len != 0u) return 0u;
    for (size_t i = 0; i < len; ++i) {
        crc ^= bytes[i];
        for (unsigned bit = 0; bit < 8; ++bit) {
            crc = (crc >> 1u) ^ (0x82f63b78u & (0u - (crc & 1u)));
        }
    }
    return ~crc;
}

bool lastro_transport_encode(const lastro_frame_view_t *frame, uint8_t *out, size_t out_len, size_t *written)
{
    if (frame == NULL || out == NULL || written == NULL || frame->payload == NULL) return false;
    if (!valid_type((uint8_t)frame->type) || frame->payload_len != expected_payload_len((uint8_t)frame->type)) return false;
    if (frame->payload_len > LASTRO_SERIAL_MAX_PAYLOAD) return false;

    const size_t total = LASTRO_SERIAL_HEADER_LEN + frame->payload_len + LASTRO_SERIAL_CRC_LEN;
    if (out_len < total) return false;

    memcpy(out, LASTRO_SERIAL_MAGIC, 4);
    out[4] = LASTRO_SERIAL_VERSION;
    out[5] = (uint8_t)frame->type;
    out[6] = 0;
    out[7] = 0;
    write_u32_le(out + 8, frame->payload_len);
    memcpy(out + LASTRO_SERIAL_HEADER_LEN, frame->payload, frame->payload_len);
    const uint32_t crc = lastro_crc32c(out + 4, 8u + frame->payload_len);
    write_u32_le(out + LASTRO_SERIAL_HEADER_LEN + frame->payload_len, crc);
    *written = total;
    return true;
}

static void discard_prefix(size_t count)
{
    if (count >= g_rx_len) {
        g_rx_len = 0;
        return;
    }
    memmove(g_rx, g_rx + count, g_rx_len - count);
    g_rx_len -= count;
}

static void resync_magic(void)
{
    while (g_rx_len >= 4 && memcmp(g_rx, LASTRO_SERIAL_MAGIC, 4) != 0) {
        discard_prefix(1);
    }
}

static void parse_buffer(void)
{
    while (!g_has_ready) {
        resync_magic();
        if (g_rx_len < LASTRO_SERIAL_HEADER_LEN) return;

        const uint8_t type = g_rx[5];
        const uint32_t payload_len = read_u32_le(g_rx + 8);
        if (g_rx[4] != LASTRO_SERIAL_VERSION
            || !valid_type(type)
            || read_u16_le(g_rx + 6) != 0u
            || payload_len > LASTRO_SERIAL_MAX_PAYLOAD
            || payload_len != expected_payload_len(type)) {
            discard_prefix(1);
            continue;
        }

        const size_t total = LASTRO_SERIAL_HEADER_LEN + payload_len + LASTRO_SERIAL_CRC_LEN;
        if (g_rx_len < total) return;

        const uint32_t expected_crc = read_u32_le(g_rx + LASTRO_SERIAL_HEADER_LEN + payload_len);
        const uint32_t actual_crc = lastro_crc32c(g_rx + 4, 8u + payload_len);
        if (actual_crc != expected_crc) {
            discard_prefix(1);
            continue;
        }

        memcpy(g_ready_payload, g_rx + LASTRO_SERIAL_HEADER_LEN, payload_len);
        g_ready.type = (lastro_message_type_t)type;
        g_ready.payload = g_ready_payload;
        g_ready.payload_len = payload_len;
        g_has_ready = true;
        discard_prefix(total);
    }
}

void lastro_transport_reset(void)
{
    memset(g_rx, 0, sizeof(g_rx));
    g_rx_len = 0;
    memset(g_ready_payload, 0, sizeof(g_ready_payload));
    memset(&g_ready, 0, sizeof(g_ready));
    g_has_ready = false;
}

bool lastro_transport_feed(const uint8_t *bytes, size_t len)
{
    if ((bytes == NULL && len != 0u) || g_has_ready) return false;

    for (size_t i = 0; i < len; ++i) {
        if (g_rx_len == sizeof(g_rx)) {
            discard_prefix(1);
            resync_magic();
        }
        g_rx[g_rx_len++] = bytes[i];
        parse_buffer();
        if (g_has_ready && i + 1u < len) {
            const size_t remaining = len - i - 1u;
            if (remaining > sizeof(g_rx) - g_rx_len) return false;
            memcpy(g_rx + g_rx_len, bytes + i + 1u, remaining);
            g_rx_len += remaining;
            break;
        }
    }
    return true;
}

bool lastro_transport_take(lastro_frame_view_t *out)
{
    if (out == NULL) return false;
    if (!g_has_ready) parse_buffer();
    if (!g_has_ready) return false;
    *out = g_ready;
    g_has_ready = false;
    return true;
}

bool lastro_command_decode(const uint8_t payload[LASTRO_COMMAND_PAYLOAD_LEN], lastro_command_payload_t *out)
{
    if (payload == NULL || out == NULL) return false;
    if (payload[16] < LASTRO_ACTION_ORIGIN || payload[16] > LASTRO_ACTION_REIDENTIFY) return false;
    if (payload[17] != 0 || payload[18] != 0 || payload[19] != 0) return false;

    memcpy(out->capture_id, payload, 16);
    out->action = payload[16];
    memcpy(out->deployment_id, payload + 20, 32);
    memcpy(out->animal_id, payload + 52, 32);
    out->event_sequence = read_u64_le(payload + 84);
    out->identity_revision = read_u32_le(payload + 92);
    memcpy(out->previous_event_hash, payload + 96, 32);
    memcpy(out->expected_old_rfid_hash, payload + 128, 32);
    memcpy(out->from_custodian, payload + 160, 32);
    memcpy(out->to_custodian, payload + 192, 32);
    return true;
}

bool lastro_event_ready_encode(const lastro_event_ready_payload_t *value, uint8_t out[LASTRO_EVENT_READY_PAYLOAD_LEN])
{
    if (value == NULL || out == NULL) return false;
    memcpy(out, value->capture_id, 16);
    memcpy(out + 16, value->event_bytes, 276);
    memcpy(out + 292, value->observed_rfid, 8);
    memcpy(out + 300, value->station_pubkey33, 33);
    memcpy(out + 333, value->station_signature64, 64);
    return true;
}

bool lastro_ack_decode(const uint8_t payload[LASTRO_ACK_PAYLOAD_LEN], lastro_ack_payload_t *out)
{
    if (payload == NULL || out == NULL) return false;
    memcpy(out->capture_id, payload, 16);
    memcpy(out->event_hash, payload + 16, 32);
    return true;
}

bool lastro_error_encode(const lastro_error_payload_t *value, uint8_t out[LASTRO_ERROR_PAYLOAD_LEN])
{
    if (value == NULL || out == NULL) return false;
    if (value->code < LASTRO_STATION_ERROR_INVALID_COMMAND || value->code > LASTRO_STATION_ERROR_RFID_TIMEOUT) return false;
    memcpy(out, value->capture_id, 16);
    write_u16_le(out + 16, (uint16_t)value->code);
    out[18] = 0;
    out[19] = 0;
    return true;
}
