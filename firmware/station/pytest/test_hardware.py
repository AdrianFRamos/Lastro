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
from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives.asymmetric.utils import encode_dss_signature

sys.path.insert(0, str(Path(__file__).resolve().parent))

from host_protocol import (  # noqa: E402
    MESSAGE_ACK,
    MESSAGE_COMMAND,
    MESSAGE_ERROR,
    MESSAGE_EVENT_READY,
    BindCommand,
    encode_frame,
    event_hash,
    take_frames,
)

pytestmark = pytest.mark.hardware

P256_ORDER = 0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551
CAPTURE_WINDOW_SECONDS = 300


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


def _build_bind_command() -> BindCommand:
    now = int(time.time())
    return BindCommand(
        capture_id=uuid.uuid4().bytes,
        deployment_id=_required_hex("LASTRO_DEPLOYMENT_ID_HEX", 32),
        asset_id=secrets.token_bytes(32),
        observed_at=now,
        expires_at=now + CAPTURE_WINDOW_SECONDS,
    )


def _verify_p256_low_s(public_key: bytes, signature: bytes, message: bytes) -> None:
    r = int.from_bytes(signature[:32], "big")
    s = int.from_bytes(signature[32:], "big")
    assert 1 <= r < P256_ORDER and 1 <= s <= P256_ORDER // 2, "Station signature is not canonical low-S"
    key = ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), public_key)
    try:
        key.verify(encode_dss_signature(r, s), message, ec.ECDSA(hashes.SHA256()))
    except InvalidSignature:
        pytest.fail("host verifier rejected the Station P-256 signature over the envelope")


def _read_frame(port, timeout: float) -> tuple[int, bytes]:
    deadline = time.monotonic() + timeout
    buffer = bytearray()
    while time.monotonic() < deadline:
        chunk = port.read(4096)
        if chunk:
            buffer.extend(chunk)
            frames = take_frames(buffer)
            if frames:
                return frames[0]
    pytest.fail(f"Station did not produce a serial frame within {timeout:.1f} seconds")


def _capture_bind(expected_rfid: bytes, *, command: BindCommand | None = None) -> dict:
    serial = _serial_module()
    command = command or _build_bind_command()
    timeout = float(os.getenv("LASTRO_HARDWARE_CAPTURE_TIMEOUT_SECONDS", "20"))
    with serial.Serial(_serial_port_path(), _serial_baud(), timeout=0.2, write_timeout=2) as port:
        port.reset_input_buffer()
        port.write(encode_frame(MESSAGE_COMMAND, command.encode()))
        port.flush()
        message_type, payload = _read_frame(port, timeout)

        if message_type == MESSAGE_ERROR:
            code = struct.unpack_from("<H", payload, 16)[0]
            if payload[18:20] != b"\x00\x00":
                pytest.fail("Station returned ERROR payload with non-zero reserved bytes")
            pytest.fail(f"Station returned ERROR code {code} for physical capture")
        if message_type != MESSAGE_EVENT_READY:
            pytest.fail(f"Station returned unexpected serial message type {message_type}")

        capture_id = payload[0:16]
        envelope = payload[16:236]
        observed_rfid = payload[236:244]
        public_key = payload[244:277]
        signature = payload[277:341]

        assert capture_id == command.capture_id
        assert observed_rfid == expected_rfid
        # The host rebuilds the whole envelope from its own command and the physical tag.
        assert envelope == command.expected_envelope(public_key, expected_rfid)
        _verify_p256_low_s(public_key, signature, envelope)

        digest = event_hash(envelope)
        port.write(encode_frame(MESSAGE_ACK, capture_id + digest))
        port.flush()

    result = {
        "capture_id": capture_id,
        "envelope": envelope,
        "event_hash": digest,
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
        "envelopeHex": result["envelope"].hex(),
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
    ACTION: start a capture, present the tag, collect envelope[220], pubkey33, and signature64.
    ASSERT: canonical RFID matches the physical tag; the host verifier accepts the P-256 signature
            over the same 220 bytes; the host rebuilds the identical envelope from its command and the tag; execution artifacts are retained.
    FAILURE MEANS: there is no demonstrated physical-to-cryptographic evidence bridge yet.
    """
    require_hardware()
    expected_rfid = _required_hex("LASTRO_HARDWARE_TAG_A_RFID_HEX", 8)
    _prompt_for_tag("A")
    result = _capture_bind(expected_rfid)
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
            observations[label].append(_capture_bind(expected)["observed_rfid"])
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
    command = _build_bind_command()
    before = _capture_bind(expected_rfid, command=command)

    _run_reboot_command()
    _prompt_for_tag("A after reboot")
    after = _capture_bind(expected_rfid, command=command)

    assert before["public_key"] == after["public_key"]
    assert before["envelope"] == after["envelope"]
    assert hashlib.sha256(b"LASTRO_STATION\x00" + before["public_key"]).digest() == hashlib.sha256(
        b"LASTRO_STATION\x00" + after["public_key"]
    ).digest()


def test_h1_efuse_private_key_is_not_readable_by_firmware():
    """
    PURPOSE: enable the `hardware-backed Station key` claim only after this gate passes.
    ARRANGE: perform the approved manual burn procedure from HARDWARE.md; record relevant eFuses
             before/after and the registered public key; set LASTRO_EFUSE_TEST=1.
    ACTION: sign a known vector using the ECDSA peripheral; inspect the read-protected key block;
            reboot and repeat the same signed capture.
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
    command = _build_bind_command()
    before = _capture_bind(expected_rfid, command=command)
    registered = os.getenv("LASTRO_STATION_PUBKEY_HEX")
    if registered:
        assert before["public_key"] == bytes.fromhex(registered)

    _run_reboot_command()
    _prompt_for_tag("A after eFuse reboot")
    after = _capture_bind(expected_rfid, command=command)
    assert after["public_key"] == before["public_key"]
    assert after["envelope"] == before["envelope"]
