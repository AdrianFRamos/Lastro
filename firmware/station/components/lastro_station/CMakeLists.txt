idf_component_register(
    SRCS
        "station.c"
        "rfid.c"
        "event.c"
        "signer.c"
        "transport.c"
        "runtime.c"
    INCLUDE_DIRS "include"
    REQUIRES mbedtls driver efuse
)
