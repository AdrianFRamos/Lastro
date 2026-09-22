idf_component_register(SRCS "app_main.c"
                       REQUIRES lastro_station
                       PRIV_REQUIRES freertos log esp_driver_usb_serial_jtag)
