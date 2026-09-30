from __future__ import annotations

import json
from pathlib import Path
import struct

from lastro_hardware_simulator.protocol import (
    MESSAGE_COMMAND,
    FrameDecoder,
    ProtocolError,
    crc32c,
    decode_command,
    derive_station_id,
    encode_capture_envelope,
    encode_frame,
    event_hash,
)

VECTORS = json.loads(
    (Path(__file__).resolve().parents[2] / "test-vectors" / "v2-capture.json").read_text()
)
STATION_PUBKEY = bytes.fromhex(VECTORS["station"]["pubkey33_hex"])
EXPECTED_STATION_ID = bytes.fromhex(VECTORS["station"]["station_id_hex"])
TAG_A = bytes.fromhex(VECTORS["tags"]["a"]["canonical_hex"])
TAG_B = bytes.fromhex(VECTORS["tags"]["b"]["canonical_hex"])


def capture(name: str) -> dict[str, str]:
    return VECTORS["captures"][name]


def origin_command_payload() -> bytes:
    return bytes.fromhex(capture("bind")["command_hex"])


def test_crc32c_matches_frozen_castagnoli_vector():
    assert crc32c(b"123456789") == 0xE3069283


def test_command_frame_round_trips_at_every_fragment_boundary():
    encoded = encode_frame(MESSAGE_COMMAND, origin_command_payload())
    for split in range(len(encoded) + 1):
        decoder = FrameDecoder()
        frames = decoder.feed(encoded[:split]) + decoder.feed(encoded[split:])
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


def test_station_id_is_derived_from_the_compressed_key():
    assert derive_station_id(STATION_PUBKEY) == EXPECTED_STATION_ID


def test_bind_replace_and_observe_envelopes_match_rust_vectors():
    """The simulated Station signs exactly the envelope the Rust protocol expects."""
    for name, tag in [("bind", TAG_A), ("replace", TAG_B), ("observe", TAG_B)]:
        vector = capture(name)
        command = decode_command(bytes.fromhex(vector["command_hex"]))
        envelope = encode_capture_envelope(command, EXPECTED_STATION_ID, tag)
        assert envelope.hex() == vector["envelope_hex"]
        assert event_hash(envelope).hex() == vector["event_hash_hex"]


def test_station_rules_reject_wrong_tags():
    replace = decode_command(bytes.fromhex(capture("replace")["command_hex"]))
    observe = decode_command(bytes.fromhex(capture("observe")["command_hex"]))
    for command, tag in [(replace, TAG_A), (observe, TAG_A)]:
        try:
            encode_capture_envelope(command, EXPECTED_STATION_ID, tag)
        except ProtocolError:
            continue
        raise AssertionError("Station must refuse a tag that violates the capture rule")


def test_command_with_forged_context_is_rejected():
    payload = bytearray(origin_command_payload())
    payload[154] = 1  # a first binding cannot expect an existing tag
    try:
        decode_command(bytes(payload))
    except ProtocolError:
        pass
    else:
        raise AssertionError("inconsistent command must be rejected")
    payload = bytearray(origin_command_payload())
    struct.pack_into("<H", payload, 16, 4)  # CUSTODY_TRANSFERRED is not a Station capture
    try:
        decode_command(bytes(payload))
    except ProtocolError:
        pass
    else:
        raise AssertionError("unknown capture event type must be rejected")
