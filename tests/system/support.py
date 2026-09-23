from __future__ import annotations

import base64
import binascii
import hashlib
import json
import os
import queue
import secrets
import select
import signal
import struct
import subprocess
import tempfile
import threading
import time
try:
    import tty
except ImportError:
    tty = None
import urllib.error
import urllib.parse
import urllib.request
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Iterable

from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec, ed25519
from cryptography.hazmat.primitives.asymmetric.utils import decode_dss_signature, encode_dss_signature

ZERO32 = bytes(32)
P256_ORDER = 0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551
SERIAL_MAGIC = b"LSTR"
SERIAL_VERSION = 1
MESSAGE_COMMAND = 1
MESSAGE_EVENT_READY = 2
MESSAGE_ACK = 3
STATION_EVENT_LENGTH = 276
SECP256R1_PROGRAM_ID = "Secp256r1SigVerify1111111111111111111111111"
SYSTEM_PROGRAM_ID = "11111111111111111111111111111111"
INSTRUCTIONS_SYSVAR_ID = "Sysvar1nstructions1111111111111111111111111"


class SystemContractError(RuntimeError):
    pass


class HttpStatusError(SystemContractError):
    def __init__(self, status: int, body: str, path: str):
        super().__init__(f"HTTP {status} for {path}: {body}")
        self.status = status
        self.body = body
        self.path = path


class RpcError(SystemContractError):
    pass


@dataclass(frozen=True)
class SystemEnvironment:
    api_url: str
    rpc_url: str
    program_id: str
    deployment_id: bytes
    agent_bin: Path
    agent_token: str
    station_scalar: int
    wallet_a: Path
    wallet_b: Path
    wallet_c: Path

    @classmethod
    def from_env(cls) -> "SystemEnvironment":
        required = {
            "LASTRO_SYSTEM_API_URL": os.getenv("LASTRO_SYSTEM_API_URL"),
            "LASTRO_SYSTEM_SOLANA_RPC_URL": os.getenv("LASTRO_SYSTEM_SOLANA_RPC_URL"),
            "LASTRO_PROGRAM_ID": os.getenv("LASTRO_PROGRAM_ID"),
            "LASTRO_DEPLOYMENT_ID_HEX": os.getenv("LASTRO_DEPLOYMENT_ID_HEX"),
            "LASTRO_SYSTEM_AGENT_BIN": os.getenv("LASTRO_SYSTEM_AGENT_BIN"),
            "LASTRO_AGENT_TOKEN": os.getenv("LASTRO_AGENT_TOKEN"),
            "LASTRO_SYSTEM_STATION_PRIVATE_SCALAR_HEX": os.getenv("LASTRO_SYSTEM_STATION_PRIVATE_SCALAR_HEX"),
            "LASTRO_SYSTEM_WALLET_A_KEYPAIR": os.getenv("LASTRO_SYSTEM_WALLET_A_KEYPAIR"),
            "LASTRO_SYSTEM_WALLET_B_KEYPAIR": os.getenv("LASTRO_SYSTEM_WALLET_B_KEYPAIR"),
            "LASTRO_SYSTEM_WALLET_C_KEYPAIR": os.getenv("LASTRO_SYSTEM_WALLET_C_KEYPAIR"),
        }
        missing = [name for name, value in required.items() if value is None or value.strip() == ""]
        if missing:
            raise SystemContractError(f"Missing full-system configuration: {', '.join(sorted(missing))}")
        deployment_id = decode_hex_exact(required["LASTRO_DEPLOYMENT_ID_HEX"], 32, "LASTRO_DEPLOYMENT_ID_HEX")
        scalar_text = required["LASTRO_SYSTEM_STATION_PRIVATE_SCALAR_HEX"]
        try:
            scalar = int(scalar_text, 16)
        except ValueError as error:
            raise SystemContractError("LASTRO_SYSTEM_STATION_PRIVATE_SCALAR_HEX must be hexadecimal") from error
        if not (1 <= scalar < P256_ORDER):
            raise SystemContractError("LASTRO_SYSTEM_STATION_PRIVATE_SCALAR_HEX is outside the P-256 scalar range")
        agent_bin = Path(required["LASTRO_SYSTEM_AGENT_BIN"]).expanduser().resolve()
        if not agent_bin.is_file():
            raise SystemContractError(f"LASTRO_SYSTEM_AGENT_BIN does not exist: {agent_bin}")
        wallets = [Path(required[name]).expanduser().resolve() for name in (
            "LASTRO_SYSTEM_WALLET_A_KEYPAIR",
            "LASTRO_SYSTEM_WALLET_B_KEYPAIR",
            "LASTRO_SYSTEM_WALLET_C_KEYPAIR",
        )]
        for wallet in wallets:
            if not wallet.is_file():
                raise SystemContractError(f"Solana wallet keypair does not exist: {wallet}")
        return cls(
            api_url=required["LASTRO_SYSTEM_API_URL"].rstrip("/"),
            rpc_url=required["LASTRO_SYSTEM_SOLANA_RPC_URL"],
            program_id=required["LASTRO_PROGRAM_ID"],
            deployment_id=deployment_id,
            agent_bin=agent_bin,
            agent_token=required["LASTRO_AGENT_TOKEN"],
            station_scalar=scalar,
            wallet_a=wallets[0],
            wallet_b=wallets[1],
            wallet_c=wallets[2],
        )


class JsonHttpClient:
    def __init__(self, base_url: str, timeout: float = 10.0):
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout

    def request(self, method: str, path: str, body: Any | None = None, headers: dict[str, str] | None = None) -> Any:
        data = None if body is None else json.dumps(body, separators=(",", ":")).encode()
        request_headers = {"accept": "application/json"}
        if data is not None:
            request_headers["content-type"] = "application/json"
        if headers:
            request_headers.update(headers)
        request = urllib.request.Request(
            f"{self.base_url}{path}",
            data=data,
            method=method,
            headers=request_headers,
        )
        try:
            with urllib.request.urlopen(request, timeout=self.timeout) as response:
                payload = response.read()
        except urllib.error.HTTPError as error:
            payload = error.read().decode(errors="replace")
            raise HttpStatusError(error.code, payload, path) from error
        except OSError as error:
            raise SystemContractError(f"HTTP request failed for {path}: {error}") from error
        if not payload:
            return None
        try:
            return json.loads(payload)
        except json.JSONDecodeError as error:
            raise SystemContractError(f"API returned invalid JSON for {path}") from error

    def get(self, path: str) -> Any:
        return self.request("GET", path)

    def post(self, path: str, body: Any) -> Any:
        return self.request("POST", path, body)


class SolanaRpcClient:
    def __init__(self, url: str, timeout: float = 15.0):
        self.url = url
        self.timeout = timeout
        self._request_id = 0

    def call(self, method: str, params: list[Any]) -> Any:
        self._request_id += 1
        data = json.dumps({"jsonrpc": "2.0", "id": self._request_id, "method": method, "params": params}).encode()
        request = urllib.request.Request(self.url, data=data, method="POST", headers={"content-type": "application/json"})
        try:
            with urllib.request.urlopen(request, timeout=self.timeout) as response:
                body = json.loads(response.read())
        except (OSError, json.JSONDecodeError) as error:
            raise RpcError(f"Solana RPC {method} request failed: {error}") from error
        if body.get("error") is not None:
            raise RpcError(f"Solana RPC {method} returned error: {body['error']}")
        if "result" not in body:
            raise RpcError(f"Solana RPC {method} response is missing result")
        return body["result"]

    def latest_blockhash(self) -> bytes:
        result = self.call("getLatestBlockhash", [{"commitment": "finalized"}])
        return b58decode_exact(result["value"]["blockhash"], 32, "recent blockhash")

    def send_transaction(self, wire: bytes, *, skip_preflight: bool = False) -> str:
        result = self.call(
            "sendTransaction",
            [
                base64.b64encode(wire).decode(),
                {
                    "encoding": "base64",
                    "preflightCommitment": "finalized",
                    "skipPreflight": skip_preflight,
                },
            ],
        )
        if not isinstance(result, str):
            raise RpcError("sendTransaction did not return a signature")
        return result

    def wait_signature(self, signature: str, timeout: float = 30.0) -> dict[str, Any]:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            result = self.call("getSignatureStatuses", [[signature], {"searchTransactionHistory": True}])
            values = result.get("value") if isinstance(result, dict) else None
            if isinstance(values, list) and values and values[0] is not None:
                status = values[0]
                if status.get("confirmationStatus") == "finalized":
                    return status
            time.sleep(0.2)
        raise RpcError(f"transaction {signature} did not reach finalized status")

    def account(self, address: str) -> dict[str, Any] | None:
        result = self.call("getAccountInfo", [address, {"commitment": "finalized", "encoding": "base64"}])
        return result.get("value") if isinstance(result, dict) else None

    def finalized_transaction(self, signature: str) -> Any:
        return self.call(
            "getTransaction",
            [signature, {"commitment": "finalized", "encoding": "json", "maxSupportedTransactionVersion": 0}],
        )


@dataclass(frozen=True)
class WalletActor:
    private_key: ed25519.Ed25519PrivateKey
    public_key: bytes
    address: str

    @classmethod
    def from_solana_keypair(cls, path: Path) -> "WalletActor":
        try:
            values = json.loads(path.read_text())
        except (OSError, json.JSONDecodeError) as error:
            raise SystemContractError(f"Cannot read Solana keypair {path}") from error
        if not isinstance(values, list) or len(values) != 64 or any(not isinstance(value, int) or not 0 <= value <= 255 for value in values):
            raise SystemContractError(f"Solana keypair {path} must be a JSON array of 64 bytes")
        raw = bytes(values)
        private_key = ed25519.Ed25519PrivateKey.from_private_bytes(raw[:32])
        public_key = private_key.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
        if public_key != raw[32:]:
            raise SystemContractError(f"Solana keypair {path} public key does not match its private seed")
        return cls(private_key=private_key, public_key=public_key, address=b58encode(public_key))

    @property
    def custodian_hex(self) -> str:
        return self.public_key.hex()


    def sign_capture_authorization(
        self,
        challenge: dict[str, Any],
        *,
        deployment_id: bytes,
        program_id: str,
        action: str,
        animal_id: str,
        next_custodian: str | None,
    ) -> dict[str, str]:
        challenge_id = challenge.get("challengeId")
        if not isinstance(challenge_id, str):
            raise SystemContractError("capture authorization challenge is missing challengeId")
        try:
            uuid.UUID(challenge_id)
        except ValueError as error:
            raise SystemContractError("capture authorization challengeId is not a UUID") from error
        if challenge.get("deploymentId") != deployment_id.hex():
            raise SystemContractError("capture authorization deployment does not match system configuration")
        if challenge.get("requiredSigner") != self.address:
            raise SystemContractError("capture authorization requiredSigner does not match selected wallet actor")
        expires_at = challenge.get("expiresAtUnix")
        if not isinstance(expires_at, int) or expires_at <= int(time.time()):
            raise SystemContractError("capture authorization challenge is expired or invalid")
        message_base64 = challenge.get("messageBase64")
        if not isinstance(message_base64, str):
            raise SystemContractError("capture authorization challenge is missing messageBase64")
        try:
            message = base64.b64decode(message_base64, validate=True)
        except (ValueError, binascii.Error) as error:
            raise SystemContractError("capture authorization message is not canonical base64") from error
        if base64.b64encode(message).decode() != message_base64:
            raise SystemContractError("capture authorization message is not canonical base64")
        expected = (
            "Lastro capture authorization v1\n"
            f"challengeId={challenge_id}\n"
            f"deploymentId={deployment_id.hex()}\n"
            f"programId={program_id}\n"
            f"action={action}\n"
            f"animalId={animal_id}\n"
            f"nextCustodian={next_custodian if next_custodian is not None else 'none'}\n"
            f"requiredSigner={self.address}\n"
            f"expiresAtUnix={expires_at}\n"
        ).encode()
        if message != expected:
            raise SystemContractError("capture authorization message does not match the exact trusted system intent")
        signature = self.private_key.sign(message)
        if len(signature) != 64:
            raise SystemContractError("capture authorization signature is not 64 bytes")
        return {
            "challengeId": challenge_id,
            "signatureBase64": base64.b64encode(signature).decode(),
        }

    def sign_transaction_data(self, rpc: SolanaRpcClient, transaction_data: dict[str, Any]) -> tuple[str, bytes]:
        if transaction_data.get("transactionVersion") != "legacy":
            raise SystemContractError("system wallet harness only accepts the API's required legacy Lastro transaction")
        if transaction_data.get("requiredSigner") != self.address:
            raise SystemContractError("transaction-data requiredSigner does not match the selected wallet actor")
        instructions = transaction_data.get("instructions")
        if not isinstance(instructions, list) or len(instructions) != 2:
            raise SystemContractError("Lastro transaction-data must contain exactly two instructions")
        if instructions[0].get("programId") != SECP256R1_PROGRAM_ID:
            raise SystemContractError("Lastro transaction-data instruction 0 is not Secp256r1")
        message = compile_legacy_message(self.public_key, rpc.latest_blockhash(), instructions)
        signature = self.private_key.sign(message)
        wire = encode_shortvec(1) + signature + message
        measured = transaction_data.get("measuredSerializedBytes")
        if measured != len(wire):
            raise SystemContractError(f"API measured {measured} bytes but system wallet compiled {len(wire)} bytes")
        if len(wire) > 1232:
            raise SystemContractError("compiled Lastro transaction exceeds 1232 bytes")
        return b58encode(signature), wire


@dataclass(frozen=True)
class StationObservation:
    action: int
    rfid: bytes


class StationHarness:
    def __init__(self, private_scalar: int):
        self.private_key = ec.derive_private_key(private_scalar, ec.SECP256R1())
        self.public_key = self.private_key.public_key().public_bytes(
            serialization.Encoding.X962,
            serialization.PublicFormat.CompressedPoint,
        )
        self.station_id = hashlib.sha256(b"LASTRO_STATION\x00" + self.public_key).digest()
        self.master_fd, self.slave_fd = os.openpty()
        tty.setraw(self.master_fd)
        tty.setraw(self.slave_fd)
        self.serial_path = os.ttyname(self.slave_fd)
        self._observations: queue.Queue[StationObservation] = queue.Queue()
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._run, name="lastro-station-harness", daemon=True)
        self._buffer = bytearray()
        self._responses: dict[bytes, bytes] = {}
        self._acks: set[bytes] = set()
        self._condition = threading.Condition()
        self._error: BaseException | None = None

    def __enter__(self) -> "StationHarness":
        self._thread.start()
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        self._stop.set()
        try:
            os.write(self.master_fd, b"\x00")
        except OSError:
            pass
        self._thread.join(timeout=2)
        for fd in (self.master_fd, self.slave_fd):
            try:
                os.close(fd)
            except OSError:
                pass
        if exc_type is None:
            self.raise_if_failed()

    def queue_observation(self, action: int, rfid_hex: str) -> None:
        rfid = decode_hex_exact(rfid_hex, 8, "RFID")
        self._observations.put(StationObservation(action=action, rfid=rfid))

    def wait_ack(self, capture_id: str, timeout: float = 15.0) -> None:
        capture = uuid.UUID(capture_id).bytes
        deadline = time.monotonic() + timeout
        with self._condition:
            while capture not in self._acks:
                self.raise_if_failed()
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    raise SystemContractError(f"Agent did not ACK durable evidence for capture {capture_id}")
                self._condition.wait(timeout=min(remaining, 0.2))

    def raise_if_failed(self) -> None:
        if self._error is not None:
            raise SystemContractError(f"Station harness failed: {self._error}") from self._error

    def _run(self) -> None:
        try:
            os.set_blocking(self.master_fd, False)
            while not self._stop.is_set():
                readable, _, _ = select.select([self.master_fd], [], [], 0.2)
                if not readable:
                    continue
                try:
                    chunk = os.read(self.master_fd, 4096)
                except BlockingIOError:
                    continue
                if not chunk:
                    continue
                self._buffer.extend(chunk)
                for message_type, payload in take_serial_frames(self._buffer):
                    if message_type == MESSAGE_COMMAND:
                        self._handle_command(payload)
                    elif message_type == MESSAGE_ACK:
                        if len(payload) != 48:
                            raise SystemContractError("Agent ACK payload length is not 48 bytes")
                        with self._condition:
                            self._acks.add(bytes(payload[:16]))
                            self._condition.notify_all()
                    else:
                        raise SystemContractError(f"Station harness received unexpected serial message type {message_type}")
        except BaseException as error:
            self._error = error
            with self._condition:
                self._condition.notify_all()

    def _handle_command(self, payload: bytes) -> None:
        command = decode_station_command(payload)
        capture = command["capture_id"]
        if capture in self._responses:
            os.write(self.master_fd, self._responses[capture])
            return
        try:
            observation = self._observations.get(timeout=10)
        except queue.Empty as error:
            raise SystemContractError("Station command arrived without a declared physical observation") from error
        if observation.action != command["action"]:
            raise SystemContractError(
                f"Station observation expected action {observation.action}, received {command['action']}"
            )
        event = build_station_event(command, observation.rfid, self.station_id)
        signature = sign_p256_compact_low_s(self.private_key, event)
        payload = capture + event + observation.rfid + self.public_key + signature
        if len(payload) != 397:
            raise AssertionError("EVENT_READY payload length drifted")
        frame = encode_serial_frame(MESSAGE_EVENT_READY, payload)
        self._responses[capture] = frame
        os.write(self.master_fd, frame)


class AgentProcess:
    def __init__(self, env: SystemEnvironment, station: StationHarness, temp_dir: Path):
        self.env = env
        self.station = station
        self.temp_dir = temp_dir
        self.process: subprocess.Popen[bytes] | None = None
        self.log_file = None

    def __enter__(self) -> "AgentProcess":
        sqlite_path = self.temp_dir / "agent.sqlite3"
        log_path = self.temp_dir / "agent.log"
        self.log_file = log_path.open("wb")
        process_env = os.environ.copy()
        process_env.update({
            "LASTRO_AGENT_API_URL": f"{self.env.api_url}/",
            "LASTRO_AGENT_TOKEN": self.env.agent_token,
            "LASTRO_AGENT_SERIAL_PORT": self.station.serial_path,
            "LASTRO_AGENT_SERIAL_BAUD": "115200",
            "LASTRO_STATION_PUBKEY_HEX": self.station.public_key.hex(),
            "LASTRO_AGENT_SQLITE_URL": f"sqlite://{sqlite_path}",
            "LASTRO_AGENT_POLL_INTERVAL_MS": "50",
            "LASTRO_AGENT_REQUEST_TIMEOUT_MS": "5000",
            "LASTRO_AGENT_STATION_RESPONSE_TIMEOUT_MS": "3000",
            "RUST_LOG": os.getenv("RUST_LOG", "lastro_agent=info"),
        })
        self.process = subprocess.Popen(
            [str(self.env.agent_bin)],
            cwd=self.temp_dir,
            env=process_env,
            stdout=self.log_file,
            stderr=subprocess.STDOUT,
        )
        time.sleep(0.15)
        self.assert_running()
        return self

    def __exit__(self, exc_type, exc, tb) -> None:
        if self.process is not None and self.process.poll() is None:
            self.process.send_signal(signal.SIGTERM)
            try:
                self.process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=3)
        if self.log_file is not None:
            self.log_file.close()
        if exc_type is None:
            self.station.raise_if_failed()

    def assert_running(self) -> None:
        if self.process is None:
            raise SystemContractError("Agent process was not started")
        code = self.process.poll()
        if code is not None:
            log = (self.temp_dir / "agent.log").read_text(errors="replace")
            raise SystemContractError(f"Agent exited with status {code}:\n{log}")


@dataclass
class FlowResult:
    animal_id: str
    visual_recovery_id: str
    projections: list[dict[str, Any]]
    transaction_data: list[dict[str, Any]]
    transaction_signatures: list[str]
    stale_attempt_signature: str | None
    evidence_package: dict[str, Any]
    rfid_a_hash: str
    rfid_b_hash: str

    @property
    def final_projection(self) -> dict[str, Any]:
        return self.projections[-1]


class FullLocalHarness:
    def __init__(self, env: SystemEnvironment):
        self.env = env
        self.api = JsonHttpClient(env.api_url)
        self.rpc = SolanaRpcClient(env.rpc_url)
        self.wallet_a = WalletActor.from_solana_keypair(env.wallet_a)
        self.wallet_b = WalletActor.from_solana_keypair(env.wallet_b)
        self.wallet_c = WalletActor.from_solana_keypair(env.wallet_c)
        self.station = StationHarness(env.station_scalar)
        configured_station = os.getenv("LASTRO_STATION_PUBKEY_HEX")
        if configured_station and configured_station != self.station.public_key.hex():
            raise SystemContractError("LASTRO_STATION_PUBKEY_HEX does not match the system Station private scalar")
        self.temp = tempfile.TemporaryDirectory(prefix="lastro-system-")
        self.agent = AgentProcess(env, self.station, Path(self.temp.name))

    def __enter__(self) -> "FullLocalHarness":
        self.station.__enter__()
        try:
            self.agent.__enter__()
            health = self.api.get("/api/health")
            if health.get("status") != "ok":
                raise SystemContractError(f"API health is not ok: {health}")
            return self
        except BaseException:
            self.station.__exit__(*__import__("sys").exc_info())
            self.temp.cleanup()
            raise

    def __exit__(self, exc_type, exc, tb) -> None:
        agent_error: BaseException | None = None
        try:
            self.agent.__exit__(exc_type, exc, tb)
        except BaseException as error:
            agent_error = error
        try:
            self.station.__exit__(exc_type, exc, tb)
        finally:
            self.temp.cleanup()
        if exc_type is None and agent_error is not None:
            raise agent_error

    def run_core_flow(
        self,
        *,
        stale_attack: bool = True,
        rfid_a_hex: str | None = None,
        rfid_b_hex: str | None = None,
    ) -> FlowResult:
        rfid_a = rfid_a_hex or fresh_rfid_hex()
        rfid_b = rfid_b_hex or fresh_rfid_hex(excluding={rfid_a})
        rfid_a_hash = rfid_hash(decode_hex_exact(rfid_a, 8, "LASTRO_SYSTEM_RFID_A_HEX")).hex()
        rfid_b_hash = rfid_hash(decode_hex_exact(rfid_b, 8, "LASTRO_SYSTEM_RFID_B_HEX")).hex()
        visual = f"0042-{uuid.uuid4().hex[:12]}"
        animal = self.api.post("/api/animals", {"visualRecoveryId": visual})
        animal_id = require_hex(animal.get("animalId"), 32, "animalId")
        projections: list[dict[str, Any]] = []
        descriptors: list[dict[str, Any]] = []
        signatures: list[str] = []

        projection, descriptor, signature = self._transition(
            action="ORIGIN", action_code=1, animal_id=animal_id, rfid_hex=rfid_a,
            wallet=self.wallet_a, next_custodian=self.wallet_a.custodian_hex,
        )
        projections.append(projection); descriptors.append(descriptor); signatures.append(signature)

        projection, descriptor, signature = self._transition(
            action="TRANSFER", action_code=2, animal_id=animal_id, rfid_hex=rfid_a,
            wallet=self.wallet_a, next_custodian=self.wallet_b.custodian_hex,
        )
        projections.append(projection); descriptors.append(descriptor); signatures.append(signature)

        stale_signature = None
        if stale_attack:
            before = self.read_animal_state(descriptor)
            stale_signature = self._submit_stale_replay(descriptor, self.wallet_a)
            after = self.read_animal_state(descriptor)
            if after != before:
                raise SystemContractError("stale custodian replay changed canonical AnimalState")

        recovered = self.api.get(f"/api/animals/by-recovery/{urllib.parse.quote(visual, safe='')}")
        if recovered.get("animalId") != animal_id:
            raise SystemContractError("visual recovery returned a different AnimalID")

        projection, descriptor, signature = self._transition(
            action="REIDENTIFY", action_code=3, animal_id=animal_id, rfid_hex=rfid_b,
            wallet=self.wallet_b, next_custodian=None,
        )
        projections.append(projection); descriptors.append(descriptor); signatures.append(signature)

        projection, descriptor, signature = self._transition(
            action="TRANSFER", action_code=2, animal_id=animal_id, rfid_hex=rfid_b,
            wallet=self.wallet_b, next_custodian=self.wallet_c.custodian_hex,
        )
        projections.append(projection); descriptors.append(descriptor); signatures.append(signature)

        package = self.api.get(f"/api/animals/{animal_id}/evidence-package")
        verify_evidence_package(package, self.rpc, self.env.program_id)
        return FlowResult(
            animal_id=animal_id,
            visual_recovery_id=visual,
            projections=projections,
            transaction_data=descriptors,
            transaction_signatures=signatures,
            stale_attempt_signature=stale_signature,
            evidence_package=package,
            rfid_a_hash=rfid_a_hash,
            rfid_b_hash=rfid_b_hash,
        )


    def reserve_capture(
        self,
        *,
        action: str,
        animal_id: str,
        wallet: WalletActor,
        next_custodian: str | None,
    ) -> dict[str, Any]:
        intent = {
            "action": action,
            "animalId": animal_id,
            "nextCustodian": next_custodian,
        }
        challenge = self.api.post("/api/captures/authorization-challenge", intent)
        if not isinstance(challenge, dict):
            raise SystemContractError("capture authorization endpoint did not return an object")
        authorization = wallet.sign_capture_authorization(
            challenge,
            deployment_id=self.env.deployment_id,
            program_id=self.env.program_id,
            action=action,
            animal_id=animal_id,
            next_custodian=next_custodian,
        )
        capture = self.api.post("/api/captures", {
            **intent,
            "authorization": authorization,
        })
        if not isinstance(capture, dict):
            raise SystemContractError("capture endpoint did not return an object")
        return capture

    def _transition(
        self,
        *,
        action: str,
        action_code: int,
        animal_id: str,
        rfid_hex: str,
        wallet: WalletActor,
        next_custodian: str | None,
    ) -> tuple[dict[str, Any], dict[str, Any], str]:
        self.station.queue_observation(action_code, rfid_hex)
        capture = self.reserve_capture(
            action=action,
            animal_id=animal_id,
            wallet=wallet,
            next_custodian=next_custodian,
        )
        capture_id = capture.get("captureId")
        if not isinstance(capture_id, str):
            raise SystemContractError("capture response is missing captureId")
        accepted = self.wait_capture(capture_id)
        self.station.wait_ack(capture_id)
        event_hash = require_hex(accepted.get("eventHash"), 32, "eventHash")
        descriptor = self.api.get(f"/api/events/{event_hash}/transaction-data")
        signature_text, wire = wallet.sign_transaction_data(self.rpc, descriptor)
        returned = self.rpc.send_transaction(wire)
        if returned != signature_text:
            raise SystemContractError("RPC returned a transaction signature different from the signed wire transaction")
        projection = self.confirm(event_hash, returned)
        assert_projection_matches_rpc(projection, self.read_animal_state(descriptor))
        return projection, descriptor, returned

    def wait_capture(self, capture_id: str, timeout: float = 20.0) -> dict[str, Any]:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.agent.assert_running()
            self.station.raise_if_failed()
            capture = self.api.get(f"/api/captures/{capture_id}")
            status = capture.get("status")
            if status == "EVIDENCE_ACCEPTED":
                return capture
            if status in ("EXPIRED", "CANCELLED"):
                raise SystemContractError(f"capture {capture_id} ended with {status}")
            time.sleep(0.1)
        raise SystemContractError(f"capture {capture_id} did not reach EVIDENCE_ACCEPTED")

    def confirm(self, event_hash: str, signature: str, timeout: float = 30.0) -> dict[str, Any]:
        deadline = time.monotonic() + timeout
        submitted = False
        while time.monotonic() < deadline:
            if not submitted:
                try:
                    response = self.api.post(f"/api/events/{event_hash}/submit", {"txSignature": signature})
                    if response.get("txSignature") != signature or response.get("status") not in ("SUBMITTED", "FINALIZED"):
                        raise SystemContractError("API returned inconsistent submitted transaction metadata")
                    submitted = True
                except HttpStatusError as error:
                    if error.status != 409 or http_error_message(error) != "transaction is not a confirmed exact Lastro transaction for this event":
                        raise
                    time.sleep(0.2)
                    continue
            try:
                return self.api.post(f"/api/events/{event_hash}/confirm", {"txSignature": signature})
            except HttpStatusError as error:
                if error.status != 409 or http_error_message(error) != "transaction is not a finalized exact Lastro transaction for this event":
                    raise
            time.sleep(0.2)
        raise SystemContractError(f"API did not finalize submitted transaction {signature}")

    def _submit_stale_replay(self, descriptor: dict[str, Any], wallet: WalletActor) -> str | None:
        _, wire = wallet.sign_transaction_data(self.rpc, descriptor)
        try:
            signature = self.rpc.send_transaction(wire, skip_preflight=True)
        except RpcError:
            return None
        status = self.rpc.wait_signature(signature)
        if status.get("err") is None:
            raise SystemContractError("stale custodian replay unexpectedly finalized successfully")
        return signature

    def read_animal_state(self, descriptor: dict[str, Any]) -> dict[str, Any]:
        accounts = descriptor["instructions"][1]["accounts"]
        address = accounts[2]["address"]
        return decode_animal_account(self.rpc.account(address), self.env.program_id)



def fresh_rfid_hex(*, excluding: set[str] | None = None) -> str:
    excluded = excluding or set()
    while True:
        value = secrets.token_bytes(8)
        if value != bytes(8) and value.hex() not in excluded:
            return value.hex()

def http_error_message(error: HttpStatusError) -> str | None:
    try:
        value = json.loads(error.body)
    except json.JSONDecodeError:
        return None
    if not isinstance(value, dict):
        return None
    message = value.get("message")
    return message if isinstance(message, str) else None


def require_system_environment() -> SystemEnvironment:
    if os.getenv("LASTRO_SYSTEM_TEST") != "1":
        import pytest
        pytest.skip("Set LASTRO_SYSTEM_TEST=1 only in the declared full local environment")
    return SystemEnvironment.from_env()


def rpc_json(url: str, method: str, params: list[Any]) -> Any:
    return SolanaRpcClient(url).call(method, params)


def compile_legacy_message(fee_payer: bytes, recent_blockhash: bytes, instructions: list[dict[str, Any]]) -> bytes:
    fee_payer_address = b58encode(fee_payer)
    roles: dict[str, list[Any]] = {fee_payer_address: [True, True, -1]}
    order = 0
    for instruction in instructions:
        accounts = instruction.get("accounts")
        if not isinstance(accounts, list):
            raise SystemContractError("instruction accounts must be an array")
        for account in accounts:
            address = account.get("address")
            if not isinstance(address, str):
                raise SystemContractError("instruction account address is invalid")
            signer = account.get("isSigner") is True
            writable = account.get("isWritable") is True
            if address in roles:
                roles[address][0] = roles[address][0] or signer
                roles[address][1] = roles[address][1] or writable
            else:
                roles[address] = [signer, writable, order]
                order += 1
        program = instruction.get("programId")
        if not isinstance(program, str):
            raise SystemContractError("instruction programId is invalid")
        if program not in roles:
            roles[program] = [False, False, order]
            order += 1

    other_signers = [key for key, role in roles.items() if role[0] and key != fee_payer_address]
    if other_signers:
        raise SystemContractError("Lastro system wallet requires fee payer to be the only transaction signer")

    def category(address: str) -> tuple[int, int]:
        signer, writable, first = roles[address]
        if address == fee_payer_address:
            return (0, -1)
        if signer and writable:
            return (1, first)
        if signer:
            return (2, first)
        if writable:
            return (3, first)
        return (4, first)

    account_addresses = sorted(roles, key=category)
    account_keys = [b58decode_exact(value, 32, "Solana account address") for value in account_addresses]
    index = {address: position for position, address in enumerate(account_addresses)}
    num_required = sum(1 for address in account_addresses if roles[address][0])
    num_readonly_signed = sum(1 for address in account_addresses if roles[address][0] and not roles[address][1])
    num_readonly_unsigned = sum(1 for address in account_addresses if not roles[address][0] and not roles[address][1])
    if len(account_addresses) - 1 > 255:
        raise SystemContractError("legacy transaction account index does not fit u8")

    compiled = bytearray()
    compiled.extend(bytes([num_required, num_readonly_signed, num_readonly_unsigned]))
    compiled.extend(encode_shortvec(len(account_keys)))
    for key in account_keys:
        compiled.extend(key)
    compiled.extend(recent_blockhash)
    compiled.extend(encode_shortvec(len(instructions)))
    for instruction in instructions:
        program = instruction["programId"]
        compiled.append(index[program])
        accounts = instruction["accounts"]
        compiled.extend(encode_shortvec(len(accounts)))
        compiled.extend(index[account["address"]] for account in accounts)
        try:
            data = base64.b64decode(instruction["dataBase64"], validate=True)
        except (ValueError, TypeError) as error:
            raise SystemContractError("instruction dataBase64 is invalid") from error
        if base64.b64encode(data).decode() != instruction["dataBase64"]:
            raise SystemContractError("instruction dataBase64 is not canonical")
        compiled.extend(encode_shortvec(len(data)))
        compiled.extend(data)
    return bytes(compiled)


def encode_shortvec(value: int) -> bytes:
    if value < 0:
        raise ValueError("shortvec cannot encode a negative value")
    out = bytearray()
    while True:
        elem = value & 0x7F
        value >>= 7
        if value:
            elem |= 0x80
        out.append(elem)
        if not value:
            return bytes(out)


def b58encode(data: bytes) -> str:
    alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    zeros = len(data) - len(data.lstrip(b"\x00"))
    number = int.from_bytes(data, "big")
    chars: list[str] = []
    while number:
        number, remainder = divmod(number, 58)
        chars.append(alphabet[remainder])
    return "1" * zeros + "".join(reversed(chars))


def b58decode(value: str) -> bytes:
    alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    table = {character: index for index, character in enumerate(alphabet)}
    number = 0
    for character in value:
        if character not in table:
            raise SystemContractError("invalid base58 value")
        number = number * 58 + table[character]
    raw = b"" if number == 0 else number.to_bytes((number.bit_length() + 7) // 8, "big")
    zeros = len(value) - len(value.lstrip("1"))
    return b"\x00" * zeros + raw


def b58decode_exact(value: str, length: int, name: str) -> bytes:
    raw = b58decode(value)
    if len(raw) != length or b58encode(raw) != value:
        raise SystemContractError(f"{name} must be canonical base58 for {length} bytes")
    return raw


def decode_hex_exact(value: str | None, length: int, name: str) -> bytes:
    if value is None or len(value) != length * 2 or value != value.lower():
        raise SystemContractError(f"{name} must be exactly {length} bytes of lowercase hexadecimal")
    try:
        raw = bytes.fromhex(value)
    except ValueError as error:
        raise SystemContractError(f"{name} must be valid lowercase hexadecimal") from error
    if raw.hex() != value:
        raise SystemContractError(f"{name} must be canonical lowercase hexadecimal")
    return raw


def require_hex(value: Any, length: int, name: str) -> str:
    if not isinstance(value, str):
        raise SystemContractError(f"{name} must be a string")
    decode_hex_exact(value, length, name)
    return value


def crc32c(data: bytes) -> int:
    crc = 0xFFFFFFFF
    for byte in data:
        crc ^= byte
        for _ in range(8):
            crc = (crc >> 1) ^ (0x82F63B78 if crc & 1 else 0)
    return (~crc) & 0xFFFFFFFF


def encode_serial_frame(message_type: int, payload: bytes) -> bytes:
    header = bytes([SERIAL_VERSION, message_type]) + b"\x00\x00" + struct.pack("<I", len(payload))
    return SERIAL_MAGIC + header + payload + struct.pack("<I", crc32c(header + payload))


def take_serial_frames(buffer: bytearray) -> Iterable[tuple[int, bytes]]:
    frames: list[tuple[int, bytes]] = []
    while True:
        index = buffer.find(SERIAL_MAGIC)
        if index < 0:
            if len(buffer) > 3:
                del buffer[:-3]
            break
        if index:
            del buffer[:index]
        if len(buffer) < 12:
            break
        version, message_type = buffer[4], buffer[5]
        reserved = buffer[6:8]
        payload_len = struct.unpack_from("<I", buffer, 8)[0]
        if version != SERIAL_VERSION or reserved != b"\x00\x00" or payload_len > 1024:
            del buffer[0]
            continue
        total = 12 + payload_len + 4
        if len(buffer) < total:
            break
        expected = struct.unpack_from("<I", buffer, 12 + payload_len)[0]
        actual = crc32c(bytes(buffer[4:12 + payload_len]))
        if expected != actual:
            del buffer[0]
            continue
        payload = bytes(buffer[12:12 + payload_len])
        del buffer[:total]
        frames.append((message_type, payload))
    return frames


def decode_station_command(payload: bytes) -> dict[str, Any]:
    if len(payload) != 224:
        raise SystemContractError("COMMAND payload length is not 224 bytes")
    if payload[17:20] != b"\x00\x00\x00":
        raise SystemContractError("COMMAND reserved bytes are not zero")
    action = payload[16]
    if action not in (1, 2, 3):
        raise SystemContractError("COMMAND action is invalid")
    return {
        "capture_id": bytes(payload[:16]),
        "action": action,
        "deployment_id": bytes(payload[20:52]),
        "animal_id": bytes(payload[52:84]),
        "event_sequence": struct.unpack_from("<Q", payload, 84)[0],
        "identity_revision": struct.unpack_from("<I", payload, 92)[0],
        "previous_event_hash": bytes(payload[96:128]),
        "expected_old_rfid_hash": bytes(payload[128:160]),
        "from_custodian": bytes(payload[160:192]),
        "to_custodian": bytes(payload[192:224]),
    }


def rfid_hash(rfid: bytes) -> bytes:
    if len(rfid) != 8:
        raise SystemContractError("canonical RFID must be exactly 8 bytes")
    return hashlib.sha256(b"LASTRO_RFID\x00" + rfid).digest()


def build_station_event(command: dict[str, Any], observed_rfid: bytes, station_id: bytes) -> bytes:
    action = command["action"]
    observed_hash = rfid_hash(observed_rfid)
    old_hash = command["expected_old_rfid_hash"]
    if action == 1:
        if old_hash != ZERO32:
            raise SystemContractError("ORIGIN command unexpectedly contains an old RFID")
        new_hash = observed_hash
    elif action == 2:
        if observed_hash != old_hash:
            raise SystemContractError("TRANSFER physical RFID does not match canonical current RFID")
        new_hash = old_hash
    elif action == 3:
        if observed_hash == old_hash:
            raise SystemContractError("REIDENTIFY physical RFID must differ from the retired RFID")
        new_hash = observed_hash
    else:
        raise SystemContractError("unsupported Station action")

    out = bytearray(STATION_EVENT_LENGTH)
    out[0:4] = b"LSTR"
    out[4] = 1
    out[5] = action
    out[6:8] = b"\x00\x00"
    out[8:40] = command["deployment_id"]
    out[40:72] = command["animal_id"]
    out[72:104] = station_id
    struct.pack_into("<Q", out, 104, command["event_sequence"])
    struct.pack_into("<I", out, 112, command["identity_revision"])
    out[116:148] = command["previous_event_hash"]
    out[148:180] = old_hash
    out[180:212] = new_hash
    out[212:244] = command["from_custodian"]
    out[244:276] = command["to_custodian"]
    return bytes(out)


def sign_p256_compact_low_s(private_key: ec.EllipticCurvePrivateKey, message: bytes) -> bytes:
    der = private_key.sign(message, ec.ECDSA(hashes.SHA256()))
    r, s = decode_dss_signature(der)
    if s > P256_ORDER // 2:
        s = P256_ORDER - s
    return r.to_bytes(32, "big") + s.to_bytes(32, "big")


def decode_station_event(data: bytes) -> dict[str, Any]:
    if len(data) != 276 or data[:4] != b"LSTR" or data[4] != 1 or data[6:8] != b"\x00\x00":
        raise SystemContractError("StationEvent framing is invalid")
    action = data[5]
    if action not in (1, 2, 3):
        raise SystemContractError("StationEvent action is invalid")
    return {
        "action": action,
        "deployment_id": data[8:40],
        "animal_id": data[40:72],
        "station_id": data[72:104],
        "event_sequence": struct.unpack_from("<Q", data, 104)[0],
        "identity_revision": struct.unpack_from("<I", data, 112)[0],
        "previous_event_hash": data[116:148],
        "old_rfid_hash": data[148:180],
        "new_rfid_hash": data[180:212],
        "from_custodian": data[212:244],
        "to_custodian": data[244:276],
    }


def decode_account_data(account: dict[str, Any] | None, program_id: str, expected_length: int) -> bytes:
    if account is None:
        raise SystemContractError("canonical Solana account does not exist")
    if account.get("owner") != program_id or account.get("executable") is not False:
        raise SystemContractError("canonical account owner/executable flag is invalid")
    data = account.get("data")
    if not isinstance(data, list) or len(data) != 2 or data[1] != "base64" or not isinstance(data[0], str):
        raise SystemContractError("canonical account data is not base64")
    try:
        raw = base64.b64decode(data[0], validate=True)
    except ValueError as error:
        raise SystemContractError("canonical account data is invalid base64") from error
    if base64.b64encode(raw).decode() != data[0] or len(raw) != expected_length:
        raise SystemContractError("canonical account data length/base64 is not canonical")
    return raw


def anchor_discriminator(account_name: str) -> bytes:
    return hashlib.sha256(f"account:{account_name}".encode()).digest()[:8]


def decode_animal_account(account: dict[str, Any] | None, program_id: str) -> dict[str, Any]:
    data = decode_account_data(account, program_id, 149)
    if data[:8] != anchor_discriminator("AnimalState"):
        raise SystemContractError("AnimalState discriminator is invalid")
    return {
        "animal_id": data[8:40].hex(),
        "current_rfid_hash": data[40:72].hex(),
        "current_custodian": data[72:104].hex(),
        "identity_revision": struct.unpack_from("<I", data, 104)[0],
        "event_sequence": struct.unpack_from("<Q", data, 108)[0],
        "last_event_hash": data[116:148].hex(),
        "bump": data[148],
    }


def decode_binding_account(account: dict[str, Any] | None, program_id: str) -> dict[str, Any]:
    data = decode_account_data(account, program_id, 74)
    if data[:8] != anchor_discriminator("RfidBinding"):
        raise SystemContractError("RfidBinding discriminator is invalid")
    if data[72] not in (1, 2):
        raise SystemContractError("RfidBinding status is invalid")
    return {
        "animal_id": data[8:40].hex(),
        "rfid_hash": data[40:72].hex(),
        "status": data[72],
        "bump": data[73],
    }


def assert_projection_matches_rpc(projection: dict[str, Any], canonical: dict[str, Any]) -> None:
    expected = {
        "animalId": canonical["animal_id"],
        "currentRfidHash": canonical["current_rfid_hash"],
        "currentCustodian": canonical["current_custodian"],
        "identityRevision": canonical["identity_revision"],
        "eventSequence": canonical["event_sequence"],
        "lastEventHash": canonical["last_event_hash"],
    }
    for key, value in expected.items():
        if projection.get(key) != value:
            raise SystemContractError(f"API projection {key} disagrees with canonical Solana state")


def create_program_address(seeds: list[bytes], program_id: bytes) -> bytes:
    if len(seeds) > 16 or any(len(seed) > 32 for seed in seeds):
        raise SystemContractError("PDA seeds exceed Solana limits")
    digest = hashlib.sha256(b"".join(seeds) + program_id + b"ProgramDerivedAddress").digest()
    if ed25519_point_exists(digest):
        raise ValueError("derived address is on the Ed25519 curve")
    return digest


def find_program_address(seeds: list[bytes], program_id: bytes) -> tuple[bytes, int]:
    for bump in range(255, -1, -1):
        try:
            return create_program_address([*seeds, bytes([bump])], program_id), bump
        except ValueError:
            continue
    raise SystemContractError("unable to derive Solana PDA")


def ed25519_point_exists(encoded: bytes) -> bool:
    if len(encoded) != 32:
        return False
    p = 2**255 - 19
    y = int.from_bytes(encoded, "little") & ((1 << 255) - 1)
    sign = encoded[31] >> 7
    if y >= p:
        return False
    d = (-121665 * pow(121666, p - 2, p)) % p
    y2 = y * y % p
    denominator = (d * y2 + 1) % p
    if denominator == 0:
        return False
    x2 = (y2 - 1) * pow(denominator, p - 2, p) % p
    x = pow(x2, (p + 3) // 8, p)
    if x * x % p != x2:
        sqrt_m1 = pow(2, (p - 1) // 4, p)
        x = x * sqrt_m1 % p
    if x * x % p != x2:
        return False
    if x == 0 and sign == 1:
        return False
    return True


def expected_event_transaction(
    raw_event: bytes,
    event: dict[str, Any],
    station_pubkey: bytes,
    station_signature: bytes,
    program_id_text: str,
) -> tuple[str, list[dict[str, Any]]]:
    program_id = b58decode_exact(program_id_text, 32, "LASTRO_PROGRAM_ID")
    deployment = event["deployment_id"]
    animal_id = event["animal_id"]
    config, _ = find_program_address([b"config", deployment], program_id)
    animal, _ = find_program_address([b"animal", deployment, animal_id], program_id)
    required_signer = b58encode(event["to_custodian"] if event["action"] == 1 else event["from_custodian"])

    def meta(address: bytes | str, signer: bool, writable: bool) -> dict[str, Any]:
        return {
            "address": address if isinstance(address, str) else b58encode(address),
            "isSigner": signer,
            "isWritable": writable,
        }

    if event["action"] == 1:
        binding, _ = find_program_address([b"rfid", deployment, event["new_rfid_hash"]], program_id)
        accounts = [
            meta(required_signer, True, True),
            meta(config, False, False),
            meta(animal, False, True),
            meta(binding, False, True),
            meta(INSTRUCTIONS_SYSVAR_ID, False, False),
            meta(SYSTEM_PROGRAM_ID, False, False),
        ]
        action_name = "origin"
    elif event["action"] == 2:
        binding, _ = find_program_address([b"rfid", deployment, event["old_rfid_hash"]], program_id)
        accounts = [
            meta(required_signer, True, False),
            meta(config, False, False),
            meta(animal, False, True),
            meta(binding, False, False),
            meta(INSTRUCTIONS_SYSVAR_ID, False, False),
        ]
        action_name = "transfer"
    elif event["action"] == 3:
        old_binding, _ = find_program_address([b"rfid", deployment, event["old_rfid_hash"]], program_id)
        new_binding, _ = find_program_address([b"rfid", deployment, event["new_rfid_hash"]], program_id)
        accounts = [
            meta(required_signer, True, True),
            meta(config, False, False),
            meta(animal, False, True),
            meta(old_binding, False, True),
            meta(new_binding, False, True),
            meta(INSTRUCTIONS_SYSVAR_ID, False, False),
            meta(SYSTEM_PROGRAM_ID, False, False),
        ]
        action_name = "reidentify"
    else:
        raise SystemContractError("unsupported StationEvent action")

    secp_data = bytearray(113)
    secp_data[0] = 1
    for index, value in enumerate((16, 0, 80, 0, 8, STATION_EVENT_LENGTH, 1)):
        struct.pack_into("<H", secp_data, 2 + index * 2, value)
    secp_data[16:80] = station_signature
    secp_data[80:113] = station_pubkey
    lastro_data = hashlib.sha256(f"global:{action_name}".encode()).digest()[:8] + raw_event
    return required_signer, [
        {
            "programId": SECP256R1_PROGRAM_ID,
            "accounts": [],
            "dataBase64": base64.b64encode(secp_data).decode(),
        },
        {
            "programId": program_id_text,
            "accounts": accounts,
            "dataBase64": base64.b64encode(lastro_data).decode(),
        },
    ]


def _transaction_int(value: Any, name: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise SystemContractError(f"finalized transaction {name} is invalid")
    return value


def verify_finalized_transaction_result(
    result: Any,
    tx_signature: str,
    required_signer: str,
    expected_instructions: list[dict[str, Any]],
) -> None:
    b58decode_exact(tx_signature, 64, "transaction signature")
    if not isinstance(result, dict):
        raise SystemContractError("evidence transaction is not finalized on canonical Solana")
    if result.get("version") not in (None, "legacy"):
        raise SystemContractError("evidence transaction is not the required legacy transaction version")
    meta = result.get("meta")
    if not isinstance(meta, dict) or "err" not in meta or meta["err"] is not None:
        raise SystemContractError("evidence transaction did not finalize successfully")
    transaction = result.get("transaction")
    if not isinstance(transaction, dict):
        raise SystemContractError("finalized transaction payload is invalid")
    signatures = transaction.get("signatures")
    if not isinstance(signatures, list) or signatures != [tx_signature]:
        raise SystemContractError("finalized transaction signature does not match EvidencePackage")
    message = transaction.get("message")
    if not isinstance(message, dict):
        raise SystemContractError("finalized transaction message is invalid")
    lookups = message.get("addressTableLookups")
    if lookups is not None and (not isinstance(lookups, list) or lookups):
        raise SystemContractError("evidence transaction unexpectedly uses address table lookups")
    account_keys = message.get("accountKeys")
    if not isinstance(account_keys, list) or not all(isinstance(value, str) for value in account_keys):
        raise SystemContractError("finalized transaction account keys are invalid")
    header = message.get("header")
    if not isinstance(header, dict):
        raise SystemContractError("finalized transaction header is invalid")
    required_signatures = _transaction_int(header.get("numRequiredSignatures"), "numRequiredSignatures")
    readonly_signed = _transaction_int(header.get("numReadonlySignedAccounts"), "numReadonlySignedAccounts")
    readonly_unsigned = _transaction_int(header.get("numReadonlyUnsignedAccounts"), "numReadonlyUnsignedAccounts")
    if required_signatures != 1 or readonly_signed > required_signatures or not account_keys or account_keys[0] != required_signer:
        raise SystemContractError("finalized transaction signer header does not match canonical custodian authority")
    if len(signatures) != required_signatures or readonly_unsigned > len(account_keys) - required_signatures:
        raise SystemContractError("finalized transaction account header is inconsistent")

    roles: dict[str, tuple[bool, bool]] = {required_signer: (True, True)}
    for instruction in expected_instructions:
        program = instruction["programId"]
        roles.setdefault(program, (False, False))
        for account in instruction["accounts"]:
            address_text = account["address"]
            current = roles.get(address_text, (False, False))
            roles[address_text] = (
                current[0] or account["isSigner"],
                current[1] or account["isWritable"],
            )
    if len(roles) != len(account_keys):
        raise SystemContractError("finalized transaction contains unexpected accounts")
    for position, account in enumerate(account_keys):
        signer = position < required_signatures
        writable = (
            position < required_signatures - readonly_signed
            if signer
            else position < len(account_keys) - readonly_unsigned
        )
        if roles.get(account) != (signer, writable):
            raise SystemContractError("finalized transaction account privileges do not match the Lastro envelope")

    instructions = message.get("instructions")
    if not isinstance(instructions, list) or len(instructions) != len(expected_instructions):
        raise SystemContractError("finalized transaction must contain exactly the frozen two-instruction Lastro envelope")
    for instruction_index, (actual, expected) in enumerate(zip(instructions, expected_instructions)):
        if not isinstance(actual, dict):
            raise SystemContractError("finalized transaction contains an invalid compiled instruction")
        program_index = _transaction_int(actual.get("programIdIndex"), "programIdIndex")
        if program_index >= len(account_keys) or account_keys[program_index] != expected["programId"]:
            raise SystemContractError(f"finalized transaction instruction {instruction_index} targets the wrong program")
        actual_accounts = actual.get("accounts")
        expected_accounts = expected["accounts"]
        if not isinstance(actual_accounts, list) or len(actual_accounts) != len(expected_accounts):
            raise SystemContractError(f"finalized transaction instruction {instruction_index} has the wrong account count")
        for actual_index, expected_account in zip(actual_accounts, expected_accounts):
            account_index = _transaction_int(actual_index, "account index")
            if account_index >= len(account_keys) or account_keys[account_index] != expected_account["address"]:
                raise SystemContractError(f"finalized transaction instruction {instruction_index} account order is invalid")
        data = actual.get("data")
        if not isinstance(data, str):
            raise SystemContractError("finalized transaction instruction data is invalid")
        expected_data = base64.b64decode(expected["dataBase64"], validate=True)
        if b58decode_exact(data, len(expected_data), "instruction data") != expected_data:
            raise SystemContractError(f"finalized transaction instruction {instruction_index} bytes do not match the exact StationEvent envelope")


def verify_evidence_package(package: dict[str, Any], rpc: SolanaRpcClient, program_id_text: str) -> list[dict[str, Any]]:
    if package.get("version") != 1:
        raise SystemContractError("EvidencePackage version is invalid")
    deployment = decode_hex_exact(package.get("deploymentId"), 32, "deploymentId")
    animal_id = decode_hex_exact(package.get("animalId"), 32, "animalId")
    program_id = b58decode_exact(program_id_text, 32, "LASTRO_PROGRAM_ID")
    events_json = package.get("events")
    if not isinstance(events_json, list) or not events_json:
        raise SystemContractError("EvidencePackage events must be non-empty")
    decoded: list[dict[str, Any]] = []
    previous_raw: bytes | None = None
    previous: dict[str, Any] | None = None
    station_keys: list[bytes] = []
    for index, entry in enumerate(events_json):
        if not isinstance(entry, dict):
            raise SystemContractError("EvidencePackage event must be an object")
        try:
            raw = base64.b64decode(entry["eventBytesBase64"], validate=True)
        except (KeyError, ValueError, TypeError) as error:
            raise SystemContractError(f"EvidencePackage event {index + 1} has invalid event bytes") from error
        if base64.b64encode(raw).decode() != entry["eventBytesBase64"] or len(raw) != 276:
            raise SystemContractError(f"EvidencePackage event {index + 1} event bytes are not canonical")
        event = decode_station_event(raw)
        if event["deployment_id"] != deployment or event["animal_id"] != animal_id:
            raise SystemContractError(f"EvidencePackage event {index + 1} belongs to a different identity")
        observed = decode_hex_exact(entry.get("observedRfidHex"), 8, "observedRfidHex")
        if rfid_hash(observed) != event["new_rfid_hash"]:
            raise SystemContractError(f"EvidencePackage event {index + 1} RFID evidence is invalid")
        pubkey = decode_hex_exact(entry.get("stationPubkeyHex"), 33, "stationPubkeyHex")
        signature = decode_hex_exact(entry.get("stationSignatureHex"), 64, "stationSignatureHex")
        if hashlib.sha256(b"LASTRO_STATION\x00" + pubkey).digest() != event["station_id"]:
            raise SystemContractError(f"EvidencePackage event {index + 1} StationID binding is invalid")
        verify_p256_compact_low_s(pubkey, signature, raw)
        tx_signature = entry.get("txSignature")
        if not isinstance(tx_signature, str) or not tx_signature:
            raise SystemContractError(f"EvidencePackage event {index + 1} is missing a finalized transaction signature")
        required_signer, expected_instructions = expected_event_transaction(
            raw, event, pubkey, signature, program_id_text
        )
        verify_finalized_transaction_result(
            rpc.finalized_transaction(tx_signature),
            tx_signature,
            required_signer,
            expected_instructions,
        )
        if index == 0:
            if event["action"] != 1 or event["event_sequence"] != 1 or event["identity_revision"] != 1:
                raise SystemContractError("EvidencePackage must begin with ORIGIN sequence/revision 1")
        else:
            assert_successor(previous, previous_raw, event)
        station_keys.append(pubkey)
        decoded.append(event)
        previous, previous_raw = event, raw

    config_address, config_bump = find_program_address([b"config", deployment], program_id)
    config = decode_account_data(rpc.account(b58encode(config_address)), program_id_text, 106)
    if config[:8] != anchor_discriminator("ProtocolConfig") or config[40:72] != deployment or config[105] != config_bump:
        raise SystemContractError("canonical ProtocolConfig does not match EvidencePackage deployment")
    canonical_station = config[72:105]
    if any(key != canonical_station for key in station_keys):
        raise SystemContractError("EvidencePackage Station key is not canonical ProtocolConfig Station key")

    animal_address, animal_bump = find_program_address([b"animal", deployment, animal_id], program_id)
    animal = decode_animal_account(rpc.account(b58encode(animal_address)), program_id_text)
    last = decoded[-1]
    last_raw = base64.b64decode(events_json[-1]["eventBytesBase64"], validate=True)
    if animal["bump"] != animal_bump:
        raise SystemContractError("canonical AnimalState PDA bump is invalid")
    terminal = {
        "animal_id": animal_id.hex(),
        "current_rfid_hash": last["new_rfid_hash"].hex(),
        "current_custodian": last["to_custodian"].hex(),
        "identity_revision": last["identity_revision"],
        "event_sequence": last["event_sequence"],
        "last_event_hash": hashlib.sha256(last_raw).hexdigest(),
    }
    for key, value in terminal.items():
        if animal[key] != value:
            raise SystemContractError(f"canonical AnimalState field {key} disagrees with EvidencePackage")

    seen_rfid: set[bytes] = set()
    for event in decoded:
        seen_rfid.add(event["new_rfid_hash"])
    for rfid in seen_rfid:
        binding_address, bump = find_program_address([b"rfid", deployment, rfid], program_id)
        binding = decode_binding_account(rpc.account(b58encode(binding_address)), program_id_text)
        expected_status = 1 if rfid == last["new_rfid_hash"] else 2
        if binding["bump"] != bump or binding["animal_id"] != animal_id.hex() or binding["rfid_hash"] != rfid.hex() or binding["status"] != expected_status:
            raise SystemContractError("canonical RfidBinding history disagrees with EvidencePackage")
    return decoded


def verify_p256_compact_low_s(pubkey: bytes, signature: bytes, message: bytes) -> None:
    if len(signature) != 64:
        raise SystemContractError("P-256 signature must be 64 bytes")
    r = int.from_bytes(signature[:32], "big")
    s = int.from_bytes(signature[32:], "big")
    if not (1 <= r < P256_ORDER and 1 <= s <= P256_ORDER // 2):
        raise SystemContractError("P-256 signature is not canonical low-S")
    try:
        public = ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), pubkey)
        public.verify(encode_dss_signature(r, s), message, ec.ECDSA(hashes.SHA256()))
    except ValueError as error:
        raise SystemContractError("Station public key is invalid") from error
    except Exception as error:
        raise SystemContractError("Station P-256 signature is invalid") from error


def assert_successor(previous: dict[str, Any] | None, previous_raw: bytes | None, current: dict[str, Any]) -> None:
    if previous is None or previous_raw is None:
        raise SystemContractError("EvidencePackage successor has no predecessor")
    if current["action"] == 1:
        raise SystemContractError("ORIGIN cannot appear after the first event")
    if current["deployment_id"] != previous["deployment_id"] or current["animal_id"] != previous["animal_id"]:
        raise SystemContractError("EvidencePackage identity changes across events")
    if current["event_sequence"] != previous["event_sequence"] + 1:
        raise SystemContractError("EvidencePackage event_sequence is not contiguous")
    if current["previous_event_hash"] != hashlib.sha256(previous_raw).digest():
        raise SystemContractError("EvidencePackage previous_event_hash is invalid")
    if current["old_rfid_hash"] != previous["new_rfid_hash"]:
        raise SystemContractError("EvidencePackage RFID continuity is invalid")
    if current["from_custodian"] != previous["to_custodian"]:
        raise SystemContractError("EvidencePackage custody continuity is invalid")
    if current["action"] == 2:
        if current["identity_revision"] != previous["identity_revision"]:
            raise SystemContractError("TRANSFER changed identity_revision")
        if current["new_rfid_hash"] != current["old_rfid_hash"]:
            raise SystemContractError("TRANSFER changed RFID")
        if current["from_custodian"] == current["to_custodian"]:
            raise SystemContractError("TRANSFER did not change custodian")
    elif current["action"] == 3:
        if current["identity_revision"] != previous["identity_revision"] + 1:
            raise SystemContractError("REIDENTIFY did not increment identity_revision exactly once")
        if current["new_rfid_hash"] == current["old_rfid_hash"]:
            raise SystemContractError("REIDENTIFY did not change RFID")
        if current["from_custodian"] != current["to_custodian"]:
            raise SystemContractError("REIDENTIFY changed custodian")
