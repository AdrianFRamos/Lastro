#include "lastro_station/runtime.h"

#include <string.h>

#include "lastro_station/station.h"
#include "lastro_station/transport.h"

#define LASTRO_RUNTIME_MAX_FRAME_LEN \
    (LASTRO_SERIAL_HEADER_LEN + LASTRO_EVENT_READY_PAYLOAD_LEN + LASTRO_SERIAL_CRC_LEN)

static lastro_runtime_write_fn g_write_bytes;
static void *g_write_context;
static uint8_t g_pending_frame[LASTRO_RUNTIME_MAX_FRAME_LEN];
static size_t g_pending_frame_len;

static bool flush_pending(void)
{
    if (g_pending_frame_len == 0u) return true;
    if (g_write_bytes == NULL || !g_write_bytes(g_pending_frame, g_pending_frame_len, g_write_context)) {
        return false;
    }
    memset(g_pending_frame, 0, g_pending_frame_len);
    g_pending_frame_len = 0u;
    return true;
}

static bool queue_frame(lastro_message_type_t type, const uint8_t *payload, size_t payload_len)
{
    if (g_pending_frame_len != 0u || payload == NULL) return false;
    lastro_frame_view_t frame = {
        .type = type,
        .payload = payload,
        .payload_len = (uint32_t)payload_len,
    };
    return lastro_transport_encode(
        &frame,
        g_pending_frame,
        sizeof(g_pending_frame),
        &g_pending_frame_len);
}

static bool queue_error(const lastro_error_payload_t *error)
{
    uint8_t payload[LASTRO_ERROR_PAYLOAD_LEN];
    return lastro_error_encode(error, payload)
        && queue_frame(LASTRO_MSG_ERROR, payload, sizeof(payload));
}

static bool handle_command(const lastro_frame_view_t *frame)
{
    lastro_command_payload_t command;
    lastro_error_payload_t error = {0};

    if (!lastro_command_decode(frame->payload, &command)) {
        memcpy(error.capture_id, frame->payload, sizeof(error.capture_id));
        error.code = LASTRO_STATION_ERROR_INVALID_COMMAND;
        return queue_error(&error);
    }
    if (!lastro_station_submit_command(&command, &error)) {
        return queue_error(&error);
    }
    return true;
}

static bool handle_ack(const lastro_frame_view_t *frame)
{
    lastro_ack_payload_t ack;
    return lastro_ack_decode(frame->payload, &ack) && lastro_station_submit_ack(&ack);
}

static bool handle_inbound_frame(const lastro_frame_view_t *frame)
{
    if (frame == NULL) return false;
    switch (frame->type) {
        case LASTRO_MSG_COMMAND:
            return handle_command(frame);
        case LASTRO_MSG_ACK:
            return handle_ack(frame);
        case LASTRO_MSG_EVENT_READY:
        case LASTRO_MSG_ERROR:
        default:
            return false;
    }
}

bool lastro_runtime_init(lastro_runtime_write_fn write_bytes, void *write_context)
{
    if (write_bytes == NULL) return false;
    g_write_bytes = write_bytes;
    g_write_context = write_context;
    memset(g_pending_frame, 0, sizeof(g_pending_frame));
    g_pending_frame_len = 0u;
    lastro_transport_reset();
    return lastro_station_init();
}

bool lastro_runtime_feed_agent(const uint8_t *bytes, size_t len)
{
    if ((bytes == NULL && len != 0u) || g_write_bytes == NULL) return false;
    if (!flush_pending()) return false;
    if (!lastro_transport_feed(bytes, len)) return false;

    lastro_frame_view_t frame;
    while (lastro_transport_take(&frame)) {
        if (!handle_inbound_frame(&frame)) return false;
        if (g_pending_frame_len != 0u && !flush_pending()) return false;
    }
    return true;
}

bool lastro_runtime_observe_rfid(const lastro_canonical_rfid_t *rfid)
{
    return lastro_station_observe_rfid(rfid);
}

void lastro_runtime_poll(void)
{
    if (g_write_bytes == NULL || !flush_pending()) return;

    lastro_station_poll();

    lastro_error_payload_t error;
    if (lastro_station_take_error(&error)) {
        if (queue_error(&error)) (void)flush_pending();
        return;
    }

    lastro_event_ready_payload_t event_ready;
    if (lastro_station_take_event_ready(&event_ready)) {
        uint8_t payload[LASTRO_EVENT_READY_PAYLOAD_LEN];
        if (lastro_event_ready_encode(&event_ready, payload)
            && queue_frame(LASTRO_MSG_EVENT_READY, payload, sizeof(payload))) {
            (void)flush_pending();
        }
    }
}

bool lastro_runtime_has_pending_output(void)
{
    return g_pending_frame_len != 0u;
}
