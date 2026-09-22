from __future__ import annotations

import struct

from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives.asymmetric.utils import encode_dss_signature

from lastro_hardware_simulator.protocol import (
    MESSAGE_ACK,
    MESSAGE_COMMAND,
    MESSAGE_ERROR,
    MESSAGE_EVENT_READY,
    Frame,
    FrameDecoder,
    event_hash,
)
from lastro_hardware_simulator.station import P256_ORDER, SimulatedStation

CAPTURE_ID = bytes.fromhex("00112233445566778899aabbccddeeff")
RFID_A_HEX = "8000130000000001"
EXPECTED_EVENT = bytes.fromhex(
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
    out[0:16] = CAPTURE_ID
    out[16] = 1
    out[20:52] = bytes.fromhex("d0" * 32)
    out[52:84] = bytes.fromhex("11" * 32)
    out[84:92] = struct.pack("<Q", 1)
    out[92:96] = struct.pack("<I", 1)
    out[192:224] = bytes.fromhex("a1" * 32)
    return bytes(out)


def decode_emitted(raw: bytes):
    frames = FrameDecoder().feed(raw)
    assert len(frames) == 1
    return frames[0]


def test_station_origin_flow_emits_valid_low_s_event_and_accepts_matching_ack():
    emitted: list[bytes] = []
    station = SimulatedStation(lambda raw: emitted.append(raw) or True)
    station.handle_frame(Frame(MESSAGE_COMMAND, origin_command_payload()))
    assert station.snapshot(True)["station"]["state"] == "WAIT_RFID"

    result = station.observe_rfid_hex(RFID_A_HEX, "Fixture tag A")
    assert result == {"accepted": True, "reason": "EVENT_READY"}
    ready = decode_emitted(emitted.pop())
    assert ready.message_type == MESSAGE_EVENT_READY

    payload = ready.payload
    event = payload[16:292]
    observed_rfid = payload[292:300]
    pubkey = payload[300:333]
    signature = payload[333:397]
    assert event == EXPECTED_EVENT
    assert observed_rfid.hex() == RFID_A_HEX
    assert pubkey.hex() == station.public_key_hex

    r = int.from_bytes(signature[:32], "big")
    s = int.from_bytes(signature[32:], "big")
    assert 0 < r < P256_ORDER
    assert 0 < s <= P256_ORDER // 2
    ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), pubkey).verify(
        encode_dss_signature(r, s), event, ec.ECDSA(hashes.SHA256())
    )

    ack = CAPTURE_ID + event_hash(event)
    station.handle_frame(Frame(MESSAGE_ACK, ack))
    assert station.snapshot(True)["station"]["state"] == "IDLE"


def test_observation_without_active_capture_is_ignored():
    emitted: list[bytes] = []
    station = SimulatedStation(lambda raw: emitted.append(raw) or True)
    result = station.observe_rfid_hex(RFID_A_HEX)
    assert result == {"accepted": False, "reason": "NO_ACTIVE_CAPTURE"}
    assert emitted == []


def test_fault_injection_reader_failure_fails_closed_and_returns_to_idle():
    emitted: list[bytes] = []
    station = SimulatedStation(lambda raw: emitted.append(raw) or True)
    station.handle_frame(Frame(MESSAGE_COMMAND, origin_command_payload()))
    station.set_fault("reader_failure_next_observation")
    result = station.observe_rfid_hex(RFID_A_HEX)
    assert result == {"accepted": False, "reason": "RFID_READ_FAILED"}
    error = decode_emitted(emitted.pop())
    assert error.message_type == MESSAGE_ERROR
    assert error.payload[:16] == CAPTURE_ID
    assert int.from_bytes(error.payload[16:18], "little") == 2
    assert station.snapshot(True)["station"]["state"] == "IDLE"


def test_corrupt_next_event_ready_crc_is_one_shot():
    emitted: list[bytes] = []
    station = SimulatedStation(lambda raw: emitted.append(raw) or True)
    station.handle_frame(Frame(MESSAGE_COMMAND, origin_command_payload()))
    station.set_fault("corrupt_next_event_ready_crc")
    station.observe_rfid_hex(RFID_A_HEX)
    raw = emitted.pop()
    decoder = FrameDecoder()
    try:
        decoder.feed(raw)
    except Exception as error:
        assert "CRC32C" in str(error)
    else:
        raise AssertionError("corrupted frame must not decode")


def test_default_signer_matches_frozen_repository_station_identity():
    emitted: list[bytes] = []
    station = SimulatedStation(lambda raw: emitted.append(raw) or True)
    snapshot = station.snapshot(False)
    assert snapshot["station"]["publicKeyHex"] == (
        "036b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
    )
    assert snapshot["station"]["stationIdHex"] == (
        "56c266d8ab41a37e7a3bb80b33f6249c05bdb965c68bbeb4775303c69594df77"
    )


def test_identical_command_replay_matches_firmware_wait_states():
    emitted: list[bytes] = []
    station = SimulatedStation(lambda raw: emitted.append(raw) or True)
    command = Frame(MESSAGE_COMMAND, origin_command_payload())

    station.handle_frame(command)
    station.handle_frame(command)
    assert emitted == []
    assert station.snapshot(True)["station"]["state"] == "WAIT_RFID"

    station.observe_rfid_hex(RFID_A_HEX)
    first_ready = emitted.pop()
    station.handle_frame(command)
    replayed_ready = emitted.pop()
    assert replayed_ready == first_ready
    assert station.snapshot(True)["station"]["state"] == "WAIT_ACK"


def test_busy_fault_emits_busy_without_consuming_a_capture():
    emitted: list[bytes] = []
    station = SimulatedStation(lambda raw: emitted.append(raw) or True)
    station.set_fault("busy_next_command")
    station.handle_frame(Frame(MESSAGE_COMMAND, origin_command_payload()))
    error = decode_emitted(emitted.pop())
    assert error.message_type == MESSAGE_ERROR
    assert int.from_bytes(error.payload[16:18], "little") == 5
    assert station.snapshot(True)["station"]["state"] == "IDLE"


def test_signing_failure_emits_error_and_does_not_leave_unsigned_evidence_pending():
    emitted: list[bytes] = []
    station = SimulatedStation(lambda raw: emitted.append(raw) or True)
    station.handle_frame(Frame(MESSAGE_COMMAND, origin_command_payload()))
    station.set_fault("signing_failure_next_observation")
    result = station.observe_rfid_hex(RFID_A_HEX)
    assert result == {"accepted": False, "reason": "SIGNING_FAILED"}
    error = decode_emitted(emitted.pop())
    assert error.message_type == MESSAGE_ERROR
    assert int.from_bytes(error.payload[16:18], "little") == 4
    assert station.snapshot(True)["station"]["state"] == "IDLE"
