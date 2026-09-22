"""Playwright full-stack bridge: real Agent + PTY Station harness with controllable network faults.

This process is test infrastructure only. It never signs Solana wallet transactions and never
updates API/database/chain state directly. Browser tests still use the production API, Wallet
Standard transaction path, and Solana RPC. The bridge controls only the physical Station
observation queue and the Agent's HTTP transport fault boundary.
"""

from __future__ import annotations

import json
import os
import signal
import socket
import tempfile
import threading
import urllib.error
import urllib.request
from dataclasses import replace
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from typing import Any

from .support import (
    AgentProcess,
    JsonHttpClient,
    StationHarness,
    SystemContractError,
    SystemEnvironment,
    WalletActor,
)

ACTION_CODES = {"ORIGIN": 1, "TRANSFER": 2, "REIDENTIFY": 3}


class ControllerState:
    def __init__(self, env: SystemEnvironment, station: StationHarness):
        self.env = env
        self.station = station
        self.lock = threading.Lock()
        self.drop_next_evidence_response = 0
        self.agent_evidence_posts = 0
        self.dropped_evidence_responses = 0

    def arm_drop_next_evidence_response(self) -> None:
        with self.lock:
            self.drop_next_evidence_response += 1

    def evidence_post_started(self) -> bool:
        with self.lock:
            self.agent_evidence_posts += 1
            if self.drop_next_evidence_response > 0:
                self.drop_next_evidence_response -= 1
                self.dropped_evidence_responses += 1
                return True
            return False

    def metrics(self) -> dict[str, int]:
        with self.lock:
            return {
                "agentEvidencePosts": self.agent_evidence_posts,
                "droppedEvidenceResponses": self.dropped_evidence_responses,
                "armedEvidenceResponseDrops": self.drop_next_evidence_response,
            }


def handler_for(state: ControllerState):
    class Handler(BaseHTTPRequestHandler):
        server_version = "LastroE2EController/1"

        def log_message(self, _format: str, *_args: object) -> None:
            return

        def do_GET(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler contract
            if self.path == "/__lastro_e2e/state":
                wallets = {
                    label: WalletActor.from_solana_keypair(path).address
                    for label, path in (
                        ("Wallet A", state.env.wallet_a),
                        ("Wallet B", state.env.wallet_b),
                        ("Wallet C", state.env.wallet_c),
                    )
                }
                self._json(200, {
                    "wallets": wallets,
                    "stationPubkeyHex": state.station.public_key.hex(),
                    "metrics": state.metrics(),
                })
                return
            if self.path == "/__lastro_e2e/metrics":
                self._json(200, state.metrics())
                return
            self._proxy("GET")

        def do_POST(self) -> None:  # noqa: N802 - BaseHTTPRequestHandler contract
            if self.path == "/__lastro_e2e/observe":
                body = self._json_body()
                action = body.get("action")
                rfid_hex = body.get("rfidHex")
                if action not in ACTION_CODES or not isinstance(rfid_hex, str):
                    self._json(400, {"error": "action and rfidHex are required"})
                    return
                try:
                    state.station.queue_observation(ACTION_CODES[action], rfid_hex)
                except Exception as error:  # control endpoint converts contract errors to test-visible JSON
                    self._json(400, {"error": str(error)})
                    return
                self._json(200, {"queued": True})
                return
            if self.path == "/__lastro_e2e/wait-ack":
                body = self._json_body()
                capture_id = body.get("captureId")
                if not isinstance(capture_id, str):
                    self._json(400, {"error": "captureId is required"})
                    return
                try:
                    state.station.wait_ack(capture_id)
                except Exception as error:
                    self._json(409, {"error": str(error)})
                    return
                self._json(200, {"acked": True})
                return
            if self.path == "/__lastro_e2e/fault/drop-next-evidence-response":
                state.arm_drop_next_evidence_response()
                self._json(200, {"armed": True})
                return
            self._proxy("POST")

        def _proxy(self, method: str) -> None:
            body = self._body()
            headers = {}
            for name in ("authorization", "accept", "content-type", "user-agent"):
                value = self.headers.get(name)
                if value:
                    headers[name] = value
            request = urllib.request.Request(
                f"{state.env.api_url}{self.path}",
                data=body if body else None,
                method=method,
                headers=headers,
            )
            try:
                with urllib.request.urlopen(request, timeout=15) as response:
                    status = response.status
                    payload = response.read()
                    content_type = response.headers.get("content-type", "application/json")
            except urllib.error.HTTPError as error:
                status = error.code
                payload = error.read()
                content_type = error.headers.get("content-type", "application/json")
            except OSError as error:
                self._json(502, {"error": f"upstream API request failed: {error}"})
                return

            drop = method == "POST" and self.path == "/api/agent/evidence" and state.evidence_post_started()
            if drop:
                # The upstream API has already accepted the request. Closing the Agent-facing
                # connection before any HTTP response simulates the ambiguous network outcome
                # that must be recovered by immutable retry/idempotency.
                self.close_connection = True
                try:
                    self.connection.shutdown(socket.SHUT_RDWR)
                except OSError:
                    pass
                try:
                    self.connection.close()
                except OSError:
                    pass
                return

            self.send_response(status)
            self.send_header("content-type", content_type)
            self.send_header("content-length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def _body(self) -> bytes:
            raw = self.headers.get("content-length")
            if raw is None:
                return b""
            try:
                length = int(raw)
            except ValueError as error:
                raise SystemContractError("invalid control/proxy content-length") from error
            return self.rfile.read(length)

        def _json_body(self) -> dict[str, Any]:
            payload = self._body()
            try:
                value = json.loads(payload or b"{}")
            except json.JSONDecodeError as error:
                raise SystemContractError("control request contains invalid JSON") from error
            if not isinstance(value, dict):
                raise SystemContractError("control request JSON must be an object")
            return value

        def _json(self, status: int, value: Any) -> None:
            payload = json.dumps(value, separators=(",", ":")).encode()
            self.send_response(status)
            self.send_header("content-type", "application/json")
            self.send_header("content-length", str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

    return Handler


def main() -> int:
    env = SystemEnvironment.from_env()
    stop = threading.Event()

    def request_stop(_signum: int, _frame: object) -> None:
        stop.set()

    signal.signal(signal.SIGTERM, request_stop)
    signal.signal(signal.SIGINT, request_stop)

    station = StationHarness(env.station_scalar)
    station.__enter__()
    server: ThreadingHTTPServer | None = None
    agent: AgentProcess | None = None
    temp: tempfile.TemporaryDirectory[str] | None = None
    thread: threading.Thread | None = None
    try:
        state = ControllerState(env, station)
        server = ThreadingHTTPServer(("127.0.0.1", 0), handler_for(state))
        host, port = server.server_address[:2]
        controller_url = f"http://{host}:{port}"
        thread = threading.Thread(target=server.serve_forever, name="lastro-e2e-controller-http", daemon=True)
        thread.start()

        temp = tempfile.TemporaryDirectory(prefix="lastro-e2e-")
        agent_env = replace(env, api_url=controller_url)
        agent = AgentProcess(agent_env, station, Path(temp.name))
        agent.__enter__()

        health = JsonHttpClient(env.api_url).get("/api/health")
        if not isinstance(health, dict) or health.get("status") != "ok":
            raise SystemContractError(f"API health is not ok: {health}")

        wallets = {
            "Wallet A": WalletActor.from_solana_keypair(env.wallet_a).address,
            "Wallet B": WalletActor.from_solana_keypair(env.wallet_b).address,
            "Wallet C": WalletActor.from_solana_keypair(env.wallet_c).address,
        }
        print(json.dumps({"controlUrl": controller_url, "wallets": wallets}), flush=True)

        while not stop.wait(0.2):
            agent.assert_running()
            station.raise_if_failed()
        return 0
    finally:
        if server is not None:
            server.shutdown()
            server.server_close()
        if thread is not None:
            thread.join(timeout=2)
        if agent is not None:
            agent.__exit__(None, None, None)
        station.__exit__(None, None, None)
        if temp is not None:
            temp.cleanup()


if __name__ == "__main__":
    raise SystemExit(main())
