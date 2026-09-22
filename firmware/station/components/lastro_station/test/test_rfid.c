#include "unity.h"
#include "lastro_station/rfid.h"
#include "test_fixture.h"

TEST_CASE("rfid_valid_frame_yields_one_canonical_id", "[lastro][contract]")
{
    /* PURPOSE: A real valid reader frame must yield exactly one canonical RFID. */
    /* ARRANGE: Feed a captured frame from the selected physical reader. */
    /* ACTION: Parse the complete frame and take the canonical value. */
    /* ASSERT: Exactly the independently decoded 8-byte ISO 11784 value is returned. */
    /* FAILURE MEANS: valid reader input does not deterministically identify the physical tag. */
    TEST_IGNORE_MESSAGE("Requires selected reader model, framing, integrity rule, and captured hardware fixture.");
}

TEST_CASE("rfid_fragmented_frame_yields_same_id", "[lastro][contract]")
{
    /* PURPOSE: Reader input fragmentation must not change physical identity. */
    /* ARRANGE: Use one captured valid reader frame. */
    /* ACTION: Feed the frame at every possible chunk boundary. */
    /* ASSERT: Every fragmentation yields the same canonical RFID. */
    /* FAILURE MEANS: UART chunking changes the identifier extracted from the same tag. */
    TEST_IGNORE_MESSAGE("Requires selected reader model and captured hardware fixture.");
}

TEST_CASE("rfid_noise_outside_valid_frame_does_not_emit_id", "[lastro][contract]")
{
    /* PURPOSE: Reader-invalid bytes must never become a physical RFID observation. */
    /* ARRANGE: Use documented noise/invalid framing followed by one captured valid frame. */
    /* ACTION: Feed the byte stream to the reader adapter. */
    /* ASSERT: Only the later valid frame produces an RFID. */
    /* FAILURE MEANS: serial noise can be promoted to signed physical evidence. */
    TEST_IGNORE_MESSAGE("Requires selected reader framing and captured hardware fixture.");
}

TEST_CASE("rfid_truncated_frame_emits_nothing", "[lastro][contract]")
{
    /* PURPOSE: Partial reader frames must not be accepted. */
    /* ARRANGE: Remove trailing bytes from a captured valid frame. */
    /* ACTION: Feed the partial frame. */
    /* ASSERT: No canonical RFID is available. */
    /* FAILURE MEANS: incomplete serial data can become physical evidence. */
    TEST_IGNORE_MESSAGE("Requires selected reader framing and captured hardware fixture.");
}

TEST_CASE("rfid_reader_integrity_error_is_rejected", "[lastro][contract]")
{
    /* PURPOSE: Enforce the selected reader's documented framing/integrity mechanism. */
    /* ARRANGE: Corrupt a captured frame according to the reader documentation. */
    /* ACTION: Feed the invalid frame. */
    /* ASSERT: No RFID is emitted. */
    /* FAILURE MEANS: a frame the reader protocol defines as invalid can become canonical identity. */
    TEST_IGNORE_MESSAGE("Requires selected reader integrity rule and captured hardware fixture.");
}

TEST_CASE("rfid_two_consecutive_tags_are_not_merged", "[lastro][contract]")
{
    /* PURPOSE: Reader framing must keep consecutive physical observations separate. */
    /* ARRANGE: Concatenate two captured valid tag frames. */
    /* ACTION: Feed the combined stream. */
    /* ASSERT: Two independent canonical RFIDs are emitted in order. */
    /* FAILURE MEANS: two physical observations can be merged into a nonexistent RFID. */
    TEST_IGNORE_MESSAGE("Requires selected reader framing and two captured hardware fixtures.");
}

TEST_CASE("rfid_hash_matches_rust_vector", "[lastro][contract]")
{
    /* PURPOSE: Keep RFID domain separation/hash interoperable across C, Rust, and TypeScript. */
    /* ARRANGE: Use the committed canonical 8-byte RFID A vector. */
    /* ACTION: Hash with SHA-256(\"LASTRO_RFID\\0\" || canonical_rfid). */
    /* ASSERT: Hash equals the committed cross-language fixture. */
    /* FAILURE MEANS: physical identifier hashing diverges between Station and verifier/backend. */
    lastro_canonical_rfid_t rfid;
    uint8_t actual[32];
    uint8_t expected[32];
    fixture_decode_hex(FIXTURE_RFID_A_HEX, rfid.bytes, sizeof(rfid.bytes));
    fixture_decode_hex(FIXTURE_RFID_A_HASH_HEX, expected, sizeof(expected));
    TEST_ASSERT_EQUAL_UINT32(LASTRO_CANONICAL_RFID_LEN, sizeof(rfid.bytes));
    TEST_ASSERT_TRUE(lastro_rfid_hash(&rfid, actual));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(expected, actual, sizeof(actual));
}

TEST_CASE("rfid_canonical_value_is_exactly_8_bytes", "[lastro][contract]")
{
    /* PURPOSE: Freeze cross-reader output to the logical unsigned 64-bit FDX-B identifier. */
    /* ARRANGE: Use a real frame whose logical identification value was decoded independently. */
    /* ACTION: Parse it through the selected reader adapter. */
    /* ASSERT: The emitted value is exactly the expected 8-byte big-endian Lastro representation. */
    /* FAILURE MEANS: different reader adapters can encode the same logical tag incompatibly. */
    TEST_IGNORE_MESSAGE("Requires selected reader model and independently decoded captured frame.");
}
