from __future__ import annotations

import json
from pathlib import Path
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

RFID_A_HEX = "8000130000000001"

VECTORS = json.loads(
    (Path(__file__).resolve().parents[2] / "test-vectors" / "v2-capture.json").read_text()
)
BIND = VECTORS["captures"]["bind"]
CAPTURE_ID = bytes.fromhex("01" * 16)


def origin_command_payload() -> bytes:
    """v2 IDENTIFIER_BOUND command from the cross-language vectors."""
    return bytes.fromhex(BIND["command_hex"])

EXPECTED_EVENT = bytes.fromhex(BIND["envelope_hex"])


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
    event = payload[16:236]
    observed_rfid = payload[236:244]
    pubkey = payload[244:277]
    signature = payload[277:341]
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
