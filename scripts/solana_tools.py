"""Minimal Solana RPC, wallet and transaction helpers for Lastro deployment scripts.

Standard library + `cryptography` only, so operators can run the scripts without the Solana
CLI SDKs. Private keys are read from Solana keypair files and never printed or stored.
"""
from __future__ import annotations

import base64
import hashlib
import json
import time
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import ed25519


class SystemContractError(RuntimeError):
    pass


class RpcError(SystemContractError):
    pass


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
        raise SystemContractError("Lastro scripts require fee payer to be the only transaction signer")

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
