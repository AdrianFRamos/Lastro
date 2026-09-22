from __future__ import annotations

import struct

from lastro_hardware_simulator.protocol import (
    MESSAGE_COMMAND,
    FrameDecoder,
    ProtocolError,
    crc32c,
    decode_command,
    derive_station_id,
    encode_frame,
    encode_station_event,
)

DEPLOYMENT = bytes.fromhex("d0" * 32)
ANIMAL = bytes.fromhex("11" * 32)
CUSTODIAN_A = bytes.fromhex("a1" * 32)
RFID_A = bytes.fromhex("8000130000000001")
STATION_PUBKEY = bytes.fromhex(
    "036b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
)
EXPECTED_STATION_ID = bytes.fromhex(
    "56c266d8ab41a37e7a3bb80b33f6249c05bdb965c68bbeb4775303c69594df77"
)
EXPECTED_ORIGIN_EVENT = bytes.fromhex(
    "4c53545201010000"
    + "d0" * 32
    + "11" * 32
    + "56c266d8ab41a37e7a3bb80b33f6249c05bdb965c68bbeb4775303c69594df77"
    + "0100000000000000"
    + "01000000"
    + "00" * 32
    + "00" * 32
    + "8a604528f19062cc9ada87a5e225a5c967e2cf148c85d86daf02d04212ebf6e1"
    + "00" * 32
    + "a1" * 32
)


def origin_command_payload() -> bytes:
    out = bytearray(224)
    out[0:16] = bytes.fromhex("00112233445566778899aabbccddeeff")
    out[16] = 1
    out[20:52] = DEPLOYMENT
    out[52:84] = ANIMAL
    out[84:92] = struct.pack("<Q", 1)
    out[92:96] = struct.pack("<I", 1)
    out[192:224] = CUSTODIAN_A
    return bytes(out)


def test_crc32c_matches_frozen_castagnoli_vector():
    assert crc32c(b"123456789") == 0xE3069283


def test_command_frame_round_trips_at_every_fragment_boundary():
    encoded = encode_frame(MESSAGE_COMMAND, origin_command_payload())
    for split in range(len(encoded) + 1):
        decoder = FrameDecoder()
        first = decoder.feed(encoded[:split])
        second = decoder.feed(encoded[split:])
        frames = first + second
        assert len(frames) == 1
        assert frames[0].message_type == MESSAGE_COMMAND
        assert frames[0].payload == origin_command_payload()


def test_decoder_rejects_bad_crc_and_recovers_following_frame():
    valid = encode_frame(MESSAGE_COMMAND, origin_command_payload())
    bad = bytearray(valid)
    bad[30] ^= 1
    decoder = FrameDecoder()
    try:
        decoder.feed(bytes(bad) + valid)
    except ProtocolError as error:
        assert "CRC32C" in str(error)
    else:
        raise AssertionError("bad CRC must be rejected")
    frames = decoder.feed(b"")
    assert len(frames) == 1
    assert frames[0].payload == origin_command_payload()


def test_origin_station_event_matches_cross_language_fixture_bytes():
    command = decode_command(origin_command_payload())
    station_id = derive_station_id(STATION_PUBKEY)
    assert station_id == EXPECTED_STATION_ID
    assert encode_station_event(command, station_id, RFID_A) == EXPECTED_ORIGIN_EVENT


def transition_command_payload(*, action: int, sequence: int, revision: int, previous: str, old_rfid: str, from_hex: str, to_hex: str) -> bytes:
    out = bytearray(224)
    out[0:16] = bytes.fromhex("00112233445566778899aabbccddeeff")
    out[16] = action
    out[20:52] = DEPLOYMENT
    out[52:84] = ANIMAL
    out[84:92] = struct.pack("<Q", sequence)
    out[92:96] = struct.pack("<I", revision)
    out[96:128] = bytes.fromhex(previous)
    out[128:160] = bytes.fromhex(old_rfid)
    out[160:192] = bytes.fromhex(from_hex)
    out[192:224] = bytes.fromhex(to_hex)
    return bytes(out)


def test_transfer_station_event_matches_cross_language_fixture_bytes():
    command = decode_command(
        transition_command_payload(
            action=2,
            sequence=2,
            revision=1,
            previous="5845dc20fd6b266ec98399f0aa93c736ec9aa778bf038af5291e5df81334b531",
            old_rfid="8a604528f19062cc9ada87a5e225a5c967e2cf148c85d86daf02d04212ebf6e1",
            from_hex="a1" * 32,
            to_hex="b2" * 32,
        )
    )
    expected = bytes.fromhex(
        "4c53545201020000"
        + "d0" * 32
        + "11" * 32
        + EXPECTED_STATION_ID.hex()
        + "0200000000000000"
        + "01000000"
        + "5845dc20fd6b266ec98399f0aa93c736ec9aa778bf038af5291e5df81334b531"
        + "8a604528f19062cc9ada87a5e225a5c967e2cf148c85d86daf02d04212ebf6e1"
        + "8a604528f19062cc9ada87a5e225a5c967e2cf148c85d86daf02d04212ebf6e1"
        + "a1" * 32
        + "b2" * 32
    )
    assert encode_station_event(command, EXPECTED_STATION_ID, RFID_A) == expected


def test_reidentify_station_event_matches_cross_language_fixture_bytes():
    command = decode_command(
        transition_command_payload(
            action=3,
            sequence=3,
            revision=2,
            previous="a709b5d24fb31448ff34c1522368a0b5ad3dddfb3993964c1a7d509caa4c801e",
            old_rfid="8a604528f19062cc9ada87a5e225a5c967e2cf148c85d86daf02d04212ebf6e1",
            from_hex="b2" * 32,
            to_hex="b2" * 32,
        )
    )
    rfid_b = bytes.fromhex("8000130000000002")
    expected = bytes.fromhex(
        "4c53545201030000"
        + "d0" * 32
        + "11" * 32
        + EXPECTED_STATION_ID.hex()
        + "0300000000000000"
        + "02000000"
        + "a709b5d24fb31448ff34c1522368a0b5ad3dddfb3993964c1a7d509caa4c801e"
        + "8a604528f19062cc9ada87a5e225a5c967e2cf148c85d86daf02d04212ebf6e1"
        + "1c5bb72358bd6303c86a4969bef05eed30f9a6b3daac8ad4207bc27f4f40f978"
        + "b2" * 32
        + "b2" * 32
    )
    assert encode_station_event(command, EXPECTED_STATION_ID, rfid_b) == expected
