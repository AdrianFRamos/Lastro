/* Lastro Station ESP32-C5 board entrypoint. */
#include <stddef.h>
#include <stdint.h>

#include "driver/usb_serial_jtag.h"
#include "esp_err.h"
#include "esp_log.h"
#include "esp_timer.h"
#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "lastro_station/rfid.h"
#include "lastro_station/runtime.h"
#include "lastro_station/station.h"

#define LASTRO_AGENT_USB_BUFFER_SIZE 1024u
#define LASTRO_AGENT_READ_CHUNK_SIZE 512u
#define LASTRO_AGENT_IO_TIMEOUT_MS 20u

static const char *TAG = "lastro_station";

static bool write_agent_bytes(const uint8_t *bytes, size_t len, void *context)
{
    size_t offset = 0u;
    (void)context;

    if (bytes == NULL || len == 0u) return false;
    while (offset < len) {
        const int written = usb_serial_jtag_write_bytes(
            bytes + offset,
            len - offset,
            pdMS_TO_TICKS(LASTRO_AGENT_IO_TIMEOUT_MS));
        if (written <= 0) return false;
        offset += (size_t)written;
    }
    return true;
}

static void fail_closed(const char *message)
{
    ESP_LOGE(TAG, "%s", message);
    vTaskSuspend(NULL);
}

void app_main(void)
{
    usb_serial_jtag_driver_config_t usb_config = {
        .tx_buffer_size = LASTRO_AGENT_USB_BUFFER_SIZE,
        .rx_buffer_size = LASTRO_AGENT_USB_BUFFER_SIZE,
    };
    uint8_t agent_bytes[LASTRO_AGENT_READ_CHUNK_SIZE];
    int64_t last_tick_us = esp_timer_get_time();

    if (usb_serial_jtag_driver_install(&usb_config) != ESP_OK) {
        fail_closed("Cannot initialize the native USB Serial/JTAG Agent transport.");
        return;
    }
    if (!lastro_runtime_init(write_agent_bytes, NULL)) {
        fail_closed("Station runtime initialization failed; verify the configured P-256 signer.");
        return;
    }

    ESP_LOGI(
        TAG,
        "Agent transport is ready on native USB Serial/JTAG. "
        "RFID capture remains disabled until the documented reader adapter produces a canonical RFID.");

    for (;;) {
        const int received = usb_serial_jtag_read_bytes(
            agent_bytes,
            sizeof(agent_bytes),
            pdMS_TO_TICKS(LASTRO_AGENT_IO_TIMEOUT_MS));
        if (received > 0 && !lastro_runtime_feed_agent(agent_bytes, (size_t)received)) {
            ESP_LOGW(TAG, "Rejected invalid or temporarily unprocessable Agent serial input.");
        }

        const int64_t now_us = esp_timer_get_time();
        const int64_t elapsed_us = now_us - last_tick_us;
        if (elapsed_us > 0) {
            const uint64_t elapsed_ms = (uint64_t)elapsed_us / 1000u;
            last_tick_us = now_us - (elapsed_us % 1000);
            lastro_station_elapse(elapsed_ms > UINT32_MAX ? UINT32_MAX : (uint32_t)elapsed_ms);
        }

        if (lastro_station_state() == LASTRO_STATION_WAIT_RFID) {
            lastro_canonical_rfid_t observed;
            lastro_rfid_poll();
            if (lastro_rfid_take(&observed) && !lastro_runtime_observe_rfid(&observed)) {
                ESP_LOGW(TAG, "Reader adapter produced an RFID outside the active capture contract.");
            }
        }

        lastro_runtime_poll();
    }
}
