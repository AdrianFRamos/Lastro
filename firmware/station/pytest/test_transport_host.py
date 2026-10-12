"""Host build of the Station serial parser (no ESP-IDF or hardware needed)."""
from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
COMPONENT = HERE.parent / "components" / "lastro_station"


@pytest.mark.skipif(shutil.which("cc") is None, reason="no host C compiler")
def test_a_burst_of_frames_never_wedges_the_serial_parser(tmp_path):
    """
    PURPOSE: A4 — a burst of ACKs (the Agent replays one per stored row on reconnect) must not
    freeze the Station's serial input until reboot.
    ARRANGE: 24 ACK frames (1.3 KiB, more than the frame buffer) split into 512-byte USB reads.
    ACTION: feed them the way lastro_runtime_feed_agent does (feed_some, then take every frame).
    ASSERT: every frame comes out once, in order, and a later frame is still accepted.
    FAILURE MEANS: a reconnecting Agent could brick the Station's serial channel.
    """
    binary = tmp_path / "transport_burst"
    subprocess.run(
        [
            "cc", "-std=c11", "-Wall", "-Wextra", "-Werror",
            "-I", str(COMPONENT / "include"),
            str(COMPONENT / "transport.c"), str(HERE / "transport_burst_host.c"),
            "-o", str(binary),
        ],
        check=True,
    )
    result = subprocess.run([str(binary)], capture_output=True, text=True)
    assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == "ok 24 frames"
