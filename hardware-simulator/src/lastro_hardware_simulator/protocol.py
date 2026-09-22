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

COMMAND_PAYLOAD_LEN: Final = 224
EVENT_READY_PAYLOAD_LEN: Final = 397
ACK_PAYLOAD_LEN: Final = 48
ERROR_PAYLOAD_LEN: Final = 20
STATION_EVENT_LEN: Final = 276
CANONICAL_RFID_LEN: Final = 8

RFID_DOMAIN: Final = b"LASTRO_RFID\0"
STATION_DOMAIN: Final = b"LASTRO_STATION\0"
ZERO32: Final = b"\x00" * 32

EXPECTED_PAYLOAD_LENGTHS: Final = {
    MESSAGE_COMMAND: COMMAND_PAYLOAD_LEN,
    MESSAGE_EVENT_READY: EVENT_READY_PAYLOAD_LEN,
    MESSAGE_ACK: ACK_PAYLOAD_LEN,
    MESSAGE_ERROR: ERROR_PAYLOAD_LEN,
}

ACTION_NAMES: Final = {1: "ORIGIN", 2: "TRANSFER", 3: "REIDENTIFY"}


class ProtocolError(ValueError):
    pass


@dataclass(frozen=True)
class Frame:
    message_type: int
    payload: bytes


@dataclass(frozen=True)
class StationCommand:
    capture_id: bytes
    action: int
    deployment_id: bytes
    animal_id: bytes
    event_sequence: int
    identity_revision: int
    previous_event_hash: bytes
    expected_old_rfid_hash: bytes
    from_custodian: bytes
    to_custodian: bytes

    @property
    def action_name(self) -> str:
        return ACTION_NAMES[self.action]

    def validate(self) -> None:
        if len(self.capture_id) != 16 or self.capture_id == b"\x00" * 16:
            raise ProtocolError("capture_id must be a non-zero 16-byte UUID")
        if self.deployment_id == ZERO32 or self.animal_id == ZERO32:
            raise ProtocolError("deployment_id and animal_id must be non-zero")

        if self.action == 1:
            valid = (
                self.event_sequence == 1
                and self.identity_revision == 1
                and self.previous_event_hash == ZERO32
                and self.expected_old_rfid_hash == ZERO32
                and self.from_custodian == ZERO32
                and self.to_custodian != ZERO32
            )
        elif self.action == 2:
            valid = (
                self.event_sequence >= 2
                and self.identity_revision >= 1
                and self.previous_event_hash != ZERO32
                and self.expected_old_rfid_hash != ZERO32
                and self.from_custodian != ZERO32
                and self.to_custodian != ZERO32
                and self.from_custodian != self.to_custodian
            )
        elif self.action == 3:
            valid = (
                self.event_sequence >= 2
                and self.identity_revision >= 2
                and self.previous_event_hash != ZERO32
                and self.expected_old_rfid_hash != ZERO32
                and self.from_custodian != ZERO32
                and self.from_custodian == self.to_custodian
            )
        else:
            valid = False

        if not valid:
            raise ProtocolError(f"invalid command semantics for action {self.action}")


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
    if payload[17:20] != b"\x00\x00\x00":
        raise ProtocolError("COMMAND reserved bytes must be zero")

    command = StationCommand(
        capture_id=payload[0:16],
        action=payload[16],
        deployment_id=payload[20:52],
        animal_id=payload[52:84],
        event_sequence=struct.unpack_from("<Q", payload, 84)[0],
        identity_revision=struct.unpack_from("<I", payload, 92)[0],
        previous_event_hash=payload[96:128],
        expected_old_rfid_hash=payload[128:160],
        from_custodian=payload[160:192],
        to_custodian=payload[192:224],
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


def encode_station_event(command: StationCommand, station_id: bytes, observed_rfid: bytes) -> bytes:
    if len(station_id) != 32:
        raise ProtocolError("Station ID must be 32 bytes")
    observed_hash = hash_canonical_rfid(observed_rfid)

    if command.action == 2 and observed_hash != command.expected_old_rfid_hash:
        raise ProtocolError("TRANSFER observation does not match expected current RFID")
    if command.action == 3 and observed_hash == command.expected_old_rfid_hash:
        raise ProtocolError("REIDENTIFY observation must differ from expected old RFID")

    out = bytearray(STATION_EVENT_LEN)
    out[0:4] = MAGIC
    out[4] = VERSION
    out[5] = command.action
    out[6:8] = b"\x00\x00"
    out[8:40] = command.deployment_id
    out[40:72] = command.animal_id
    out[72:104] = station_id
    out[104:112] = struct.pack("<Q", command.event_sequence)
    out[112:116] = struct.pack("<I", command.identity_revision)
    out[116:148] = command.previous_event_hash
    out[148:180] = command.expected_old_rfid_hash
    out[180:212] = observed_hash
    out[212:244] = command.from_custodian
    out[244:276] = command.to_custodian
    return bytes(out)


def encode_event_ready(
    capture_id: bytes,
    event_bytes: bytes,
    observed_rfid: bytes,
    station_pubkey33: bytes,
    station_signature64: bytes,
) -> bytes:
    if len(capture_id) != 16:
        raise ProtocolError("EVENT_READY capture_id must be 16 bytes")
    if len(event_bytes) != STATION_EVENT_LEN:
        raise ProtocolError("EVENT_READY event must be 276 bytes")
    if len(observed_rfid) != CANONICAL_RFID_LEN:
        raise ProtocolError("EVENT_READY observed RFID must be 8 bytes")
    if len(station_pubkey33) != 33:
        raise ProtocolError("EVENT_READY Station public key must be 33 bytes")
    if len(station_signature64) != 64:
        raise ProtocolError("EVENT_READY Station signature must be 64 bytes")
    return capture_id + event_bytes + observed_rfid + station_pubkey33 + station_signature64


def event_hash(event_bytes: bytes) -> bytes:
    if len(event_bytes) != STATION_EVENT_LEN:
        raise ProtocolError("StationEvent must be exactly 276 bytes")
    return hashlib.sha256(event_bytes).digest()
