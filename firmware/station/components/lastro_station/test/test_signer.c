#include "unity.h"
#include "lastro_station/event.h"
#include "lastro_station/signer.h"

#include <string.h>

#include "psa/crypto.h"
#include "sdkconfig.h"

static const uint8_t EXPECTED_COMPRESSED_PUBLIC_KEY[33] = {
    0x03, 0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc,
    0xe6, 0xe5, 0x63, 0xa4, 0x40, 0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d,
    0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45, 0xd8, 0x98, 0xc2, 0x96,
};

static const uint8_t EXPECTED_UNCOMPRESSED_PUBLIC_KEY[65] = {
    0x04,
    0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4, 0x40,
    0xf2, 0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45, 0xd8, 0x98, 0xc2, 0x96,
    0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e, 0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e, 0x16,
    0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce, 0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf, 0x51, 0xf5,
};

static const uint8_t P256_HALF_ORDER[32] = {
    0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00,
    0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42,
    0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31, 0x92, 0xa8,
};

static const uint8_t ORIGIN_NEW_RFID_HASH[32] = {
    0x8a, 0x60, 0x45, 0x28, 0xf1, 0x90, 0x62, 0xcc, 0x9a, 0xda, 0x87, 0xa5, 0xe2, 0x25, 0xa5, 0xc9,
    0x67, 0xe2, 0xcf, 0x14, 0x8c, 0x85, 0xd8, 0x6d, 0xaf, 0x02, 0xd0, 0x42, 0x12, 0xeb, 0xf6, 0xe1,
};

static void build_origin_event(uint8_t out[LASTRO_STATION_EVENT_LEN])
{
    lastro_station_event_fields_t fields = {0};
    fields.action = LASTRO_ACTION_ORIGIN;
    memset(fields.deployment_id, 0xd0, sizeof(fields.deployment_id));
    memset(fields.animal_id, 0x11, sizeof(fields.animal_id));
    const uint8_t station_id[32] = {
        0x56, 0xc2, 0x66, 0xd8, 0xab, 0x41, 0xa3, 0x7e, 0x7a, 0x3b, 0xb8, 0x0b, 0x33, 0xf6, 0x24, 0x9c,
        0x05, 0xbd, 0xb9, 0x65, 0xc6, 0x8b, 0xbe, 0xb4, 0x77, 0x53, 0x03, 0xc6, 0x95, 0x94, 0xdf, 0x77,
    };
    memcpy(fields.station_id, station_id, sizeof(station_id));
    fields.event_sequence = 1;
    fields.identity_revision = 1;
    memcpy(fields.new_rfid_hash, ORIGIN_NEW_RFID_HASH, sizeof(ORIGIN_NEW_RFID_HASH));
    memset(fields.to_custodian, 0xa1, sizeof(fields.to_custodian));
    TEST_ASSERT_TRUE(lastro_event_encode(&fields, out));
}

static bool less_than_or_equal_32(const uint8_t left[32], const uint8_t right[32])
{
    for (size_t i = 0; i < 32; ++i) {
        if (left[i] < right[i]) return true;
        if (left[i] > right[i]) return false;
    }
    return true;
}

TEST_CASE("signer rejects messages that are not exactly 276 bytes", "[lastro][contract]")
{
    /* PURPOSE: the Station must never sign bytes outside the StationEvent contract.
     * ASSERT: lengths other than exactly 276 bytes are rejected before signing.
     * FAILURE: the Station could sign an ambiguous or attacker-controlled message shape. */
    uint8_t message[LASTRO_STATION_EVENT_LEN] = {0};
    uint8_t signature[LASTRO_P256_SIGNATURE_COMPACT_LEN];
    TEST_ASSERT_FALSE(lastro_signer_sign_event(message, LASTRO_STATION_EVENT_LEN - 1, signature));
    TEST_ASSERT_FALSE(lastro_signer_sign_event(message, LASTRO_STATION_EVENT_LEN + 1, signature));
}

TEST_CASE("development signer exposes the frozen compressed Station public key", "[lastro][contract]")
{
#if !CONFIG_LASTRO_STATION_USE_TEST_VECTOR_KEY
    TEST_IGNORE_MESSAGE("Enable LASTRO_STATION_USE_TEST_VECTOR_KEY for the frozen Station identity contract.");
#else
    /* PURPOSE: Station identity must be stable and match the registered development key.
     * ASSERT: the signer exposes exactly the frozen 33-byte compressed P-256 public key.
     * FAILURE: StationID registration and signature verification could bind to different keys. */
    uint8_t public_key[LASTRO_P256_PUBKEY_COMPRESSED_LEN];
    TEST_ASSERT_TRUE(lastro_signer_public_key(public_key));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(EXPECTED_COMPRESSED_PUBLIC_KEY, public_key, sizeof(public_key));
#endif
}

TEST_CASE("development signature verifies as P-256 SHA-256 over the raw StationEvent", "[lastro][contract]")
{
#if !CONFIG_LASTRO_STATION_USE_TEST_VECTOR_KEY
    TEST_IGNORE_MESSAGE("Enable LASTRO_STATION_USE_TEST_VECTOR_KEY for the frozen signature interoperability contract.");
#else
    /* PURPOSE: device output must interoperate with independent P-256 verification.
     * ASSERT: PSA verifies the compact signature as ECDSA P-256/SHA-256 over the raw 276 bytes.
     * FAILURE: firmware signatures would not match the verifier or Solana precompile semantics. */
    uint8_t event[LASTRO_STATION_EVENT_LEN];
    uint8_t signature[LASTRO_P256_SIGNATURE_COMPACT_LEN];
    build_origin_event(event);
    TEST_ASSERT_TRUE(lastro_signer_sign_event(event, sizeof(event), signature));

    TEST_ASSERT_EQUAL(PSA_SUCCESS, psa_crypto_init());
    psa_key_attributes_t attributes = PSA_KEY_ATTRIBUTES_INIT;
    psa_set_key_type(&attributes, PSA_KEY_TYPE_ECC_PUBLIC_KEY(PSA_ECC_FAMILY_SECP_R1));
    psa_set_key_bits(&attributes, 256);
    psa_set_key_usage_flags(&attributes, PSA_KEY_USAGE_VERIFY_MESSAGE);
    psa_set_key_algorithm(&attributes, PSA_ALG_ECDSA(PSA_ALG_SHA_256));
    psa_key_id_t key_id = 0;
    TEST_ASSERT_EQUAL(
        PSA_SUCCESS,
        psa_import_key(
            &attributes,
            EXPECTED_UNCOMPRESSED_PUBLIC_KEY,
            sizeof(EXPECTED_UNCOMPRESSED_PUBLIC_KEY),
            &key_id));
    psa_reset_key_attributes(&attributes);
    TEST_ASSERT_EQUAL(
        PSA_SUCCESS,
        psa_verify_message(
            key_id,
            PSA_ALG_ECDSA(PSA_ALG_SHA_256),
            event,
            sizeof(event),
            signature,
            sizeof(signature)));
    TEST_ASSERT_EQUAL(PSA_SUCCESS, psa_destroy_key(key_id));
#endif
}

TEST_CASE("signer emits canonical compact low-S signatures", "[lastro][contract]")
{
#if !CONFIG_LASTRO_STATION_USE_EFUSE_KEY
    if (!CONFIG_LASTRO_STATION_USE_TEST_VECTOR_KEY && CONFIG_LASTRO_STATION_DEV_PRIVATE_KEY_HEX[0] == '\0') {
        TEST_IGNORE_MESSAGE("Configure a development signing key before running the low-S signer contract.");
    }
#endif
    /* PURPOSE: the output format must match the Secp256r1 precompile contract.
     * ASSERT: output is a 64-byte compact signature whose S value is canonical low-S.
     * FAILURE: malleable or differently encoded signatures could be rejected on-chain. */
    uint8_t event[LASTRO_STATION_EVENT_LEN];
    uint8_t signature[LASTRO_P256_SIGNATURE_COMPACT_LEN];
    build_origin_event(event);
    TEST_ASSERT_EQUAL_UINT32(64, sizeof(signature));
    TEST_ASSERT_TRUE(lastro_signer_sign_event(event, sizeof(event), signature));
    TEST_ASSERT_TRUE(less_than_or_equal_32(signature + 32, P256_HALF_ORDER));
}

TEST_CASE("eFuse signer preserves the Station protocol contract", "[lastro][contract][hardware]")
{
#if !CONFIG_LASTRO_STATION_USE_EFUSE_KEY
    TEST_IGNORE_MESSAGE("Enable LASTRO_STATION_USE_EFUSE_KEY only on a manually provisioned ESP32-C5.");
#else
    /* PURPOSE: H1 hardening must not change StationEvent/P-256 semantics.
     * ASSERT: a provisioned hardware key exports one stable compressed public key and signs low-S.
     * FAILURE: eFuse hardening could silently create an incompatible protocol variant or software fallback. */
    uint8_t first_public_key[LASTRO_P256_PUBKEY_COMPRESSED_LEN];
    uint8_t second_public_key[LASTRO_P256_PUBKEY_COMPRESSED_LEN];
    uint8_t event[LASTRO_STATION_EVENT_LEN];
    uint8_t signature[LASTRO_P256_SIGNATURE_COMPACT_LEN];

    TEST_ASSERT_TRUE(lastro_signer_public_key(first_public_key));
    TEST_ASSERT_TRUE(lastro_signer_public_key(second_public_key));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(first_public_key, second_public_key, sizeof(first_public_key));
    TEST_ASSERT_TRUE(first_public_key[0] == 0x02 || first_public_key[0] == 0x03);

    build_origin_event(event);
    TEST_ASSERT_TRUE(lastro_signer_sign_event(event, sizeof(event), signature));
    TEST_ASSERT_TRUE(less_than_or_equal_32(signature + 32, P256_HALF_ORDER));
#endif
}
