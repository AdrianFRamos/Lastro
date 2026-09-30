from __future__ import annotations

import json
import mimetypes
import os
from pathlib import Path
import signal
import socket
import socketserver
from http import HTTPStatus
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Event, RLock, Thread
from typing import Any
from urllib.parse import urlparse

from .protocol import FrameDecoder, ProtocolError
from .station import SimulatedStation, configured_private_scalar

WEB_ROOT = Path(__file__).with_name("web").resolve()
MAX_REQUEST_BYTES = 16 * 1024


class WireHub:
    def __init__(self) -> None:
        self._lock = RLock()
        self._client: socket.socket | None = None

    def connected(self) -> bool:
        with self._lock:
            return self._client is not None

    def register(self, client: socket.socket) -> bool:
        with self._lock:
            if self._client is not None:
                return False
            self._client = client
            return True

    def unregister(self, client: socket.socket) -> None:
        with self._lock:
            if self._client is client:
                self._client = None

    def send(self, payload: bytes) -> bool:
        with self._lock:
            if self._client is None:
                return False
            try:
                self._client.sendall(payload)
                return True
            except OSError:
                try:
                    self._client.close()
                finally:
                    self._client = None
                return False

    def disconnect(self) -> bool:
        with self._lock:
            if self._client is None:
                return False
            try:
                self._client.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            self._client.close()
            self._client = None
            return True


class SimulatorContext:
    def __init__(self) -> None:
        self.wire = WireHub()
        self.station = SimulatedStation(self.wire.send, configured_private_scalar())

    def snapshot(self) -> dict[str, object]:
        return self.station.snapshot(self.wire.connected())


CONTEXT = SimulatorContext()


class WireHandler(socketserver.BaseRequestHandler):
    def handle(self) -> None:
        client = self.request
        assert isinstance(client, socket.socket)
        client.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        peer = f"{self.client_address[0]}:{self.client_address[1]}"
        if not CONTEXT.wire.register(client):
            CONTEXT.station.log_transport("Rejected second Agent wire connection", peer)
            return

        CONTEXT.station.log_transport("Agent wire connected", peer)
        decoder = FrameDecoder()
        try:
            while True:
                chunk = client.recv(4096)
                if not chunk:
                    return
                pending = chunk
                while True:
                    try:
                        frames = decoder.feed(pending)
                        pending = b""
                    except ProtocolError as error:
                        CONTEXT.station.log_transport("Rejected malformed LSTR frame", str(error))
                        # The decoder discards one corrupt prefix byte before raising. Drain
                        # any already-buffered bytes immediately so a valid following frame
                        # does not wait for another TCP packet before it can be recovered.
                        pending = b""
                        continue
                    for frame in frames:
                        CONTEXT.station.handle_frame(frame)
                    break
        except OSError as error:
            CONTEXT.station.log_transport("Agent wire closed", str(error))
        finally:
            CONTEXT.wire.unregister(client)
            CONTEXT.station.log_transport("Agent wire disconnected", peer)


class WireServer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True


class SimulatorHttpHandler(BaseHTTPRequestHandler):
    server_version = "LastroHardwareSimulator/1"

    def do_GET(self) -> None:  # noqa: N802
        parsed = urlparse(self.path)
        if parsed.path == "/healthz":
            self._json({"status": "ok", "wireConnected": CONTEXT.wire.connected()})
            return
        if parsed.path == "/api/state":
            self._json(CONTEXT.snapshot())
            return
        self._serve_static(parsed.path)

    def do_POST(self) -> None:  # noqa: N802
        parsed = urlparse(self.path)
        try:
            body = self._read_json()
            if parsed.path == "/api/observe":
                result = CONTEXT.station.observe_rfid_hex(
                    str(body.get("rfidHex", "")),
                    str(body.get("label", "")),
                )
                self._json(result)
                return
            if parsed.path == "/api/faults":
                name = str(body.get("name", ""))
                enabled = bool(body.get("enabled", True))
                CONTEXT.station.set_fault(name, enabled)
                self._json({"ok": True})
                return
            if parsed.path == "/api/reset":
                CONTEXT.station.reset()
                self._json({"ok": True})
                return
            if parsed.path == "/api/disconnect":
                disconnected = CONTEXT.wire.disconnect()
                CONTEXT.station.log_transport("Operator requested Agent wire disconnect")
                self._json({"ok": True, "disconnected": disconnected})
                return
            self._json({"error": "not found"}, HTTPStatus.NOT_FOUND)
        except (ProtocolError, ValueError, TypeError) as error:
            self._json({"error": str(error)}, HTTPStatus.BAD_REQUEST)
        except json.JSONDecodeError:
            self._json({"error": "request body must be valid JSON"}, HTTPStatus.BAD_REQUEST)

    def _read_json(self) -> dict[str, Any]:
        raw_length = self.headers.get("Content-Length")
        if raw_length is None:
            return {}
        try:
            length = int(raw_length)
        except ValueError as error:
            raise ValueError("invalid Content-Length") from error
        if length < 0 or length > MAX_REQUEST_BYTES:
            raise ValueError(f"request body exceeds {MAX_REQUEST_BYTES} bytes")
        data = self.rfile.read(length)
        if not data:
            return {}
        decoded = json.loads(data.decode("utf-8"))
        if not isinstance(decoded, dict):
            raise ValueError("request body must be a JSON object")
        return decoded

    def _serve_static(self, path: str) -> None:
        requested = "index.html" if path in {"", "/"} else path.lstrip("/")
        candidate = (WEB_ROOT / requested).resolve()
        if WEB_ROOT not in candidate.parents and candidate != WEB_ROOT:
            self.send_error(HTTPStatus.NOT_FOUND)
            return
        if not candidate.is_file():
            self.send_error(HTTPStatus.NOT_FOUND)
            return
        content = candidate.read_bytes()
        mime, _ = mimetypes.guess_type(candidate.name)
        self.send_response(HTTPStatus.OK)
        self.send_header("Content-Type", mime or "application/octet-stream")
        self.send_header("Content-Length", str(len(content)))
        self.send_header("Cache-Control", "no-store")
        self._security_headers()
        self.end_headers()
        self.wfile.write(content)

    def _json(self, value: object, status: HTTPStatus = HTTPStatus.OK) -> None:
        content = json.dumps(value, separators=(",", ":")).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json; charset=utf-8")
        self.send_header("Content-Length", str(len(content)))
        self.send_header("Cache-Control", "no-store")
        self._security_headers()
        self.end_headers()
        self.wfile.write(content)

    def _security_headers(self) -> None:
        self.send_header("X-Content-Type-Options", "nosniff")
        self.send_header("X-Frame-Options", "DENY")
        self.send_header("Referrer-Policy", "no-referrer")
        self.send_header(
            "Content-Security-Policy",
            "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'",
        )

    def log_message(self, format: str, *args: object) -> None:
        return


def parse_bind(value: str, name: str) -> tuple[str, int]:
    host, separator, raw_port = value.rpartition(":")
    if not separator or not host:
        raise ValueError(f"{name} must use host:port syntax")
    try:
        port = int(raw_port)
    except ValueError as error:
        raise ValueError(f"{name} port must be an integer") from error
    if port < 1 or port > 65535:
        raise ValueError(f"{name} port must be between 1 and 65535")
    return host, port


def main() -> None:
    http_bind = parse_bind(
        os.environ.get("LASTRO_HARDWARE_SIM_HTTP_BIND", "0.0.0.0:8090"),
        "LASTRO_HARDWARE_SIM_HTTP_BIND",
    )
    wire_bind = parse_bind(
        os.environ.get("LASTRO_HARDWARE_SIM_WIRE_BIND", "0.0.0.0:9100"),
        "LASTRO_HARDWARE_SIM_WIRE_BIND",
    )

    http_server = ThreadingHTTPServer(http_bind, SimulatorHttpHandler)
    wire_server = WireServer(wire_bind, WireHandler)
    stop = Event()

    def request_stop(_signum: int, _frame: object) -> None:
        stop.set()
        CONTEXT.wire.disconnect()
        Thread(target=http_server.shutdown, daemon=True).start()
        Thread(target=wire_server.shutdown, daemon=True).start()

    signal.signal(signal.SIGTERM, request_stop)
    signal.signal(signal.SIGINT, request_stop)

    wire_thread = Thread(target=wire_server.serve_forever, name="lastro-wire", daemon=True)
    wire_thread.start()
    CONTEXT.station.log_transport(
        "Hardware simulator listening",
        f"HTTP {http_bind[0]}:{http_bind[1]} · wire {wire_bind[0]}:{wire_bind[1]}",
    )

    try:
        http_server.serve_forever()
    finally:
        if not stop.is_set():
            wire_server.shutdown()
        http_server.server_close()
        wire_server.server_close()


if __name__ == "__main__":
    main()
