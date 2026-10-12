/*
 * Host regression for the serial parser (review finding A4): a burst of frames larger than
 * the frame buffer, delivered in 512-byte USB reads, must be fully consumed and every frame
 * exposed in order; the parser must keep accepting input afterwards.
 * Built and run by test_transport_host.py with the system C compiler.
 */
#include <stdio.h>
#include <string.h>

#include "lastro_station/transport.h"

#define ACK_FRAME_LEN (LASTRO_SERIAL_HEADER_LEN + LASTRO_ACK_PAYLOAD_LEN + LASTRO_SERIAL_CRC_LEN)
#define FRAMES 24u
#define USB_READ 512u

static int fail(const char *message)
{
    fprintf(stderr, "FAIL: %s\n", message);
    return 1;
}

int main(void)
{
    static uint8_t stream[FRAMES * ACK_FRAME_LEN];
    size_t stream_len = 0u;
    for (unsigned i = 0; i < FRAMES; ++i) {
        uint8_t payload[LASTRO_ACK_PAYLOAD_LEN];
        memset(payload, (int)(i + 1u), sizeof(payload));
        lastro_frame_view_t frame = {LASTRO_MSG_ACK, payload, sizeof(payload)};
        size_t written = 0u;
        if (!lastro_transport_encode(&frame, stream + stream_len, sizeof(stream) - stream_len, &written))
            return fail("encode");
        stream_len += written;
    }

    lastro_transport_reset();
    unsigned seen = 0u;
    for (size_t chunk = 0u; chunk < stream_len; chunk += USB_READ) {
        const size_t len = stream_len - chunk < USB_READ ? stream_len - chunk : USB_READ;
        size_t offset = 0u;
        lastro_frame_view_t frame;
        do {
            offset += lastro_transport_feed_some(stream + chunk + offset, len - offset);
            while (lastro_transport_take(&frame)) {
                if (frame.type != LASTRO_MSG_ACK || frame.payload_len != LASTRO_ACK_PAYLOAD_LEN)
                    return fail("unexpected frame");
                if (frame.payload[0] != (uint8_t)(seen + 1u)) return fail("frames out of order");
                ++seen;
            }
        } while (offset < len);
    }
    if (seen != FRAMES) return fail("a frame of the burst was lost");

    /* The parser still accepts a frame after the burst. */
    uint8_t payload[LASTRO_ACK_PAYLOAD_LEN] = {0xab};
    uint8_t frame_bytes[ACK_FRAME_LEN];
    size_t written = 0u;
    lastro_frame_view_t ack = {LASTRO_MSG_ACK, payload, sizeof(payload)};
    if (!lastro_transport_encode(&ack, frame_bytes, sizeof(frame_bytes), &written)) return fail("encode");
    if (lastro_transport_feed_some(frame_bytes, written) != written) return fail("parser stopped accepting input");
    lastro_frame_view_t after;
    if (!lastro_transport_take(&after) || after.payload[0] != 0xab) return fail("frame after burst");
    printf("ok %u frames\n", seen);
    return 0;
}
