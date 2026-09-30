from __future__ import annotations

from dataclasses import dataclass
import hashlib
import struct
from typing import Final

MAGIC: Final = b"LSTR"
VERSION: Final = 1
HEADER_LEN: Final = 12
CRC_LEN: Final = 4
MAX_PAYLOAD: Final = 1024

MESSAGE_COMMAND: Final = 1
MESSAGE_EVENT_READY: Final = 2
MESSAGE_ACK: Final = 3
MESSAGE_ERROR: Final = 4

COMMAND_PAYLOAD_LEN: Final = 202
EVENT_READY_PAYLOAD_LEN: Final = 341
ACK_PAYLOAD_LEN: Final = 48
ERROR_PAYLOAD_LEN: Final = 20
ENVELOPE_LEN: Final = 220
CANONICAL_RFID_LEN: Final = 8
SCHEMA_VERSION: Final = 1
MAX_EVENT_AGE_SECONDS: Final = 86_400

RFID_DOMAIN: Final = b"LASTRO_RFID\0"
STATION_DOMAIN: Final = b"LASTRO_STATION\0"
PAYLOAD_DOMAIN: Final = b"LASTRO_V2_PAYLOAD\0"
EVENT_DOMAIN: Final = b"LASTRO_V2_EVENT\0"
CAPTURE_EVENT_DOMAIN: Final = b"LASTRO_V2_CAPTURE_EVENT\0"
ZERO32: Final = b"\x00" * 32

EVENT_OBSERVATION_RECORDED: Final = 2
EVENT_IDENTIFIER_BOUND: Final = 18
EVENT_IDENTIFIER_REPLACED: Final = 19

EXPECTED_PAYLOAD_LENGTHS: Final = {
    MESSAGE_COMMAND: COMMAND_PAYLOAD_LEN,
    MESSAGE_EVENT_READY: EVENT_READY_PAYLOAD_LEN,
    MESSAGE_ACK: ACK_PAYLOAD_LEN,
    MESSAGE_ERROR: ERROR_PAYLOAD_LEN,
}

EVENT_TYPE_NAMES: Final = {
    EVENT_OBSERVATION_RECORDED: "OBSERVATION_RECORDED",
    EVENT_IDENTIFIER_BOUND: "IDENTIFIER_BOUND",
    EVENT_IDENTIFIER_REPLACED: "IDENTIFIER_REPLACED",
}


class ProtocolError(ValueError):
    pass


@dataclass(frozen=True)
class Frame:
    message_type: int
    payload: bytes


def capture_event_id(capture_id: bytes) -> bytes:
    return hashlib.sha256(CAPTURE_EVENT_DOMAIN + capture_id).digest()


@dataclass(frozen=True)
class StationCommand:
    """v2 capture command (see docs/MIGRACAO_V1_PARA_V2.md)."""

    capture_id: bytes
    event_type: int
    deployment_id: bytes
    asset_id: bytes
    event_id: bytes
    state_version: int
    previous_event_hash: bytes
    expected_rfid_hash: bytes
    observed_at: int
    expires_at: int

    @property
    def event_type_name(self) -> str:
        return EVENT_TYPE_NAMES.get(self.event_type, "UNKNOWN")

    def validate(self) -> None:
        if len(self.capture_id) != 16 or self.capture_id == b"\x00" * 16:
            raise ProtocolError("capture_id must be a non-zero 16-byte UUID")
        expects_tag = self.expected_rfid_hash != ZERO32
        if self.event_type == EVENT_IDENTIFIER_BOUND:
            type_ok = not expects_tag
        elif self.event_type in (EVENT_IDENTIFIER_REPLACED, EVENT_OBSERVATION_RECORDED):
            type_ok = expects_tag
        else:
            type_ok = False
        if (
            not type_ok
            or self.event_id != capture_event_id(self.capture_id)
            or self.state_version == 0
        ):
            raise ProtocolError(f"invalid command semantics for event type {self.event_type}")


@dataclass(frozen=True)
class AckPayload:
    capture_id: bytes
    event_hash: bytes


def crc32c(data: bytes) -> int:
    """CRC-32C/Castagnoli, reflected polynomial 0x82F63B78."""
    crc = 0xFFFFFFFF
    for value in data:
        crc ^= value
        for _ in range(8):
            crc = (crc >> 1) ^ (0x82F63B78 if crc & 1 else 0)
    return crc ^ 0xFFFFFFFF


def encode_frame(message_type: int, payload: bytes) -> bytes:
    expected = EXPECTED_PAYLOAD_LENGTHS.get(message_type)
    if expected is None:
        raise ProtocolError(f"unknown message type {message_type}")
    if len(payload) != expected:
        raise ProtocolError(
            f"message type {message_type} payload must be {expected} bytes, got {len(payload)}"
        )
    if len(payload) > MAX_PAYLOAD:
        raise ProtocolError("payload exceeds serial maximum")

    header = MAGIC + bytes([VERSION, message_type]) + b"\x00\x00" + struct.pack("<I", len(payload))
    covered = header[4:] + payload
    return header + payload + struct.pack("<I", crc32c(covered))


class FrameDecoder:
    def __init__(self) -> None:
        self._buffer = bytearray()

    def feed(self, data: bytes) -> list[Frame]:
        self._buffer.extend(data)
        out: list[Frame] = []
        while True:
            frame = self._next()
            if frame is None:
                return out
            out.append(frame)

    def _next(self) -> Frame | None:
        self._resynchronize()
        if len(self._buffer) < HEADER_LEN:
            return None

        if self._buffer[4] != VERSION:
            version = self._buffer[4]
            del self._buffer[0]
            raise ProtocolError(f"unsupported serial protocol version {version}")

        message_type = self._buffer[5]
        expected = EXPECTED_PAYLOAD_LENGTHS.get(message_type)
        if expected is None:
            del self._buffer[0]
            raise ProtocolError(f"unknown serial message type {message_type}")

        if self._buffer[6:8] != b"\x00\x00":
            del self._buffer[0]
            raise ProtocolError("serial reserved bytes must be zero")

        payload_len = struct.unpack_from("<I", self._buffer, 8)[0]
        if payload_len > MAX_PAYLOAD:
            del self._buffer[0]
            raise ProtocolError(f"serial payload length {payload_len} exceeds {MAX_PAYLOAD}")
        if payload_len != expected:
            del self._buffer[0]
            raise ProtocolError(
                f"serial message type {message_type} payload length must be {expected}, got {payload_len}"
            )

        frame_len = HEADER_LEN + payload_len + CRC_LEN
        if len(self._buffer) < frame_len:
            return None

        expected_crc = struct.unpack_from("<I", self._buffer, HEADER_LEN + payload_len)[0]
        actual_crc = crc32c(bytes(self._buffer[4 : HEADER_LEN + payload_len]))
        if expected_crc != actual_crc:
            del self._buffer[0]
            raise ProtocolError("serial CRC32C mismatch")

        frame = bytes(self._buffer[:frame_len])
        del self._buffer[:frame_len]
        return Frame(message_type=message_type, payload=frame[HEADER_LEN : HEADER_LEN + payload_len])

    def _resynchronize(self) -> None:
        if self._buffer.startswith(MAGIC):
            return
        index = self._buffer.find(MAGIC)
        if index >= 0:
            del self._buffer[:index]
            return

        keep = 0
        for length in range(1, len(MAGIC)):
            if self._buffer.endswith(MAGIC[:length]):
                keep = length
        if len(self._buffer) > keep:
            del self._buffer[: len(self._buffer) - keep]


def decode_command(payload: bytes) -> StationCommand:
    if len(payload) != COMMAND_PAYLOAD_LEN:
        raise ProtocolError(f"COMMAND payload must be {COMMAND_PAYLOAD_LEN} bytes")
    command = StationCommand(
        capture_id=payload[0:16],
        event_type=struct.unpack_from("<H", payload, 16)[0],
        deployment_id=payload[18:50],
        asset_id=payload[50:82],
        event_id=payload[82:114],
        state_version=struct.unpack_from("<Q", payload, 114)[0],
        previous_event_hash=payload[122:154],
        expected_rfid_hash=payload[154:186],
        observed_at=struct.unpack_from("<q", payload, 186)[0],
        expires_at=struct.unpack_from("<q", payload, 194)[0],
    )
    command.validate()
    return command


def decode_ack(payload: bytes) -> AckPayload:
    if len(payload) != ACK_PAYLOAD_LEN:
        raise ProtocolError(f"ACK payload must be {ACK_PAYLOAD_LEN} bytes")
    return AckPayload(capture_id=payload[:16], event_hash=payload[16:48])


def encode_error(capture_id: bytes, code: int) -> bytes:
    if len(capture_id) != 16:
        raise ProtocolError("ERROR capture_id must be 16 bytes")
    if code not in {1, 2, 3, 4, 5}:
        raise ProtocolError(f"unknown Station error code {code}")
    return capture_id + struct.pack("<H", code) + b"\x00\x00"


def canonical_rfid_from_hex(value: str) -> bytes:
    normalized = value.strip().lower().replace(" ", "")
    if len(normalized) != CANONICAL_RFID_LEN * 2:
        raise ProtocolError("canonical RFID must be exactly 8 bytes / 16 hex characters")
    try:
        decoded = bytes.fromhex(normalized)
    except ValueError as error:
        raise ProtocolError("canonical RFID must contain only hexadecimal characters") from error
    if len(decoded) != CANONICAL_RFID_LEN:
        raise ProtocolError("canonical RFID must be exactly 8 bytes")
    return decoded


def hash_canonical_rfid(canonical_rfid: bytes) -> bytes:
    if len(canonical_rfid) != CANONICAL_RFID_LEN:
        raise ProtocolError("canonical RFID must be exactly 8 bytes")
    return hashlib.sha256(RFID_DOMAIN + canonical_rfid).digest()


def derive_station_id(compressed_public_key: bytes) -> bytes:
    if len(compressed_public_key) != 33 or compressed_public_key[0] not in {2, 3}:
        raise ProtocolError("Station public key must be a compressed 33-byte P-256 SEC1 point")
    return hashlib.sha256(STATION_DOMAIN + compressed_public_key).digest()


def identifier_payload_hash(old_rfid_hash: bytes, new_rfid_hash: bytes) -> bytes:
    return hashlib.sha256(PAYLOAD_DOMAIN + old_rfid_hash + new_rfid_hash).digest()


def encode_capture_envelope(command: StationCommand, station_id: bytes, observed_rfid: bytes) -> bytes:
    """Station rule: bind to an untagged asset, replace the active tag, or prove it is present."""
    if len(station_id) != 32:
        raise ProtocolError("Station ID must be 32 bytes")
    command.validate()
    observed = hash_canonical_rfid(observed_rfid)
    expected = command.expected_rfid_hash
    if command.event_type == EVENT_IDENTIFIER_BOUND:
        old, new = ZERO32, observed
    elif command.event_type == EVENT_IDENTIFIER_REPLACED:
        if observed == expected:
            raise ProtocolError("replacement RFID must differ from the active RFID")
        old, new = expected, observed
    else:
        if observed != expected:
            raise ProtocolError("observed RFID does not match the active RFID")
        old, new = observed, observed
    if not (0 <= command.observed_at <= command.expires_at) or (
        command.expires_at - command.observed_at > MAX_EVENT_AGE_SECONDS
    ):
        raise ProtocolError("invalid event time window")

    out = bytearray(ENVELOPE_LEN)
    struct.pack_into("<HH", out, 0, SCHEMA_VERSION, command.event_type)
    out[4:36] = command.deployment_id
    out[36:68] = command.asset_id
    out[68:100] = command.event_id
    struct.pack_into("<Q", out, 100, command.state_version)
    out[108:140] = command.previous_event_hash
    out[140:172] = identifier_payload_hash(old, new)
    out[172:204] = station_id
    struct.pack_into("<qq", out, 204, command.observed_at, command.expires_at)
    return bytes(out)


def encode_event_ready(
    capture_id: bytes,
    envelope: bytes,
    observed_rfid: bytes,
    station_pubkey33: bytes,
    station_signature64: bytes,
) -> bytes:
    if len(capture_id) != 16:
        raise ProtocolError("EVENT_READY capture_id must be 16 bytes")
    if len(envelope) != ENVELOPE_LEN:
        raise ProtocolError("EVENT_READY envelope must be 220 bytes")
    if len(observed_rfid) != CANONICAL_RFID_LEN:
        raise ProtocolError("EVENT_READY observed RFID must be 8 bytes")
    if len(station_pubkey33) != 33:
        raise ProtocolError("EVENT_READY Station public key must be 33 bytes")
    if len(station_signature64) != 64:
        raise ProtocolError("EVENT_READY Station signature must be 64 bytes")
    return capture_id + envelope + observed_rfid + station_pubkey33 + station_signature64


def event_hash(envelope: bytes) -> bytes:
    if len(envelope) != ENVELOPE_LEN:
        raise ProtocolError("v2 envelope must be exactly 220 bytes")
    return hashlib.sha256(EVENT_DOMAIN + envelope).digest()
