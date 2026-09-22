from __future__ import annotations

import json
import os
from pathlib import Path
import socket
import struct
import subprocess
import sys
import time
from urllib.request import Request, urlopen

from lastro_hardware_simulator.protocol import (
    MESSAGE_ACK,
    MESSAGE_COMMAND,
    MESSAGE_EVENT_READY,
    FrameDecoder,
    encode_frame,
    event_hash,
)

ROOT = Path(__file__).resolve().parents[1]
SRC = ROOT / "src"
CAPTURE_ID = bytes.fromhex("00112233445566778899aabbccddeeff")
RFID_A_HEX = "8000130000000001"


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


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


def request_json(url: str, method: str = "GET", body: dict[str, object] | None = None):
    data = None if body is None else json.dumps(body).encode("utf-8")
    request = Request(url, data=data, method=method)
    if data is not None:
        request.add_header("content-type", "application/json")
    with urlopen(request, timeout=2) as response:
        return json.loads(response.read())


def wait_for_health(base_url: str) -> None:
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        try:
            if request_json(f"{base_url}/healthz")["status"] == "ok":
                return
        except OSError:
            time.sleep(0.05)
    raise AssertionError("simulator HTTP server did not become healthy")


def test_real_http_and_wire_servers_complete_one_origin_station_capture():
    http_port = free_port()
    wire_port = free_port()
    base_url = f"http://127.0.0.1:{http_port}"
    env = os.environ.copy()
    env.update(
        {
            "PYTHONPATH": str(SRC),
            "LASTRO_HARDWARE_SIM_HTTP_BIND": f"127.0.0.1:{http_port}",
            "LASTRO_HARDWARE_SIM_WIRE_BIND": f"127.0.0.1:{wire_port}",
        }
    )
    process = subprocess.Popen(
        [sys.executable, "-m", "lastro_hardware_simulator.server"],
        env=env,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    wire: socket.socket | None = None
    try:
        wait_for_health(base_url)
        with urlopen(f"{base_url}/", timeout=2) as response:
            html = response.read().decode("utf-8")
            assert "Lastro Hardware Simulator" in html
            assert response.headers["X-Frame-Options"] == "DENY"

        wire = socket.create_connection(("127.0.0.1", wire_port), timeout=2)
        wire.sendall(encode_frame(MESSAGE_COMMAND, origin_command_payload()))

        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            snapshot = request_json(f"{base_url}/api/state")
            if snapshot["station"]["state"] == "WAIT_RFID":
                break
            time.sleep(0.02)
        else:
            raise AssertionError("Station never entered WAIT_RFID")

        observed = request_json(
            f"{base_url}/api/observe",
            "POST",
            {"rfidHex": RFID_A_HEX, "label": "Fixture tag A"},
        )
        assert observed == {"accepted": True, "reason": "EVENT_READY"}

        wire.settimeout(2)
        decoder = FrameDecoder()
        frames = []
        while not frames:
            frames.extend(decoder.feed(wire.recv(1024)))
        ready = frames[0]
        assert ready.message_type == MESSAGE_EVENT_READY
        event = ready.payload[16:292]
        assert ready.payload[292:300].hex() == RFID_A_HEX

        ack = CAPTURE_ID + event_hash(event)
        wire.sendall(encode_frame(MESSAGE_ACK, ack))

        deadline = time.monotonic() + 2
        while time.monotonic() < deadline:
            snapshot = request_json(f"{base_url}/api/state")
            if snapshot["station"]["state"] == "IDLE":
                break
            time.sleep(0.02)
        else:
            raise AssertionError("matching ACK did not return Station to IDLE")
    finally:
        if wire is not None:
            wire.close()
        process.terminate()
        try:
            process.wait(timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=2)
        if process.returncode not in {0, -15, 1}:
            stdout, stderr = process.communicate()
            raise AssertionError(f"simulator exited {process.returncode}\nstdout={stdout}\nstderr={stderr}")
