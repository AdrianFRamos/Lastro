from __future__ import annotations

import http.client
import json
import threading
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path

from tests.system.e2e_controller import ControllerState, handler_for
from tests.system.support import SystemEnvironment


class _StationStub:
    public_key = bytes([2]) + bytes(32)

    def __init__(self) -> None:
        self.observations: list[tuple[int, str]] = []

    def queue_observation(self, action: int, rfid_hex: str) -> None:
        self.observations.append((action, rfid_hex))

    def wait_ack(self, _capture_id: str) -> None:
        return


class _UpstreamState:
    def __init__(self) -> None:
        self.posts: list[bytes] = []


def _start_server(handler: type[BaseHTTPRequestHandler]) -> tuple[ThreadingHTTPServer, threading.Thread, str]:
    server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
    host, port = server.server_address[:2]
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server, thread, f"http://{host}:{port}"


def _stop_server(server: ThreadingHTTPServer, thread: threading.Thread) -> None:
    server.shutdown()
    server.server_close()
    thread.join(timeout=2)


def _environment(api_url: str) -> SystemEnvironment:
    return SystemEnvironment(
        api_url=api_url,
        rpc_url="http://127.0.0.1:8899",
        program_id="11111111111111111111111111111111",
        deployment_id=bytes(32),
        agent_bin=Path("/nonexistent-agent"),
        agent_token="test-token",
        station_scalar=1,
        wallet_a=Path("/nonexistent-a"),
        wallet_b=Path("/nonexistent-b"),
        wallet_c=Path("/nonexistent-c"),
    )


def test_evidence_response_fault_happens_only_after_upstream_acceptance_and_only_once():
    """
    PURPOSE: prove the E2E fault injector models an ambiguous delivery outcome rather than dropping the request before API acceptance.
    ARRANGE: run a real local upstream HTTP server and arm exactly one Agent evidence-response drop in the controller proxy.
    ACTION: POST identical evidence twice; the first upstream response is cut off and the retry receives the normal accepted response.
    ASSERT: upstream receives both bodies, exactly one client response is dropped, and metrics count both Agent POST attempts.
    FAILURE MEANS: the retry E2E test could pass with a fault model that never exercised API idempotency after acceptance.
    """
    upstream = _UpstreamState()

    class UpstreamHandler(BaseHTTPRequestHandler):
        def log_message(self, _format: str, *_args: object) -> None:
            return

        def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler contract
            length = int(self.headers.get("content-length", "0"))
            upstream.posts.append(self.rfile.read(length))
            payload = b'{"accepted":true}'
            self.send_response(200)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

    upstream_server, upstream_thread, upstream_url = _start_server(UpstreamHandler)
    station = _StationStub()
    state = ControllerState(_environment(upstream_url), station)  # type: ignore[arg-type]
    controller_server, controller_thread, controller_url = _start_server(handler_for(state))
    try:
        state.arm_drop_next_evidence_response()
        body = json.dumps({"captureId": "capture-1"}).encode()
        request = urllib.request.Request(
            f"{controller_url}/api/agent/evidence",
            data=body,
            method="POST",
            headers={"content-type": "application/json"},
        )
        try:
            urllib.request.urlopen(request, timeout=2).read()
            raise AssertionError("faulted Agent response unexpectedly reached the client")
        except (http.client.RemoteDisconnected, ConnectionResetError, urllib.error.URLError):
            pass

        retry = urllib.request.Request(
            f"{controller_url}/api/agent/evidence",
            data=body,
            method="POST",
            headers={"content-type": "application/json"},
        )
        with urllib.request.urlopen(retry, timeout=2) as response:
            assert response.status == 200
            assert json.loads(response.read()) == {"accepted": True}

        assert upstream.posts == [body, body]
        assert state.metrics() == {
            "agentEvidencePosts": 2,
            "droppedEvidenceResponses": 1,
            "armedEvidenceResponseDrops": 0,
        }
    finally:
        _stop_server(controller_server, controller_thread)
        _stop_server(upstream_server, upstream_thread)


def test_observation_control_endpoint_maps_only_the_three_protocol_actions():
    """
    PURPOSE: prove browser E2E control can queue only ORIGIN/TRANSFER/REIDENTIFY physical observations and preserves the exact RFID bytes.
    ARRANGE: start the controller with a Station stub and no need for an upstream API request.
    ACTION: submit one valid REIDENTIFY observation and one unsupported action.
    ASSERT: action code 3 and the exact RFID reach the Station once; unsupported action returns HTTP 400 and queues nothing else.
    FAILURE MEANS: E2E infrastructure could inject an operation outside the frozen Lastro domain protocol.
    """
    station = _StationStub()
    state = ControllerState(_environment("http://127.0.0.1:1"), station)  # type: ignore[arg-type]
    server, thread, url = _start_server(handler_for(state))
    try:
        valid_body = json.dumps({"action": "REIDENTIFY", "rfidHex": "0102030405060708"}).encode()
        request = urllib.request.Request(
            f"{url}/__lastro_e2e/observe",
            data=valid_body,
            method="POST",
            headers={"content-type": "application/json"},
        )
        with urllib.request.urlopen(request, timeout=2) as response:
            assert response.status == 200

        invalid_body = json.dumps({"action": "DELETE", "rfidHex": "0102030405060708"}).encode()
        invalid = urllib.request.Request(
            f"{url}/__lastro_e2e/observe",
            data=invalid_body,
            method="POST",
            headers={"content-type": "application/json"},
        )
        try:
            urllib.request.urlopen(invalid, timeout=2)
            raise AssertionError("unsupported action unexpectedly succeeded")
        except urllib.error.HTTPError as error:
            assert error.code == 400

        assert station.observations == [(3, "0102030405060708")]
    finally:
        _stop_server(server, thread)
