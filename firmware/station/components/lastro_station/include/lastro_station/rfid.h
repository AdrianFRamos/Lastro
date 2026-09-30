#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* The reader adapter must decode the logical unsigned 64-bit FDX-B identifier first.
 * Lastro then represents that value as exactly eight big-endian bytes. */
#define LASTRO_CANONICAL_RFID_LEN 8u

typedef struct {
    uint8_t bytes[LASTRO_CANONICAL_RFID_LEN];
} lastro_canonical_rfid_t;

/** Advance adapter-owned reader I/O once. Reader-specific bus access stays inside rfid.c. */
void lastro_rfid_poll(void);

/** Feed reader-specific bytes into the parser for the selected physical reader. */
bool lastro_rfid_feed(const uint8_t *data, size_t len);

/** Return one canonical RFID only after a complete valid reader frame. */
bool lastro_rfid_take(lastro_canonical_rfid_t *out);

/** SHA-256("LASTRO_RFID\0" || canonical_rfid[8]). */
bool lastro_rfid_hash(const lastro_canonical_rfid_t *rfid, uint8_t out_hash[32]);

#ifdef __cplusplus
}
#endif
