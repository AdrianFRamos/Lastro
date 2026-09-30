"""v2 full-system harness: a real Agent, a PTY Station, real wallets and a real Solana cluster.

The harness plays only the roles outside the product boundary:

- the Station (the simulator's firmware-equivalent state machine behind a PTY, so the real
  Agent talks to it exactly as to USB serial);
- the wallets (Ed25519 keypair files; the harness signs the exact transaction the API prepared);
- the operator (operator-token API calls for parties/facilities/custody proposals).

Everything else — Agent, API, PostgreSQL, the v2 program and the RPC — is the real stack.
Evidence is verified independently of the API (envelopes, signatures, hash chain, RFID
transitions, and the canonical accounts/transactions at finalized commitment).
"""
from __future__ import annotations

import base64
import hashlib
import json
import os
import secrets
import struct
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request
import uuid
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Callable

import pytest
from cryptography.exceptions import InvalidSignature
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives.asymmetric.utils import encode_dss_signature

from lastro_hardware_simulator.protocol import MESSAGE_EVENT_READY, FrameDecoder, ProtocolError
from lastro_hardware_simulator.station import STATE_WAIT_RFID, SimulatedStation
from scripts.solana_tools import (
    SolanaRpcClient,
    WalletActor,
    anchor_discriminator,
    b58decode_exact,
    b58encode,
    compile_legacy_message,
    decode_account_data,
    encode_shortvec,
    find_program_address,
)

ROOT = Path(__file__).resolve().parents[2]
ZERO32 = bytes(32)
P256_ORDER = 0xFFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551
EVENT_OBSERVATION = 2
EVENT_BOUND = 18
EVENT_REPLACED = 19
RFID_ACTIVE = 1
RFID_RETIRED = 2
ANIMAL = 1
LIVE_WEIGHT_GRAMS = 450_000
FINALITY_TIMEOUT = float(os.getenv("LASTRO_SYSTEM_FINALITY_TIMEOUT_SECONDS", "90"))
STATION_TIMEOUT = float(os.getenv("LASTRO_SYSTEM_STATION_TIMEOUT_SECONDS", "30"))


class SystemContractError(AssertionError):
    pass


# --------------------------------------------------------------------------- environment


@dataclass(frozen=True)
class SystemEnvironment:
    api_url: str
    rpc_url: str
    program_id: str
    deployment_id: bytes
    agent_bin: Path
    agent_token: str
    operator_token: str
    station_scalar_hex: str
    authority: WalletActor
    wallet_b: WalletActor
    wallet_c: WalletActor


def _required(name: str) -> str:
    value = os.getenv(name, "").strip()
    if not value:
        pytest.fail(f"{name} is required for the v2 system gate")
    return value


def require_system_environment(flag: str = "LASTRO_SYSTEM_TEST") -> SystemEnvironment:
    """Skip unless `flag`=1; then every declared dependency is mandatory (fail, never skip)."""
    if os.getenv(flag) != "1":
        pytest.skip(f"set {flag}=1 with a deployed v2 stack (see docs/TESTING.md)")
    if os.name != "posix":
        pytest.fail("the PTY Station harness requires a POSIX host")
    deployment = bytes.fromhex(_required("LASTRO_DEPLOYMENT_ID_HEX"))
    if len(deployment) != 32:
        pytest.fail("LASTRO_DEPLOYMENT_ID_HEX must be 32 bytes")
    agent_bin = Path(_required("LASTRO_SYSTEM_AGENT_BIN"))
    if not agent_bin.is_file():
        pytest.fail(f"Agent binary not found: {agent_bin}")
    return SystemEnvironment(
        api_url=_required("LASTRO_SYSTEM_API_URL").rstrip("/"),
        rpc_url=_required("LASTRO_SYSTEM_SOLANA_RPC_URL"),
        program_id=_required("LASTRO_PROGRAM_ID"),
        deployment_id=deployment,
        agent_bin=agent_bin,
        agent_token=_required("LASTRO_AGENT_TOKEN"),
        operator_token=_required("LASTRO_OPERATOR_TOKEN"),
        station_scalar_hex=_required("LASTRO_SYSTEM_STATION_PRIVATE_SCALAR_HEX"),
        authority=WalletActor.from_solana_keypair(Path(_required("LASTRO_SYSTEM_WALLET_A_KEYPAIR"))),
        wallet_b=WalletActor.from_solana_keypair(Path(_required("LASTRO_SYSTEM_WALLET_B_KEYPAIR"))),
        wallet_c=WalletActor.from_solana_keypair(Path(_required("LASTRO_SYSTEM_WALLET_C_KEYPAIR"))),
    )


def fresh_rfid_hex() -> str:
    """RFIDs can never be reused on-chain, so every run reads brand-new tags."""
    return "80" + secrets.token_hex(7)


def rfid_hash(canonical_rfid: bytes) -> bytes:
    return hashlib.sha256(b"LASTRO_RFID\0" + canonical_rfid).digest()


def domain_hash(domain: bytes, data: bytes) -> bytes:
    return hashlib.sha256(domain + data).digest()


def wait_until(check: Callable[[], Any], timeout: float, what: str, interval: float = 0.5) -> Any:
    deadline = time.monotonic() + timeout
    while True:
        value = check()
        if value:
            return value
        if time.monotonic() >= deadline:
            raise SystemContractError(f"timed out after {timeout:.0f}s waiting for {what}")
        time.sleep(interval)


# --------------------------------------------------------------------------- HTTP API


class HttpStatusError(SystemContractError):
    def __init__(self, status: int, body: str, path: str):
        super().__init__(f"HTTP {status} for {path}: {body}")
        self.status = status
        self.body = body

    @property
    def message(self) -> str | None:
        try:
            return json.loads(self.body).get("message")
        except (ValueError, AttributeError):
            return None


class Api:
    def __init__(self, env: SystemEnvironment):
        self.env = env

    def call(self, method: str, path: str, body: Any = None, *, token: str | None = None) -> tuple[int, Any]:
        data = None if body is None else json.dumps(body).encode()
        headers = {"content-type": "application/json"}
        if token:
            headers["authorization"] = f"Bearer {token}"
        request = urllib.request.Request(self.env.api_url + path, data=data, method=method, headers=headers)
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                raw = response.read()
                return response.status, json.loads(raw) if raw else None
        except urllib.error.HTTPError as error:
            raise HttpStatusError(error.code, error.read().decode(errors="replace"), path) from None

    def get(self, path: str) -> Any:
        return self.call("GET", path)[1]

    def post(self, path: str, body: Any = None) -> Any:
        return self.call("POST", path, body)[1]

    def operator(self, path: str, body: Any) -> Any:
        return self.call("POST", path, body, token=self.env.operator_token)[1]

    def agent(self, path: str, body: Any) -> tuple[int, Any]:
        return self.call("POST", path, body, token=self.env.agent_token)

    def poll_conflict(self, path: str, body: Any, pending: set[str], what: str) -> Any:
        """POST until the API stops answering 409 with one of the `pending` finality messages."""

        def attempt():
            try:
                return self.post(path, body) or True
            except HttpStatusError as error:
                if error.status == 409 and error.message in pending:
                    return None
                raise
            except TimeoutError:
                return None  # submit/confirm are idempotent: retry a slow response

        return wait_until(attempt, FINALITY_TIMEOUT, what, interval=1.0)

    def asset(self, asset_id: str) -> dict | None:
        try:
            return self.get(f"/api/v2/assets/{asset_id}")
        except HttpStatusError as error:
            if error.status == 404:
                return None
            raise


# --------------------------------------------------------------------------- Station + Agent


class StationHarness:
    """The firmware-equivalent Station on the master side of a PTY the real Agent opens."""

    def __init__(self, private_scalar_hex: str):
        import pty
        import tty

        self._master, self._slave = pty.openpty()
        tty.setraw(self._slave)
        self.path = os.ttyname(self._slave)
        self.last_event_ready: bytes | None = None
        self.station = SimulatedStation(self._emit, private_scalar_hex)
        self._decoder = FrameDecoder()
        self._stop = threading.Event()
        self._thread = threading.Thread(target=self._pump, name="pty-station", daemon=True)

    def __enter__(self) -> "StationHarness":
        self._thread.start()
        return self

    def __exit__(self, *_exc) -> None:
        # Join before closing: a closed fd number is reused by the next open(), and a still
        # running pump would read (and consume) that unrelated file.
        self._stop.set()
        self._thread.join(timeout=5)
        os.close(self._slave)
        os.close(self._master)

    @property
    def public_key_hex(self) -> str:
        return self.station.public_key_hex

    def _emit(self, frame: bytes) -> bool:
        if frame[5] == MESSAGE_EVENT_READY:
            self.last_event_ready = frame[12:-4]
        os.write(self._master, frame)
        return True

    def _pump(self) -> None:
        import select

        while not self._stop.is_set():
            try:
                ready, _, _ = select.select([self._master], [], [], 0.2)
                if not ready:
                    continue
                data = os.read(self._master, 4096)
            except OSError:
                return
            try:
                for frame in self._decoder.feed(data):
                    self.station.handle_frame(frame)
            except ProtocolError:
                continue  # corrupt bytes are the Station's to reject; the Agent retries

    def read_tag(self, rfid_hex: str) -> None:
        """Wait for the Agent's COMMAND, then present the tag to the reader."""
        wait_until(
            lambda: self.station.snapshot(True)["station"]["state"] == STATE_WAIT_RFID,
            STATION_TIMEOUT,
            "the Agent to dispatch the capture COMMAND to the Station",
        )
        result = self.station.observe_rfid_hex(rfid_hex)
        if not result["accepted"]:
            raise SystemContractError(f"Station rejected the read: {result['reason']}")


class AgentProcess:
    """The real `lastro-agent` binary with its own durable SQLite outbox."""

    def __init__(self, env: SystemEnvironment, serial_path: str, station_pubkey_hex: str):
        self._dir = tempfile.TemporaryDirectory(prefix="lastro-agent-")
        self.log_path = Path(self._dir.name) / "agent.log"
        self._env = {
            **os.environ,
            "LASTRO_AGENT_API_URL": env.api_url,
            "LASTRO_AGENT_TOKEN": env.agent_token,
            "LASTRO_AGENT_SERIAL_PORT": serial_path,
            "LASTRO_AGENT_SERIAL_BAUD": "115200",
            "LASTRO_STATION_PUBKEY_HEX": station_pubkey_hex,
            "LASTRO_AGENT_SQLITE_URL": f"sqlite://{Path(self._dir.name) / 'agent.db'}?mode=rwc",
            "LASTRO_AGENT_POLL_INTERVAL_MS": "250",
            "LASTRO_AGENT_REQUEST_TIMEOUT_MS": "5000",
            "LASTRO_AGENT_STATION_RESPONSE_TIMEOUT_MS": str(int(STATION_TIMEOUT * 1000)),
        }
        self._binary = env.agent_bin
        self._process: subprocess.Popen | None = None

    def __enter__(self) -> "AgentProcess":
        log = self.log_path.open("wb")
        self._process = subprocess.Popen([str(self._binary)], env=self._env, stdout=log, stderr=subprocess.STDOUT)
        time.sleep(1.0)
        if self._process.poll() is not None:
            raise SystemContractError(f"Agent exited at startup:\n{self.log_path.read_text(errors='replace')}")
        return self

    def __exit__(self, *_exc) -> None:
        if self._process and self._process.poll() is None:
            self._process.terminate()
            try:
                self._process.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self._process.kill()
        artifacts = os.getenv("LASTRO_SYSTEM_ARTIFACT_DIR")
        if artifacts and self.log_path.exists():
            Path(artifacts).mkdir(parents=True, exist_ok=True)
            (Path(artifacts) / f"agent-{uuid.uuid4().hex[:8]}.log").write_bytes(self.log_path.read_bytes())
        self._dir.cleanup()


# --------------------------------------------------------------------------- flow


@dataclass
class CaptureResult:
    capture_id: str
    event_hash: str
    tx_signature: str
    event_ready: bytes


class LastroFlow:
    """One operator driving the real stack through the same API the web console uses."""

    def __init__(self, env: SystemEnvironment, station: StationHarness):
        self.env = env
        self.api = Api(env)
        self.rpc = SolanaRpcClient(env.rpc_url)
        self.station = station

    # -- wallets

    def sign_and_send(self, transaction_data: dict, wallet: WalletActor) -> str:
        """Sign exactly what the API prepared (after checking who must sign and which program)."""
        if transaction_data["requiredSigner"] != wallet.address:
            raise SystemContractError(
                f"API asked {transaction_data['requiredSigner']} to sign, expected {wallet.address}"
            )
        if transaction_data["lastroProgramId"] != self.env.program_id:
            raise SystemContractError("API prepared a transaction for a different program")
        message = compile_legacy_message(wallet.public_key, self.rpc.latest_blockhash(), transaction_data["instructions"])
        signature = wallet.private_key.sign(message)
        wire = encode_shortvec(1) + signature + message
        if len(wire) != transaction_data["measuredSerializedBytes"]:
            raise SystemContractError(
                f"API measured {transaction_data['measuredSerializedBytes']} bytes, compiled {len(wire)}"
            )
        returned = self.rpc.send_transaction(wire)
        if returned != b58encode(signature):
            raise SystemContractError("RPC returned a different transaction signature")
        return returned

    def wait_finalized(self, signature: str) -> None:
        status = self.rpc.wait_signature(signature, timeout=FINALITY_TIMEOUT)
        if status.get("err") is not None:
            raise SystemContractError(f"transaction {signature} failed: {status['err']}")

    # -- operations

    def register_asset(self, custodian: WalletActor) -> str:
        asset_id = secrets.token_hex(32)
        tx = self.api.post(
            "/api/v2/assets/transaction-data",
            {
                "assetId": asset_id,
                "custodian": custodian.public_key.hex(),
                "assetType": ANIMAL,
                "availableWeightGrams": LIVE_WEIGHT_GRAMS,
            },
        )
        self.wait_finalized(self.sign_and_send(tx, self.env.authority))
        wait_until(lambda: self.api.asset(asset_id), FINALITY_TIMEOUT, "the finalized AssetState")
        return asset_id

    def authorize_capture(self, action: str, asset_id: str, wallet: WalletActor) -> dict:
        challenge = self.api.post(
            "/api/captures/authorization-challenge", {"action": action, "assetId": asset_id}
        )
        message = base64.b64decode(challenge["messageBase64"], validate=True)
        proof = {
            "challengeId": challenge["challengeId"],
            "signatureBase64": base64.b64encode(wallet.private_key.sign(message)).decode(),
        }
        try:
            status, capture = self.api.call(
                "POST", "/api/captures", {"action": action, "assetId": asset_id, "authorization": proof}
            )
        except HttpStatusError as error:
            status, capture = error.status, None
        return {"challenge": challenge, "status": status, "capture": capture}

    def capture(self, action: str, asset_id: str, wallet: WalletActor, rfid_hex: str) -> CaptureResult:
        """Wallet authorization → Station read → Agent evidence → wallet transaction → finality."""
        authorized = self.authorize_capture(action, asset_id, wallet)
        if authorized["challenge"]["requiredSigner"] != wallet.address:
            raise SystemContractError(f"{action} requires {authorized['challenge']['requiredSigner']}")
        if authorized["capture"] is None:
            raise SystemContractError(f"{action} capture was refused with HTTP {authorized['status']}")
        capture_id = authorized["capture"]["captureId"]
        self.station.read_tag(rfid_hex)
        accepted = wait_until(
            lambda: (lambda c: c if c["status"] == "EVIDENCE_ACCEPTED" else None)(
                self.api.get(f"/api/captures/{capture_id}")
            ),
            STATION_TIMEOUT,
            "the Agent to deliver the Station evidence",
        )
        event_hash = accepted["eventHash"]
        event_ready = self.station.last_event_ready or b""
        tx = self.api.get(f"/api/v2/events/{event_hash}/transaction-data")
        signature = self.sign_and_send(tx, wallet)
        self.api.poll_conflict(
            f"/api/v2/events/{event_hash}/submit",
            {"txSignature": signature},
            {"transaction is not the confirmed exact transaction for this v2 event"},
            "confirmed transaction registration",
        )
        final = self.api.poll_conflict(
            f"/api/v2/events/{event_hash}/confirm",
            {"txSignature": signature},
            {
                "transaction is not the finalized exact transaction for this v2 event",
                "finalized transaction did not produce EventAnchor",
            },
            "finalized confirmation",
        )
        if final["status"] != "FINALIZED":
            raise SystemContractError(f"event ended as {final['status']}")
        return CaptureResult(capture_id, event_hash, signature, event_ready)

    def transfer_custody(self, asset_id: str, current: WalletActor, recipient: WalletActor) -> str:
        party_id, facility_id = secrets.token_hex(32), secrets.token_hex(32)
        self.api.operator(
            "/api/v2/parties",
            {"partyId": party_id, "legalName": "System harness recipient", "taxIdHash": None,
             "wallet": recipient.public_key.hex(), "role": 1},
        )
        self.api.operator(
            "/api/v2/facilities",
            {"facilityId": facility_id, "ownerPartyId": party_id, "facilityType": 1,
             "displayName": "System harness farm", "credentialHash": secrets.token_hex(32),
             "validFrom": 0, "validUntil": 4102444800},
        )
        transfer_id = str(uuid.uuid4())
        self.api.operator(
            "/api/v2/custody-transfers",
            {"transferId": transfer_id, "assetId": asset_id, "fromPartyId": None, "toPartyId": party_id,
             "fromFacilityId": None, "toFacilityId": facility_id, "reason": "System harness transfer",
             "createdByPartyId": None},
        )
        base = f"/api/v2/custody-transfers/{transfer_id}"
        self.wait_finalized(self.sign_and_send(self.api.get(f"{base}/transaction-data?phase=propose"), current))
        accept_signature = self.sign_and_send(self.api.get(f"{base}/transaction-data?phase=accept"), recipient)
        self.wait_finalized(accept_signature)
        self.api.poll_conflict(
            f"{base}/accept",
            {"txSignature": accept_signature},
            {
                "finalized Solana state does not show the recipient as custodian",
                "transaction is not the finalized recipient acceptance of this transfer",
            },
            "custody acceptance",
        )
        return accept_signature


def run_full_flow(env: SystemEnvironment, flow: LastroFlow) -> dict:
    """register → bind(tag A) → presence(tag A) → replace(tag B) → custody B→C, all finalized.

    Every capture is authorized by the current custodian (B); A only registers the asset.
    """
    tag_a, tag_b = fresh_rfid_hex(), fresh_rfid_hex()
    asset_id = flow.register_asset(env.wallet_b)
    bind = flow.capture("BIND_IDENTIFIER", asset_id, env.wallet_b, tag_a)
    observe = flow.capture("OBSERVE_PRESENCE", asset_id, env.wallet_b, tag_a)
    replace = flow.capture("REPLACE_IDENTIFIER", asset_id, env.wallet_b, tag_b)
    custody = flow.transfer_custody(asset_id, env.wallet_b, env.wallet_c)
    return {
        "asset_id": asset_id,
        "tag_a": tag_a,
        "tag_b": tag_b,
        "captures": [bind, observe, replace],
        "custody_signature": custody,
    }


# --------------------------------------------------------------------------- verification


def _verify_p256(pubkey: bytes, signature: bytes, message: bytes) -> bool:
    r, s = int.from_bytes(signature[:32], "big"), int.from_bytes(signature[32:], "big")
    if not (1 <= r < P256_ORDER and 1 <= s <= P256_ORDER // 2):
        return False
    try:
        key = ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), pubkey)
        key.verify(encode_dss_signature(r, s), message, ec.ECDSA(hashes.SHA256()))
        return True
    except (InvalidSignature, ValueError):
        return False


def verify_package_locally(package: dict) -> list[str]:
    """Recompute every claim of a `lastro.evidence-package.v2` from the signed envelopes."""
    problems: list[str] = []
    if package.get("schema") != "lastro.evidence-package.v2":
        return ["unsupported package schema"]
    asset = package["asset"]
    deployment, asset_id = bytes.fromhex(package["deploymentId"]), bytes.fromhex(asset["assetId"])
    previous, version, current_rfid = ZERO32, 0, ZERO32
    for index, event in enumerate(package["events"], start=1):
        envelope = base64.b64decode(event["envelopeBytesBase64"], validate=True)
        if len(envelope) != 220:
            problems.append(f"event {index}: envelope is not 220 bytes")
            continue
        event_type = struct.unpack_from("<H", envelope, 2)[0]
        state_version = struct.unpack_from("<Q", envelope, 100)[0]
        digest = domain_hash(b"LASTRO_V2_EVENT\0", envelope)
        pubkey = bytes.fromhex(event["stationPubkeyHex"])
        if digest.hex() != event["eventHash"]:
            problems.append(f"event {index}: eventHash differs from the envelope")
        if envelope[4:36] != deployment or envelope[36:68] != asset_id:
            problems.append(f"event {index}: belongs to another deployment or asset")
        if envelope[108:140] != previous or state_version <= version:
            problems.append(f"event {index}: breaks the hash chain or version order")
        if envelope[172:204] != domain_hash(b"LASTRO_STATION\0", pubkey):
            problems.append(f"event {index}: sourceId is not the Station key")
        if not _verify_p256(pubkey, bytes.fromhex(event["stationSignatureHex"]), envelope):
            problems.append(f"event {index}: Station signature is invalid")
        if event["observedRfidHex"] is not None:
            observed = rfid_hash(bytes.fromhex(event["observedRfidHex"]))
            old = {EVENT_BOUND: ZERO32, EVENT_REPLACED: current_rfid, EVENT_OBSERVATION: observed}.get(event_type)
            if old is None:
                problems.append(f"event {index}: observed RFID on a non-capture event")
            else:
                if (event_type == EVENT_BOUND and current_rfid != ZERO32) or (
                    event_type == EVENT_REPLACED and observed == current_rfid
                ) or (event_type == EVENT_OBSERVATION and observed != current_rfid):
                    problems.append(f"event {index}: violates the RFID transition rule")
                if domain_hash(b"LASTRO_V2_PAYLOAD\0", old + observed) != envelope[140:172]:
                    problems.append(f"event {index}: observed RFID does not match the signed payload")
                current_rfid = observed
        previous, version = digest, state_version
    if asset["lastEventHash"] != previous.hex():
        problems.append("asset last event hash is not the end of the history")
    if bytes.fromhex(asset["currentRfidHash"] or "00" * 32) != current_rfid:
        problems.append("asset current RFID is not where the history ends")
    transfers = package["custodyTransfers"]
    if transfers and transfers[-1]["newCustodian"] != asset["custodian"]:
        problems.append("last custody transfer is not the current custodian")
    return problems


def verify_package_on_chain(package: dict, rpc: SolanaRpcClient, program_id: str, authority: WalletActor) -> list[str]:
    """Compare the package with ProtocolConfigV2, AssetState, EventAnchors, RfidBindings and txs."""
    problems: list[str] = []
    program = b58decode_exact(program_id, 32, "program ID")
    deployment = bytes.fromhex(package["deploymentId"])
    asset = package["asset"]
    asset_id = bytes.fromhex(asset["assetId"])

    def account(seeds: list[bytes], name: str, length: int) -> bytes:
        address, bump = find_program_address(seeds, program)
        data = decode_account_data(rpc.account(b58encode(address)), program_id, length)
        if data[:8] != anchor_discriminator(name) or data[length - 1] != bump:
            raise SystemContractError(f"{name} layout or bump is invalid")
        return data

    config = account([b"config-v2", deployment], "ProtocolConfigV2", 221)
    if config[8:40] != authority.public_key:
        problems.append("ProtocolConfigV2 authority is not the pinned deployment authority")
    state = account([b"asset", deployment, asset_id], "AssetState", 341)
    if state[74:106].hex() != asset["custodian"]:
        problems.append("canonical custodian differs from the package")
    if state[234:266].hex() != asset["lastEventHash"]:
        problems.append("canonical last event hash differs from the package")
    if state[308:340] != bytes.fromhex(asset["currentRfidHash"] or "00" * 32):
        problems.append("canonical current RFID differs from the package")

    previous_rfid = ZERO32
    for index, event in enumerate(package["events"], start=1):
        anchor = account([b"event", deployment, bytes.fromhex(event["eventId"])], "EventAnchor", 259)
        if anchor[226:258].hex() != event["eventHash"] or anchor[72:104] != asset_id:
            problems.append(f"event {index}: EventAnchor differs from the package")
        if event["observedRfidHex"] is not None:
            observed = rfid_hash(bytes.fromhex(event["observedRfidHex"]))
            if event["eventType"] == EVENT_REPLACED:
                retired = account([b"rfid", deployment, previous_rfid], "RfidBinding", 74)
                if retired[8:40] != asset_id or retired[72] != RFID_RETIRED:
                    problems.append(f"event {index}: replaced RFID is not RETIRED for this asset")
            previous_rfid = observed
        problems.extend(_finalized_success(rpc, event["txSignature"], program_id, f"event {index}"))
    if asset["currentRfidHash"]:
        active = account([b"rfid", deployment, bytes.fromhex(asset["currentRfidHash"])], "RfidBinding", 74)
        if active[8:40] != asset_id or active[72] != RFID_ACTIVE:
            problems.append("current RFID binding is not ACTIVE for this asset")
    for index, transfer in enumerate(package["custodyTransfers"], start=1):
        problems.extend(_finalized_success(rpc, transfer["txSignature"], program_id, f"custody {index}"))
        tx = rpc.finalized_transaction(transfer["txSignature"])
        if tx and tx["transaction"]["message"]["accountKeys"][0] != b58encode(bytes.fromhex(transfer["newCustodian"])):
            problems.append(f"custody {index}: not signed by the recipient")
    return problems


def _finalized_success(rpc: SolanaRpcClient, signature: str, program_id: str, label: str) -> list[str]:
    tx = rpc.finalized_transaction(signature)
    if tx is None:
        return [f"{label}: transaction is not finalized"]
    if tx["meta"]["err"] is not None:
        return [f"{label}: transaction failed"]
    if program_id not in tx["transaction"]["message"]["accountKeys"]:
        return [f"{label}: transaction does not invoke the Lastro program"]
    return []
