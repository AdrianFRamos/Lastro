#include "lastro_station/rfid.h"

#include "psa/crypto.h"

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
    psa_hash_operation_t operation = PSA_HASH_OPERATION_INIT;
    size_t out_len = 0;
    psa_status_t status;

    if (rfid == NULL || out_hash == NULL) {
        return false;
    }

    status = psa_crypto_init();
    if (status == PSA_SUCCESS) status = psa_hash_setup(&operation, PSA_ALG_SHA_256);
    if (status == PSA_SUCCESS) status = psa_hash_update(&operation, domain, sizeof(domain) - 1u);
    if (status == PSA_SUCCESS) status = psa_hash_update(&operation, rfid->bytes, LASTRO_CANONICAL_RFID_LEN);
    if (status == PSA_SUCCESS) status = psa_hash_finish(&operation, out_hash, 32u, &out_len);
    psa_hash_abort(&operation);
    return status == PSA_SUCCESS && out_len == 32u;
}
