from __future__ import annotations

from dataclasses import asdict, dataclass
from datetime import datetime, timezone
import os
from threading import RLock
from typing import Callable, Final

from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import ec
from cryptography.hazmat.primitives.asymmetric.utils import decode_dss_signature

from .protocol import (
    ACTION_NAMES,
    MESSAGE_ACK,
    MESSAGE_COMMAND,
    MESSAGE_ERROR,
    MESSAGE_EVENT_READY,
    Frame,
    ProtocolError,
    StationCommand,
    canonical_rfid_from_hex,
    decode_ack,
    decode_command,
    derive_station_id,
    encode_error,
    encode_event_ready,
    encode_frame,
    encode_station_event,
    event_hash,
)

P256_ORDER: Final = int("FFFFFFFF00000000FFFFFFFFFFFFFFFFBCE6FAADA7179E84F3B9CAC2FC632551", 16)
DEFAULT_PRIVATE_SCALAR_HEX: Final = "0" * 63 + "1"
DEFAULT_PUBLIC_KEY_HEX: Final = (
    "036b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
)

STATE_IDLE: Final = "IDLE"
STATE_WAIT_RFID: Final = "WAIT_RFID"
STATE_WAIT_ACK: Final = "WAIT_ACK"

ERROR_INVALID_COMMAND: Final = 1
ERROR_RFID_READ_FAILED: Final = 2
ERROR_INVALID_EVENT_CONTEXT: Final = 3
ERROR_SIGNING_FAILED: Final = 4
ERROR_BUSY: Final = 5

ERROR_NAMES: Final = {
    ERROR_INVALID_COMMAND: "INVALID_COMMAND",
    ERROR_RFID_READ_FAILED: "RFID_READ_FAILED",
    ERROR_INVALID_EVENT_CONTEXT: "INVALID_EVENT_CONTEXT",
    ERROR_SIGNING_FAILED: "SIGNING_FAILED",
    ERROR_BUSY: "BUSY",
}


@dataclass
class FaultState:
    busy_next_command: bool = False
    reader_failure_next_observation: bool = False
    signing_failure_next_observation: bool = False
    corrupt_next_event_ready_crc: bool = False


@dataclass(frozen=True)
class LogEntry:
    at: str
    kind: str
    message: str
    detail: str | None = None


class DevelopmentSigner:
    def __init__(self, private_scalar_hex: str | None = None) -> None:
        raw = (private_scalar_hex or DEFAULT_PRIVATE_SCALAR_HEX).strip().lower()
        if len(raw) != 64:
            raise ValueError("simulator P-256 private scalar must be exactly 32 bytes of hex")
        try:
            scalar = int(raw, 16)
        except ValueError as error:
            raise ValueError("simulator P-256 private scalar must be hexadecimal") from error
        if scalar <= 0 or scalar >= P256_ORDER:
            raise ValueError("simulator P-256 private scalar must be in the valid P-256 scalar range")
        self._private_key = ec.derive_private_key(scalar, ec.SECP256R1())
        self.public_key = self._private_key.public_key().public_bytes(
            serialization.Encoding.X962,
            serialization.PublicFormat.CompressedPoint,
        )
        self.station_id = derive_station_id(self.public_key)
        self.uses_frozen_fixture_key = raw == DEFAULT_PRIVATE_SCALAR_HEX

    def sign_event(self, event_bytes: bytes) -> bytes:
        der = self._private_key.sign(event_bytes, ec.ECDSA(hashes.SHA256()))
        r, s = decode_dss_signature(der)
        if s > P256_ORDER // 2:
            s = P256_ORDER - s
        return r.to_bytes(32, "big") + s.to_bytes(32, "big")


class SimulatedStation:
    """State machine mirroring the firmware Station boundary, not the Lastro backend."""

    def __init__(self, emit: Callable[[bytes], bool], private_scalar_hex: str | None = None) -> None:
        self._emit = emit
        self._signer = DevelopmentSigner(private_scalar_hex)
        self._lock = RLock()
        self._state = STATE_IDLE
        self._command: StationCommand | None = None
        self._command_payload: bytes | None = None
        self._event_ready_payload: bytes | None = None
        self._event_hash: bytes | None = None
        self._last_observed_rfid: bytes | None = None
        self._faults = FaultState()
        self._logs: list[LogEntry] = []
        self._log("station", "Simulator Station initialized", self._station_identity_detail())

    @property
    def public_key_hex(self) -> str:
        return self._signer.public_key.hex()

    def _station_identity_detail(self) -> str:
        notice = "frozen test-only scalar 1" if self._signer.uses_frozen_fixture_key else "custom simulator scalar"
        return f"P-256 {notice}; public key {self.public_key_hex}"

    def set_fault(self, name: str, enabled: bool = True) -> None:
        with self._lock:
            if not hasattr(self._faults, name):
                raise ValueError(f"unknown fault {name}")
            setattr(self._faults, name, bool(enabled))
            self._log("fault", f"Fault {'armed' if enabled else 'cleared'}: {name}")

    def reset(self) -> None:
        with self._lock:
            self._clear_capture()
            self._faults = FaultState()
            self._log("station", "Station reset to IDLE")

    def handle_frame(self, frame: Frame) -> None:
        with self._lock:
            if frame.message_type == MESSAGE_COMMAND:
                self._handle_command(frame.payload)
                return
            if frame.message_type == MESSAGE_ACK:
                self._handle_ack(frame.payload)
                return
            self._log(
                "protocol",
                "Rejected inbound Station-only message type",
                f"message_type={frame.message_type}",
            )

    def _handle_command(self, payload: bytes) -> None:
        capture_id = payload[:16] if len(payload) >= 16 else b"\x00" * 16
        if self._faults.busy_next_command:
            self._faults.busy_next_command = False
            self._send_error(capture_id, ERROR_BUSY)
            return

        try:
            command = decode_command(payload)
        except ProtocolError as error:
            self._send_error(capture_id, ERROR_INVALID_COMMAND, str(error))
            return

        if self._state != STATE_IDLE:
            if payload == self._command_payload:
                if self._state == STATE_WAIT_ACK and self._event_ready_payload is not None:
                    self._log("protocol", "Repeated identical COMMAND; replaying signed EVENT_READY")
                    self._send_event_ready(self._event_ready_payload)
                else:
                    self._log("protocol", "Repeated identical COMMAND accepted while waiting for RFID")
                return
            self._send_error(command.capture_id, ERROR_BUSY)
            return

        self._command = command
        self._command_payload = bytes(payload)
        self._event_ready_payload = None
        self._event_hash = None
        self._last_observed_rfid = None
        self._state = STATE_WAIT_RFID
        self._log(
            "capture",
            f"{command.action_name} command accepted; waiting for physical RFID evidence",
            f"sequence={command.event_sequence}, revision={command.identity_revision}",
        )

    def observe_rfid_hex(self, value: str, label: str | None = None) -> dict[str, object]:
        rfid = canonical_rfid_from_hex(value)
        with self._lock:
            display = label.strip() if label and label.strip() else rfid.hex()
            if self._state != STATE_WAIT_RFID or self._command is None:
                self._log(
                    "reader",
                    "RFID observation ignored because no capture is waiting for evidence",
                    display,
                )
                return {"accepted": False, "reason": "NO_ACTIVE_CAPTURE"}

            command = self._command
            self._last_observed_rfid = rfid
            self._log("reader", "Validated reader-adapter output observed", f"{display} · {rfid.hex()}")

            if self._faults.reader_failure_next_observation:
                self._faults.reader_failure_next_observation = False
                self._fail_current_capture(ERROR_RFID_READ_FAILED, "Injected reader failure")
                return {"accepted": False, "reason": "RFID_READ_FAILED"}

            try:
                event_bytes = encode_station_event(command, self._signer.station_id, rfid)
            except ProtocolError as error:
                self._fail_current_capture(ERROR_INVALID_EVENT_CONTEXT, str(error))
                return {"accepted": False, "reason": "INVALID_EVENT_CONTEXT"}

            if self._faults.signing_failure_next_observation:
                self._faults.signing_failure_next_observation = False
                self._fail_current_capture(ERROR_SIGNING_FAILED, "Injected signing failure")
                return {"accepted": False, "reason": "SIGNING_FAILED"}

            signature = self._signer.sign_event(event_bytes)
            self._event_hash = event_hash(event_bytes)
            self._event_ready_payload = encode_event_ready(
                command.capture_id,
                event_bytes,
                rfid,
                self._signer.public_key,
                signature,
            )
            self._state = STATE_WAIT_ACK
            self._log(
                "station",
                "StationEvent built and signed; EVENT_READY emitted",
                f"event_hash={self._event_hash.hex()}",
            )
            self._send_event_ready(self._event_ready_payload)
            return {"accepted": True, "reason": "EVENT_READY"}

    def _handle_ack(self, payload: bytes) -> None:
        try:
            ack = decode_ack(payload)
        except ProtocolError as error:
            self._log("protocol", "Rejected malformed ACK", str(error))
            return

        if (
            self._state != STATE_WAIT_ACK
            or self._command is None
            or self._event_hash is None
            or ack.capture_id != self._command.capture_id
            or ack.event_hash != self._event_hash
        ):
            self._log("protocol", "Rejected ACK that does not match active signed evidence")
            return

        self._log("capture", "Matching ACK received; capture completed and Station returned to IDLE")
        self._clear_capture()

    def _send_event_ready(self, payload: bytes) -> None:
        encoded = bytearray(encode_frame(MESSAGE_EVENT_READY, payload))
        if self._faults.corrupt_next_event_ready_crc:
            self._faults.corrupt_next_event_ready_crc = False
            encoded[-1] ^= 0x01
            self._log("fault", "Corrupted next EVENT_READY CRC32C before transport")
        if not self._emit(bytes(encoded)):
            self._log("transport", "EVENT_READY is ready but no Agent wire connection is available")

    def _send_error(self, capture_id: bytes, code: int, detail: str | None = None) -> None:
        payload = encode_error(capture_id if len(capture_id) == 16 else b"\x00" * 16, code)
        self._log("station", f"Station emitted ERROR {ERROR_NAMES[code]}", detail)
        self._emit(encode_frame(MESSAGE_ERROR, payload))

    def _fail_current_capture(self, code: int, detail: str) -> None:
        capture_id = self._command.capture_id if self._command is not None else b"\x00" * 16
        self._clear_capture()
        self._send_error(capture_id, code, detail)

    def _clear_capture(self) -> None:
        self._state = STATE_IDLE
        self._command = None
        self._command_payload = None
        self._event_ready_payload = None
        self._event_hash = None
        self._last_observed_rfid = None

    def _log(self, kind: str, message: str, detail: str | None = None) -> None:
        self._logs.append(
            LogEntry(
                at=datetime.now(timezone.utc).isoformat(timespec="milliseconds"),
                kind=kind,
                message=message,
                detail=detail,
            )
        )
        del self._logs[:-120]

    def log_transport(self, message: str, detail: str | None = None) -> None:
        with self._lock:
            self._log("transport", message, detail)

    def snapshot(self, wire_connected: bool) -> dict[str, object]:
        with self._lock:
            command = self._command
            active_command = None
            if command is not None:
                active_command = {
                    "captureIdHex": command.capture_id.hex(),
                    "action": command.action_name,
                    "animalIdHex": command.animal_id.hex(),
                    "deploymentIdHex": command.deployment_id.hex(),
                    "eventSequence": command.event_sequence,
                    "identityRevision": command.identity_revision,
                    "previousEventHashHex": command.previous_event_hash.hex(),
                    "expectedOldRfidHashHex": command.expected_old_rfid_hash.hex(),
                    "fromCustodianHex": command.from_custodian.hex(),
                    "toCustodianHex": command.to_custodian.hex(),
                }
            return {
                "station": {
                    "state": self._state,
                    "publicKeyHex": self._signer.public_key.hex(),
                    "stationIdHex": self._signer.station_id.hex(),
                    "signer": "P-256 / SHA-256 / compact low-S",
                    "developmentKey": self._signer.uses_frozen_fixture_key,
                },
                "reader": {
                    "technology": "FDX-B",
                    "activationFrequency": "134.2 kHz",
                    "profile": "Generic ISO 11784/11785 adapter boundary (uncalibrated)",
                    "canonicalOutput": "8-byte unsigned big-endian Lastro RFID value",
                    "lastObservedRfidHex": self._last_observed_rfid.hex() if self._last_observed_rfid else None,
                },
                "transport": {
                    "wireConnected": wire_connected,
                    "protocol": "LSTR v1",
                    "messages": "COMMAND / EVENT_READY / ACK / ERROR",
                },
                "activeCommand": active_command,
                "faults": asdict(self._faults),
                "logs": [asdict(entry) for entry in reversed(self._logs[-40:])],
            }


def configured_private_scalar() -> str:
    return os.environ.get("LASTRO_HARDWARE_SIM_PRIVATE_SCALAR_HEX", DEFAULT_PRIVATE_SCALAR_HEX)
