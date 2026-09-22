#include "unity.h"
#include "lastro_station/event.h"
#include "test_fixture.h"

static void assert_vector(lastro_station_event_fields_t fields, const char *fixture_hex)
{
    uint8_t actual[LASTRO_STATION_EVENT_LEN];
    uint8_t expected[LASTRO_STATION_EVENT_LEN];
    fixture_decode_hex(fixture_hex, expected, sizeof(expected));
    TEST_ASSERT_TRUE(lastro_event_encode(&fields, actual));
    TEST_ASSERT_EQUAL_UINT8_ARRAY(expected, actual, sizeof(actual));
}

TEST_CASE("c_origin_vector_equals_repository_fixture", "[lastro][contract]")
{
    /* PURPOSE: Preserve cross-language ORIGIN interoperability. */
    /* ARRANGE: Build the committed semantic ORIGIN vector in C. */
    /* ACTION: Encode StationEvent. */
    /* ASSERT: Bytes equal the repository fixture exactly. */
    /* FAILURE MEANS: C is no longer interoperable with Rust/TypeScript ORIGIN bytes. */
    assert_vector(fixture_origin_fields(), FIXTURE_ORIGIN_EVENT_HEX);
}

TEST_CASE("c_transfer_vector_equals_repository_fixture", "[lastro][contract]")
{
    /* PURPOSE: Preserve cross-language TRANSFER interoperability. */
    /* ARRANGE: Build the committed semantic TRANSFER vector in C. */
    /* ACTION: Encode StationEvent. */
    /* ASSERT: Bytes equal the repository fixture exactly. */
    /* FAILURE MEANS: C is no longer interoperable with Rust/TypeScript TRANSFER bytes. */
    assert_vector(fixture_transfer_fields(), FIXTURE_TRANSFER_EVENT_HEX);
}

TEST_CASE("c_reidentify_vector_equals_repository_fixture", "[lastro][contract]")
{
    /* PURPOSE: Preserve cross-language REIDENTIFY interoperability. */
    /* ARRANGE: Build the committed semantic REIDENTIFY vector in C. */
    /* ACTION: Encode StationEvent. */
    /* ASSERT: Bytes equal the repository fixture exactly. */
    /* FAILURE MEANS: C is no longer interoperable with Rust/TypeScript REIDENTIFY bytes. */
    assert_vector(fixture_reidentify_fields(), FIXTURE_REIDENTIFY_EVENT_HEX);
}
