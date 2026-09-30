"""Deterministic check of the host protocol used by the hardware gates (no hardware needed)."""
from __future__ import annotations

import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from host_protocol import BindCommand, encode_frame, event_hash, take_frames  # noqa: E402

VECTORS = Path(__file__).resolve().parents[3] / "test-vectors" / "v2-capture.json"


def test_host_protocol_reproduces_the_frozen_bind_vector():
    """
    PURPOSE: the hardware gates must judge the Station with a correct, independent host codec.
    ARRANGE: the frozen bind COMMAND/EVENT_READY vectors from test-vectors/v2-capture.json.
    ACTION: rebuild the command, the expected envelope and the event hash; round-trip a frame.
    ASSERT: every byte equals the vector.
    FAILURE MEANS: a real Station could be failed (or passed) because of a host-side bug.
    """
    vectors = json.loads(VECTORS.read_text(encoding="utf-8"))
    bind = vectors["captures"]["bind"]
    raw = bytes.fromhex(bind["command_hex"])
    observed_at, expires_at = struct.unpack_from("<qq", raw, 186)
    command = BindCommand(raw[0:16], raw[18:50], raw[50:82], observed_at, expires_at)

    assert command.encode() == raw
    envelope = command.expected_envelope(
        bytes.fromhex(vectors["station"]["pubkey33_hex"]), bytes.fromhex(bind["observed_rfid_hex"])
    )
    assert envelope.hex() == bind["envelope_hex"]
    assert event_hash(envelope).hex() == bind["event_hash_hex"]

    ready = bytes.fromhex(bind["event_ready_hex"])
    buffer = bytearray(b"noise" + encode_frame(2, ready))
    assert take_frames(buffer) == [(2, ready)]
