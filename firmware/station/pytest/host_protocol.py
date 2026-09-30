"""Independent host-side v2 Station protocol for the physical hardware gates.

Deliberately separate from the firmware and the hardware simulator so the gates verify the
Station against a third implementation of docs/PROTOCOL.md.
"""
from __future__ import annotations

import hashlib
import struct
from dataclasses import dataclass

MAGIC = b"LSTR"
VERSION = 1
MESSAGE_COMMAND = 1
MESSAGE_EVENT_READY = 2
MESSAGE_ACK = 3
MESSAGE_ERROR = 4
PAYLOAD_LENGTHS = {MESSAGE_COMMAND: 202, MESSAGE_EVENT_READY: 341, MESSAGE_ACK: 48, MESSAGE_ERROR: 20}
EVENT_IDENTIFIER_BOUND = 18
ZERO32 = bytes(32)


def crc32c(data: bytes) -> int:
    crc = 0xFFFFFFFF
    for byte in data:
        crc ^= byte
        for _ in range(8):
            crc = (crc >> 1) ^ (0x82F63B78 if crc & 1 else 0)
    return crc ^ 0xFFFFFFFF


def encode_frame(message_type: int, payload: bytes) -> bytes:
    header = struct.pack("<BBHI", VERSION, message_type, 0, len(payload))
    return MAGIC + header + payload + struct.pack("<I", crc32c(header + payload))


def take_frames(buffer: bytearray) -> list[tuple[int, bytes]]:
    """Consume complete, CRC-valid frames from `buffer`; raise on a corrupt frame."""
    frames = []
    while True:
        start = buffer.find(MAGIC)
        if start < 0:
            del buffer[: max(0, len(buffer) - 3)]
            return frames
        del buffer[:start]
        if len(buffer) < 12:
            return frames
        version, message_type, reserved, length = struct.unpack_from("<BBHI", buffer, 4)
        if version != VERSION or reserved != 0 or PAYLOAD_LENGTHS.get(message_type) != length:
            raise ValueError(f"invalid frame header: type={message_type} len={length}")
        if len(buffer) < 16 + length:
            return frames
        body = bytes(buffer[4 : 12 + length])
        if struct.unpack_from("<I", buffer, 12 + length)[0] != crc32c(body):
            raise ValueError("serial CRC32C mismatch")
        frames.append((message_type, bytes(buffer[12 : 12 + length])))
        del buffer[: 16 + length]


def domain_hash(domain: bytes, data: bytes) -> bytes:
    return hashlib.sha256(domain + data).digest()


def rfid_hash(canonical_rfid: bytes) -> bytes:
    return domain_hash(b"LASTRO_RFID\0", canonical_rfid)


def station_id(pubkey33: bytes) -> bytes:
    return domain_hash(b"LASTRO_STATION\0", pubkey33)


def capture_event_id(capture_id: bytes) -> bytes:
    return domain_hash(b"LASTRO_V2_CAPTURE_EVENT\0", capture_id)


def event_hash(envelope: bytes) -> bytes:
    return domain_hash(b"LASTRO_V2_EVENT\0", envelope)


@dataclass(frozen=True)
class BindCommand:
    """IDENTIFIER_BOUND for a fresh asset: the one capture that needs no prior chain state."""

    capture_id: bytes
    deployment_id: bytes
    asset_id: bytes
    observed_at: int
    expires_at: int

    def encode(self) -> bytes:
        return (
            self.capture_id
            + struct.pack("<H", EVENT_IDENTIFIER_BOUND)
            + self.deployment_id
            + self.asset_id
            + capture_event_id(self.capture_id)
            + struct.pack("<Q", 1)
            + ZERO32
            + ZERO32
            + struct.pack("<qq", self.observed_at, self.expires_at)
        )

    def expected_envelope(self, pubkey33: bytes, observed_rfid: bytes) -> bytes:
        payload_hash = domain_hash(b"LASTRO_V2_PAYLOAD\0", ZERO32 + rfid_hash(observed_rfid))
        return (
            struct.pack("<HH", 1, EVENT_IDENTIFIER_BOUND)
            + self.deployment_id
            + self.asset_id
            + capture_event_id(self.capture_id)
            + struct.pack("<Q", 1)
            + ZERO32
            + payload_hash
            + station_id(pubkey33)
            + struct.pack("<qq", self.observed_at, self.expires_at)
        )
