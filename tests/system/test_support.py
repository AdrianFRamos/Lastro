"""Deterministic contracts of the v2 system harness itself (no stack required)."""
from __future__ import annotations

import base64
import copy
import json
import os
import time
from pathlib import Path

import pytest

from lastro_hardware_simulator.protocol import MESSAGE_COMMAND, MESSAGE_EVENT_READY, FrameDecoder, encode_frame
from tests.system.support import StationHarness, verify_package_locally

ROOT = Path(__file__).resolve().parents[2]
PACKAGE = json.loads((ROOT / "test-vectors/v2-evidence-package.valid.json").read_text(encoding="utf-8"))
VECTORS = json.loads((ROOT / "test-vectors/v2-capture.json").read_text(encoding="utf-8"))


def test_local_package_verifier_accepts_genuine_and_rejects_tampered_history():
    """
    PURPOSE: the harness verdict on exported packages must itself be trustworthy.
    ARRANGE: the frozen bind → replace → observe package and three tampered copies.
    ACTION: verify each with the independent Python verifier.
    ASSERT: the genuine package has no problems; a flipped envelope byte, a swapped observed RFID
            and a dropped middle event are each reported.
    FAILURE MEANS: G3/G4/G5 could report VALID for altered evidence.
    """
    assert verify_package_locally(PACKAGE) == []

    flipped = copy.deepcopy(PACKAGE)
    envelope = bytearray(base64.b64decode(flipped["events"][0]["envelopeBytesBase64"]))
    envelope[80] ^= 1
    flipped["events"][0]["envelopeBytesBase64"] = base64.b64encode(bytes(envelope)).decode()
    swapped = copy.deepcopy(PACKAGE)
    swapped["events"][0]["observedRfidHex"] = "8000130000000009"
    dropped = copy.deepcopy(PACKAGE)
    del dropped["events"][1]
    for tampered in (flipped, swapped, dropped):
        assert verify_package_locally(tampered) != []


@pytest.mark.skipif(os.name != "posix", reason="the PTY Station harness needs a POSIX host")
def test_pty_station_answers_the_frozen_command_with_the_frozen_envelope():
    """
    PURPOSE: the real Agent must see the same Station behavior through the PTY as over USB.
    ARRANGE: PTY Station with the test-only key; the frozen bind COMMAND.
    ACTION: write the COMMAND frame on the Agent side of the PTY and present the vector tag.
    ASSERT: the Station answers EVENT_READY whose envelope is byte-identical to the vector.
    FAILURE MEANS: the harness Station diverges from the firmware contract, invalidating G3.
    """
    bind = VECTORS["captures"]["bind"]
    with StationHarness(VECTORS["station"]["private_scalar_hex"]) as station:
        agent_side = os.open(station.path, os.O_RDWR | os.O_NOCTTY)
        try:
            os.write(agent_side, encode_frame(MESSAGE_COMMAND, bytes.fromhex(bind["command_hex"])))
            station.read_tag(bind["observed_rfid_hex"])
            decoder, frames, deadline = FrameDecoder(), [], time.monotonic() + 5
            while not frames and time.monotonic() < deadline:
                frames = decoder.feed(os.read(agent_side, 4096))
        finally:
            os.close(agent_side)
    assert frames and frames[0].message_type == MESSAGE_EVENT_READY
    assert frames[0].payload[16:236].hex() == bind["envelope_hex"]
