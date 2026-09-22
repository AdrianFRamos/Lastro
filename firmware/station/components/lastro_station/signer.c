#include "lastro_station/signer.h"

#include <string.h>

#include "psa/crypto.h"
#include "sdkconfig.h"

#if CONFIG_LASTRO_STATION_USE_EFUSE_KEY
#include "esp_efuse.h"
#include "psa_crypto_driver_esp_ecdsa.h"
#include "psa_crypto_driver_esp_ecdsa_contexts.h"
#endif

#define LASTRO_STATION_EVENT_LEN 276u
#if !CONFIG_LASTRO_STATION_USE_EFUSE_KEY
#define LASTRO_P256_PRIVATE_KEY_LEN 32u
#endif
#define LASTRO_P256_UNCOMPRESSED_PUBKEY_LEN 65u

/*
 * Development signing uses a software-imported key so the complete protocol can be
 * validated before irreversible provisioning. The optional eFuse backend imports only
 * an opaque hardware key reference. This component never writes, burns, or changes
 * eFuse state.
 */

static const uint8_t P256_ORDER[32] = {
    0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00,
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84,
    0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63, 0x25, 0x51,
};

static const uint8_t P256_HALF_ORDER[32] = {
    0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00,
    0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42,
    0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31, 0x92, 0xa8,
};

#if !CONFIG_LASTRO_STATION_USE_EFUSE_KEY
static int hex_nibble(char value)
{
    if (value >= '0' && value <= '9') return value - '0';
    if (value >= 'a' && value <= 'f') return value - 'a' + 10;
    if (value >= 'A' && value <= 'F') return value - 'A' + 10;
    return -1;
}

static bool decode_private_key(uint8_t out[LASTRO_P256_PRIVATE_KEY_LEN])
{
#if CONFIG_LASTRO_STATION_USE_TEST_VECTOR_KEY
    memset(out, 0, LASTRO_P256_PRIVATE_KEY_LEN);
    out[LASTRO_P256_PRIVATE_KEY_LEN - 1u] = 1u;
    return true;
#else
    const char *hex = CONFIG_LASTRO_STATION_DEV_PRIVATE_KEY_HEX;
    if (hex == NULL || strlen(hex) != LASTRO_P256_PRIVATE_KEY_LEN * 2u) {
        return false;
    }
    for (size_t i = 0; i < LASTRO_P256_PRIVATE_KEY_LEN; ++i) {
        int high = hex_nibble(hex[i * 2u]);
        int low = hex_nibble(hex[i * 2u + 1u]);
        if (high < 0 || low < 0) {
            return false;
        }
        out[i] = (uint8_t)((high << 4) | low);
    }

    uint8_t nonzero = 0;
    for (size_t i = 0; i < LASTRO_P256_PRIVATE_KEY_LEN; ++i) nonzero |= out[i];
    if (nonzero == 0 || memcmp(out, P256_ORDER, sizeof(P256_ORDER)) >= 0) {
        return false;
    }
    return true;
#endif
}

static bool import_development_key(psa_key_id_t *key_id)
{
    uint8_t private_key[LASTRO_P256_PRIVATE_KEY_LEN];
    if (key_id == NULL || !decode_private_key(private_key)) {
        return false;
    }

    psa_key_attributes_t attributes = PSA_KEY_ATTRIBUTES_INIT;
    psa_set_key_type(&attributes, PSA_KEY_TYPE_ECC_KEY_PAIR(PSA_ECC_FAMILY_SECP_R1));
    psa_set_key_bits(&attributes, 256);
    psa_set_key_usage_flags(&attributes, PSA_KEY_USAGE_SIGN_HASH);
    psa_set_key_algorithm(&attributes, PSA_ALG_ECDSA(PSA_ALG_SHA_256));

    psa_status_t status = psa_crypto_init();
    if (status == PSA_SUCCESS) {
        status = psa_import_key(
            &attributes,
            private_key,
            sizeof(private_key),
            key_id);
    }
    psa_reset_key_attributes(&attributes);
    memset(private_key, 0, sizeof(private_key));
    return status == PSA_SUCCESS;
}

#endif

#if CONFIG_LASTRO_STATION_USE_EFUSE_KEY
static esp_efuse_block_t configured_efuse_block(void)
{
    return (esp_efuse_block_t)(EFUSE_BLK_KEY0 + CONFIG_LASTRO_STATION_EFUSE_KEY_BLOCK_INDEX);
}

static bool validate_efuse_key(void)
{
    const esp_efuse_block_t block = configured_efuse_block();
    if (block < EFUSE_BLK_KEY0 || block >= EFUSE_BLK_KEY_MAX) {
        return false;
    }
    if (!esp_efuse_is_ecdsa_p256_curve_supported()) {
        return false;
    }
    if (esp_efuse_get_key_purpose(block) != ESP_EFUSE_KEY_PURPOSE_ECDSA_KEY_P256) {
        return false;
    }
    /* A hardware-private Station key must not remain software-readable. */
    if (!esp_efuse_get_key_dis_read(block)) {
        return false;
    }
    return true;
}

static bool import_efuse_key(psa_key_id_t *key_id)
{
#if !defined(CONFIG_MBEDTLS_HARDWARE_ECDSA_SIGN)
    (void)key_id;
    return false;
#else
    if (key_id == NULL || !validate_efuse_key()) {
        return false;
    }

    psa_status_t status = psa_crypto_init();
    if (status != PSA_SUCCESS) {
        return false;
    }

    esp_ecdsa_opaque_key_t opaque_key = {0};
    opaque_key.curve = ESP_ECDSA_CURVE_SECP256R1;
    opaque_key.efuse_block = (uint8_t)configured_efuse_block();

    psa_key_attributes_t attributes = PSA_KEY_ATTRIBUTES_INIT;
    psa_set_key_type(&attributes, PSA_KEY_TYPE_ECC_KEY_PAIR(PSA_ECC_FAMILY_SECP_R1));
    psa_set_key_bits(&attributes, 256);
    psa_set_key_usage_flags(&attributes, PSA_KEY_USAGE_SIGN_HASH);
    psa_set_key_algorithm(&attributes, PSA_ALG_ECDSA(PSA_ALG_SHA_256));
    psa_set_key_lifetime(&attributes, PSA_KEY_LIFETIME_ESP_ECDSA_VOLATILE);

    status = psa_import_key(
        &attributes,
        (const uint8_t *)&opaque_key,
        sizeof(opaque_key),
        key_id);
    psa_reset_key_attributes(&attributes);
    memset(&opaque_key, 0, sizeof(opaque_key));
    return status == PSA_SUCCESS;
#endif
}
#endif

static bool import_signing_key(psa_key_id_t *key_id)
{
#if CONFIG_LASTRO_STATION_USE_EFUSE_KEY
    return import_efuse_key(key_id);
#else
    return import_development_key(key_id);
#endif
}

static int compare_big_endian_32(const uint8_t left[32], const uint8_t right[32])
{
    for (size_t i = 0; i < 32; ++i) {
        if (left[i] < right[i]) return -1;
        if (left[i] > right[i]) return 1;
    }
    return 0;
}

static void subtract_big_endian_32(
    const uint8_t minuend[32],
    const uint8_t subtrahend[32],
    uint8_t out[32])
{
    unsigned borrow = 0;
    for (size_t i = 32; i-- > 0;) {
        unsigned left = minuend[i];
        unsigned right = (unsigned)subtrahend[i] + borrow;
        if (left >= right) {
            out[i] = (uint8_t)(left - right);
            borrow = 0;
        } else {
            out[i] = (uint8_t)(256u + left - right);
            borrow = 1;
        }
    }
}

bool lastro_signer_public_key(uint8_t out[LASTRO_P256_PUBKEY_COMPRESSED_LEN])
{
    if (out == NULL) {
        return false;
    }

    psa_key_id_t key_id = 0;
    if (!import_signing_key(&key_id)) {
        return false;
    }

    uint8_t uncompressed[LASTRO_P256_UNCOMPRESSED_PUBKEY_LEN];
    size_t written = 0;
    psa_status_t status = psa_export_public_key(
        key_id,
        uncompressed,
        sizeof(uncompressed),
        &written);
    psa_destroy_key(key_id);

    if (status != PSA_SUCCESS || written != sizeof(uncompressed) || uncompressed[0] != 0x04) {
        return false;
    }

    out[0] = (uint8_t)(0x02u | (uncompressed[64] & 1u));
    memcpy(out + 1, uncompressed + 1, 32);
    memset(uncompressed, 0, sizeof(uncompressed));
    return true;
}

bool lastro_signer_sign_event(
    const uint8_t *message,
    size_t message_len,
    uint8_t out_signature[LASTRO_P256_SIGNATURE_COMPACT_LEN])
{
    if (message == NULL || out_signature == NULL || message_len != LASTRO_STATION_EVENT_LEN) {
        return false;
    }

    psa_key_id_t key_id = 0;
    if (!import_signing_key(&key_id)) {
        return false;
    }

    uint8_t digest[32];
    size_t digest_len = 0;
    psa_status_t status = psa_hash_compute(
        PSA_ALG_SHA_256,
        message,
        message_len,
        digest,
        sizeof(digest),
        &digest_len);

    size_t signature_len = 0;
    if (status == PSA_SUCCESS && digest_len == sizeof(digest)) {
        status = psa_sign_hash(
            key_id,
            PSA_ALG_ECDSA(PSA_ALG_SHA_256),
            digest,
            digest_len,
            out_signature,
            LASTRO_P256_SIGNATURE_COMPACT_LEN,
            &signature_len);
    }
    psa_destroy_key(key_id);
    memset(digest, 0, sizeof(digest));

    if (status != PSA_SUCCESS || signature_len != LASTRO_P256_SIGNATURE_COMPACT_LEN) {
        memset(out_signature, 0, LASTRO_P256_SIGNATURE_COMPACT_LEN);
        return false;
    }

    uint8_t *s = out_signature + 32;
    if (compare_big_endian_32(s, P256_HALF_ORDER) > 0) {
        uint8_t normalized[32];
        subtract_big_endian_32(P256_ORDER, s, normalized);
        memcpy(s, normalized, sizeof(normalized));
        memset(normalized, 0, sizeof(normalized));
    }
    return true;
}
