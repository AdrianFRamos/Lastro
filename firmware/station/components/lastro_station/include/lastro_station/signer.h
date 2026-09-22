/*
 * Station P-256 signing boundary.
 *
 * The implementation signs the raw 276-byte StationEvent using ECDSA P-256/SHA-256,
 * outputs compact r||s, and enforces low-S. Development-key and eFuse backends must expose
 * the same API so later H1 hardening cannot change protocol bytes.
 */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define LASTRO_P256_PUBKEY_COMPRESSED_LEN 33u
#define LASTRO_P256_SIGNATURE_COMPACT_LEN 64u

/** Return the SEC1-compressed public key for the active Station signing key. */
bool lastro_signer_public_key(uint8_t out[LASTRO_P256_PUBKEY_COMPRESSED_LEN]);

/**
 * Sign exactly one raw 276-byte StationEvent with ECDSA P-256/SHA-256 and return low-S r||s.
 * The caller passes raw event bytes, never event_hash, so the signer cannot double-hash the
 * protocol message accidentally.
 */
bool lastro_signer_sign_event(
    const uint8_t *message,
    size_t message_len,
    uint8_t out_signature[LASTRO_P256_SIGNATURE_COMPACT_LEN]);

#ifdef __cplusplus
}
#endif
