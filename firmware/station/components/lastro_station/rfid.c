#include "lastro_station/rfid.h"

#include "mbedtls/sha256.h"

/*
 * The reader parser intentionally remains unimplemented until docs/HARDWARE.md contains
 * the selected reader model, framing, integrity rule, pinout, and repeated real captures.
 * Inventing those details would make a false physical-observation claim.
 */
void lastro_rfid_poll(void)
{
    /* No reader bus or frame contract exists until the physical reader gate is satisfied. */
}

bool lastro_rfid_feed(const uint8_t *data, size_t len)
{
    (void)data;
    (void)len;
    return false;
}

bool lastro_rfid_take(lastro_canonical_rfid_t *out)
{
    (void)out;
    return false;
}

bool lastro_rfid_hash(const lastro_canonical_rfid_t *rfid, uint8_t out_hash[32])
{
    static const uint8_t domain[] = "LASTRO_RFID\0";
    mbedtls_sha256_context context;
    int status;

    if (rfid == NULL || out_hash == NULL) {
        return false;
    }

    mbedtls_sha256_init(&context);
    status = mbedtls_sha256_starts(&context, 0);
    if (status == 0) status = mbedtls_sha256_update(&context, domain, sizeof(domain) - 1u);
    if (status == 0) status = mbedtls_sha256_update(&context, rfid->bytes, LASTRO_CANONICAL_RFID_LEN);
    if (status == 0) status = mbedtls_sha256_finish(&context, out_hash);
    mbedtls_sha256_free(&context);
    return status == 0;
}
