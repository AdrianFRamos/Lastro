"""Real physical-hardware gates. Never run these tests without the declared hardware."""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import secrets
import shlex
import struct
import subprocess
import sys
import time
import uuid

import pytest

ROOT = Path(__file__).resolve().parents[3]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from tests.system.support import (
    MESSAGE_ACK,
    MESSAGE_COMMAND,
    MESSAGE_EVENT_READY,
    ZERO32,
    decode_station_event,
    encode_serial_frame,
    rfid_hash,
    take_serial_frames,
    verify_p256_compact_low_s,
)

pytestmark = pytest.mark.hardware

MESSAGE_ERROR = 4
COMMAND_PAYLOAD_LEN = 224
EVENT_READY_PAYLOAD_LEN = 397
ACK_PAYLOAD_LEN = 48


def require_hardware() -> None:
    if os.getenv("LASTRO_HARDWARE_TEST") != "1":
        pytest.skip("Set LASTRO_HARDWARE_TEST=1 only with the declared physical board and reader connected")


def _required_text(name: str) -> str:
    value = os.getenv(name)
    if value is None or not value.strip():
        pytest.fail(f"{name} is required for this hardware gate")
    return value.strip()


def _required_hex(name: str, length: int) -> bytes:
    text = _required_text(name)
    try:
        value = bytes.fromhex(text)
    except ValueError as error:
        pytest.fail(f"{name} must be hexadecimal: {error}")
    if len(value) != length:
        pytest.fail(f"{name} must decode to exactly {length} bytes")
    return value


def _serial_module():
    try:
        import serial  # type: ignore[import-not-found]
    except ImportError:
        pytest.fail("Install pyserial from requirements-dev.txt before running physical hardware gates")
    return serial


def _serial_port_path() -> str:
    return _required_text("LASTRO_AGENT_SERIAL_PORT")


def _serial_baud() -> int:
    text = os.getenv("LASTRO_AGENT_SERIAL_BAUD", "115200")
    try:
        value = int(text)
    except ValueError:
        pytest.fail("LASTRO_AGENT_SERIAL_BAUD must be an integer")
    if value <= 0:
        pytest.fail("LASTRO_AGENT_SERIAL_BAUD must be positive")
    return value


def _prompt_for_tag(label: str) -> None:
    if os.getenv("LASTRO_HARDWARE_INTERACTIVE") == "1":
        input(f"Place physical tag {label} in the reader field, remove other tags, then press Enter: ")


def _build_origin_command(*, capture_id: bytes | None = None, animal_id: bytes | None = None) -> tuple[bytes, dict[str, bytes | int]]:
    capture = capture_id or uuid.uuid4().bytes
    deployment = _required_hex("LASTRO_DEPLOYMENT_ID_HEX", 32)
    animal = animal_id or secrets.token_bytes(32)
    to_custodian = _required_hex("LASTRO_HARDWARE_CUSTODIAN_HEX", 32)

    payload = bytearray(COMMAND_PAYLOAD_LEN)
    payload[0:16] = capture
    payload[16] = 1  # ORIGIN
    payload[17:20] = b"\x00\x00\x00"
    payload[20:52] = deployment
    payload[52:84] = animal
    struct.pack_into("<Q", payload, 84, 1)
    struct.pack_into("<I", payload, 92, 1)
    payload[96:128] = ZERO32
    payload[128:160] = ZERO32
    payload[160:192] = ZERO32
    payload[192:224] = to_custodian
    return bytes(payload), {
        "capture_id": capture,
        "deployment_id": deployment,
        "animal_id": animal,
        "to_custodian": to_custodian,
    }


def _read_frame(port, timeout: float) -> tuple[int, bytes]:
    deadline = time.monotonic() + timeout
    buffer = bytearray()
    while time.monotonic() < deadline:
        chunk = port.read(4096)
        if chunk:
            buffer.extend(chunk)
            frames = list(take_serial_frames(buffer))
            if frames:
                return frames[0]
    pytest.fail(f"Station did not produce a serial frame within {timeout:.1f} seconds")


def _capture_origin(expected_rfid: bytes, *, command: bytes | None = None, expected: dict | None = None) -> dict:
    serial = _serial_module()
    command_payload, command_expected = (command, expected) if command is not None and expected is not None else _build_origin_command()
    assert command_payload is not None and command_expected is not None

    timeout = float(os.getenv("LASTRO_HARDWARE_CAPTURE_TIMEOUT_SECONDS", "20"))
    with serial.Serial(_serial_port_path(), _serial_baud(), timeout=0.2, write_timeout=2) as port:
        port.reset_input_buffer()
        port.write(encode_serial_frame(MESSAGE_COMMAND, command_payload))
        port.flush()
        message_type, payload = _read_frame(port, timeout)

        if message_type == MESSAGE_ERROR:
            if len(payload) != 20:
                pytest.fail(f"Station returned malformed ERROR payload of {len(payload)} bytes")
            code = struct.unpack_from("<H", payload, 16)[0]
            if payload[18:20] != b"\x00\x00":
                pytest.fail("Station returned ERROR payload with non-zero reserved bytes")
            pytest.fail(f"Station returned ERROR code {code} for physical capture")
        if message_type != MESSAGE_EVENT_READY:
            pytest.fail(f"Station returned unexpected serial message type {message_type}")
        if len(payload) != EVENT_READY_PAYLOAD_LEN:
            pytest.fail(f"EVENT_READY payload is {len(payload)} bytes, expected {EVENT_READY_PAYLOAD_LEN}")

        capture_id = payload[0:16]
        event_bytes = payload[16:292]
        observed_rfid = payload[292:300]
        public_key = payload[300:333]
        signature = payload[333:397]

        assert capture_id == command_expected["capture_id"]
        assert observed_rfid == expected_rfid
        event = decode_station_event(event_bytes)
        assert event["action"] == 1
        assert event["deployment_id"] == command_expected["deployment_id"]
        assert event["animal_id"] == command_expected["animal_id"]
        assert event["event_sequence"] == 1
        assert event["identity_revision"] == 1
        assert event["previous_event_hash"] == ZERO32
        assert event["old_rfid_hash"] == ZERO32
        assert event["new_rfid_hash"] == rfid_hash(expected_rfid)
        assert event["from_custodian"] == ZERO32
        assert event["to_custodian"] == command_expected["to_custodian"]
        assert event["station_id"] == hashlib.sha256(b"LASTRO_STATION\x00" + public_key).digest()
        verify_p256_compact_low_s(public_key, signature, event_bytes)

        event_hash = hashlib.sha256(event_bytes).digest()
        ack = capture_id + event_hash
        assert len(ack) == ACK_PAYLOAD_LEN
        port.write(encode_serial_frame(MESSAGE_ACK, ack))
        port.flush()

    result = {
        "capture_id": capture_id,
        "event_bytes": event_bytes,
        "event_hash": event_hash,
        "observed_rfid": observed_rfid,
        "public_key": public_key,
        "signature": signature,
    }
    _retain_capture_artifact(result)
    return result


def _retain_capture_artifact(result: dict) -> None:
    directory = os.getenv("LASTRO_HARDWARE_ARTIFACT_DIR")
    if not directory:
        return
    root = Path(directory).expanduser().resolve()
    root.mkdir(parents=True, exist_ok=True)
    capture_id = uuid.UUID(bytes=result["capture_id"])
    payload = {
        "captureId": str(capture_id),
        "stationEventHex": result["event_bytes"].hex(),
        "eventHashHex": result["event_hash"].hex(),
        "observedRfidHex": result["observed_rfid"].hex(),
        "stationPubkeyHex": result["public_key"].hex(),
        "stationSignatureHex": result["signature"].hex(),
    }
    (root / f"capture-{capture_id}.json").write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")


def _run_reboot_command() -> None:
    command = _required_text("LASTRO_HARDWARE_REBOOT_COMMAND")
    result = subprocess.run(shlex.split(command), check=False, text=True, capture_output=True, timeout=30)
    if result.returncode != 0:
        pytest.fail(f"reboot command failed ({result.returncode}): {result.stderr.strip()}")
    deadline = time.monotonic() + float(os.getenv("LASTRO_HARDWARE_REBOOT_WAIT_SECONDS", "8"))
    path = Path(_serial_port_path())
    while time.monotonic() < deadline:
        if path.exists():
            time.sleep(0.5)
            return
        time.sleep(0.2)
    pytest.fail(f"serial port did not return after reboot: {path}")


def test_g1_real_rfid_to_p256_to_host_verification():
    """
    PURPOSE: prove the physical G1 path end to end.
    ARRANGE: ESP32-C5 + documented reader + physical tag A + host connected over serial.
    ACTION: start a capture, present the tag, collect event[276], pubkey33, and signature64.
    ASSERT: canonical RFID matches the physical tag; the host verifier accepts the P-256 signature
            over the same 276 bytes; event hash and StationID match; execution artifacts are retained.
    FAILURE MEANS: there is no demonstrated physical-to-cryptographic evidence bridge yet.
    """
    require_hardware()
    expected_rfid = _required_hex("LASTRO_HARDWARE_TAG_A_RFID_HEX", 8)
    _prompt_for_tag("A")
    result = _capture_origin(expected_rfid)
    assert result["observed_rfid"] == expected_rfid


def test_reader_two_physical_tags_produce_distinct_canonical_ids():
    """
    PURPOSE: prove the real reader adapter deterministically produces the logical FDX-B identifier
             used as `canonical_rfid[8]`.
    ARRANGE: reader/antenna approved in HARDWARE.md; two distinct physical tags A/B; perform at
             least three independent reads of each tag.
    ACTION: capture the reader's raw frame and the canonical_rfid produced by firmware.
    ASSERT: every read of A produces the same 8 bytes; every read of B produces the same 8 bytes;
            A != B; vendor framing/ASCII/checksum bytes never enter the canonical 8 bytes. Retain
            raw frames and results as gate evidence.
    FAILURE MEANS: the reader adapter cannot support stable physical evidence.
    """
    require_hardware()
    if os.getenv("LASTRO_HARDWARE_INTERACTIVE") != "1":
        pytest.skip("Set LASTRO_HARDWARE_INTERACTIVE=1 to perform the required physical A/B tag swaps")

    tag_a = _required_hex("LASTRO_HARDWARE_TAG_A_RFID_HEX", 8)
    tag_b = _required_hex("LASTRO_HARDWARE_TAG_B_RFID_HEX", 8)
    assert tag_a != tag_b

    observations: dict[str, list[bytes]] = {"A": [], "B": []}
    for label, expected in (("A", tag_a), ("B", tag_b)):
        for read_number in range(1, 4):
            input(f"Present tag {label} for physical read {read_number}/3, then press Enter: ")
            observations[label].append(_capture_origin(expected)["observed_rfid"])
        raw_path = Path(_required_text(f"LASTRO_HARDWARE_TAG_{label}_RAW_FRAME_FILE")).expanduser().resolve()
        assert raw_path.is_file() and raw_path.stat().st_size > 0, f"missing retained raw reader frame for tag {label}"

    assert observations["A"] == [tag_a] * 3
    assert observations["B"] == [tag_b] * 3
    assert observations["A"][0] != observations["B"][0]


def test_reboot_preserves_station_public_key():
    """
    PURPOSE: prove normal reboot does not change the Station cryptographic identity.
    ARRANGE: provisioned Station using the key selected for the gate; host can obtain or derive the
             compressed 33-byte public key before and after reboot.
    ACTION: read the public key; reboot without reprovisioning; read it again; sign the same known
            vector after reboot.
    ASSERT: pubkey_before == pubkey_after; derived StationID is also identical; the post-reboot
            signature verifies with the same deployment-registered public key.
    FAILURE MEANS: the deployment does not have a persistent Station identity.
    """
    require_hardware()
    expected_rfid = _required_hex("LASTRO_HARDWARE_TAG_A_RFID_HEX", 8)
    _prompt_for_tag("A")
    command, expected = _build_origin_command()
    before = _capture_origin(expected_rfid, command=command, expected=expected)

    _run_reboot_command()
    _prompt_for_tag("A after reboot")
    after = _capture_origin(expected_rfid, command=command, expected=expected)

    assert before["public_key"] == after["public_key"]
    assert before["event_bytes"] == after["event_bytes"]
    assert hashlib.sha256(b"LASTRO_STATION\x00" + before["public_key"]).digest() == hashlib.sha256(
        b"LASTRO_STATION\x00" + after["public_key"]
    ).digest()


def test_h1_efuse_private_key_is_not_readable_by_firmware():
    """
    PURPOSE: enable the `hardware-backed Station key` claim only after this gate passes.
    ARRANGE: perform the approved manual burn procedure from HARDWARE.md; record relevant eFuses
             before/after and the registered public key; set LASTRO_EFUSE_TEST=1.
    ACTION: sign a known vector using the ECDSA peripheral; inspect the read-protected key block;
            reboot and repeat the same signed StationEvent capture.
    ASSERT: signatures verify with the same registered public key; the configured eFuse block is
            reported non-readable with ECDSA purpose; reboot preserves identity.
    FAILURE MEANS: G1 may remain valid, but the `hardware-backed` claim is prohibited.
    SAFETY: this test never performs an eFuse burn automatically and never modifies eFuse state.
    """
    if os.getenv("LASTRO_EFUSE_TEST") != "1":
        pytest.skip("H1 is deliberately opt-in")
    require_hardware()

    summary_path = Path(_required_text("LASTRO_EFUSE_SUMMARY_JSON")).expanduser().resolve()
    block_field = _required_text("LASTRO_EFUSE_BLOCK_FIELD")
    purpose_field = _required_text("LASTRO_EFUSE_PURPOSE_FIELD")
    try:
        summary = json.loads(summary_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        pytest.fail(f"cannot read espefuse JSON summary {summary_path}: {error}")

    block = summary.get(block_field)
    purpose = summary.get(purpose_field)
    assert isinstance(block, dict), f"eFuse summary does not contain {block_field}"
    assert isinstance(purpose, dict), f"eFuse summary does not contain {purpose_field}"
    assert block.get("readable") is False, f"{block_field} is still software-readable"
    assert "ECDSA" in str(purpose.get("value", "")).upper(), f"{purpose_field} is not an ECDSA key purpose"

    expected_rfid = _required_hex("LASTRO_HARDWARE_TAG_A_RFID_HEX", 8)
    _prompt_for_tag("A")
    command, expected = _build_origin_command()
    before = _capture_origin(expected_rfid, command=command, expected=expected)
    registered = os.getenv("LASTRO_STATION_PUBKEY_HEX")
    if registered:
        assert before["public_key"] == bytes.fromhex(registered)

    _run_reboot_command()
    _prompt_for_tag("A after eFuse reboot")
    after = _capture_origin(expected_rfid, command=command, expected=expected)
    assert after["public_key"] == before["public_key"]
    assert after["event_bytes"] == before["event_bytes"]
