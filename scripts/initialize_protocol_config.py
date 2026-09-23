#!/usr/bin/env python3
"""Initialize or validate Lastro ProtocolConfig for local/Devnet validation.

This helper never creates a program identity and never stores private keys. It either validates
the immutable ProtocolConfig account or, when an authority keypair is supplied, submits an
explicit compute-budget request followed by the one Anchor `initialize` instruction required to
create it.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
from pathlib import Path
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from cryptography.hazmat.primitives.asymmetric import ec

from tests.system.support import (
    SolanaRpcClient,
    SystemContractError,
    WalletActor,
    anchor_discriminator,
    b58decode_exact,
    b58encode,
    compile_legacy_message,
    decode_account_data,
    decode_hex_exact,
    encode_shortvec,
    find_program_address,
)

SYSTEM_PROGRAM_ID = "11111111111111111111111111111111"
COMPUTE_BUDGET_PROGRAM_ID = "ComputeBudget111111111111111111111111111111"
INITIALIZE_COMPUTE_UNIT_LIMIT = 1400000
PROTOCOL_CONFIG_LENGTH = 106


def wait_for_finalized_funding(rpc: SolanaRpcClient, address: str, timeout: float = 30.0) -> None:
    """Do not simulate an initialize transaction against an unfunded finalized bank."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        balance = rpc.call("getBalance", [address, {"commitment": "finalized"}])
        if isinstance(balance, dict) and isinstance(balance.get("value"), int) and balance["value"] > 0:
            return
        time.sleep(0.25)
    raise SystemContractError("initialize authority funding did not finalize before the deadline")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rpc-url", required=True)
    parser.add_argument("--program-id", required=True)
    parser.add_argument("--deployment-id-hex", required=True)
    parser.add_argument("--station-pubkey-hex", required=True)
    parser.add_argument("--authority-keypair")
    parser.add_argument("--validate-only", action="store_true")
    return parser.parse_args()


def validate_station_key(station_pubkey: bytes) -> None:
    try:
        ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), station_pubkey)
    except ValueError as error:
        raise SystemContractError("station public key is not a valid compressed P-256 key") from error


def validate_config(
    rpc: SolanaRpcClient,
    address: str,
    *,
    program_id: str,
    deployment_id: bytes,
    station_pubkey: bytes,
    bump: int,
    authority: WalletActor | None,
) -> dict[str, str | int]:
    data = decode_account_data(rpc.account(address), program_id, PROTOCOL_CONFIG_LENGTH)
    if data[:8] != anchor_discriminator("ProtocolConfig"):
        raise SystemContractError("ProtocolConfig discriminator is invalid")
    stored_authority = data[8:40]
    if data[40:72] != deployment_id:
        raise SystemContractError("ProtocolConfig deployment ID does not match the requested deployment")
    if data[72:105] != station_pubkey:
        raise SystemContractError("ProtocolConfig Station public key does not match the requested Station")
    if data[105] != bump:
        raise SystemContractError("ProtocolConfig PDA bump is invalid")
    if authority is not None and stored_authority != authority.public_key:
        raise SystemContractError("ProtocolConfig authority does not match the supplied authority keypair")
    return {
        "address": address,
        "authority": b58encode(stored_authority),
        "deploymentId": deployment_id.hex(),
        "stationPubkeyHex": station_pubkey.hex(),
        "bump": bump,
    }


def initialize(
    rpc: SolanaRpcClient,
    *,
    program_id: str,
    config_address: str,
    deployment_id: bytes,
    station_pubkey: bytes,
    authority: WalletActor,
) -> str:
    wait_for_finalized_funding(rpc, authority.address)
    compute_budget = {
        "programId": COMPUTE_BUDGET_PROGRAM_ID,
        "accounts": [],
        "dataBase64": base64.b64encode(
            bytes([2]) + INITIALIZE_COMPUTE_UNIT_LIMIT.to_bytes(4, "little")
        ).decode(),
    }
    discriminator = hashlib.sha256(b"global:initialize").digest()[:8]
    instruction = {
        "programId": program_id,
        "accounts": [
            {"address": authority.address, "isSigner": True, "isWritable": True},
            {"address": config_address, "isSigner": False, "isWritable": True},
            {"address": SYSTEM_PROGRAM_ID, "isSigner": False, "isWritable": False},
        ],
        "dataBase64": base64.b64encode(discriminator + deployment_id + station_pubkey).decode(),
    }
    message = compile_legacy_message(authority.public_key, rpc.latest_blockhash(), [compute_budget, instruction])
    signature = authority.private_key.sign(message)
    wire = encode_shortvec(1) + signature + message
    transaction_signature = b58encode(signature)
    returned = rpc.send_transaction(wire)
    if returned != transaction_signature:
        raise SystemContractError("RPC returned a transaction signature different from the signed initialize transaction")
    status = rpc.wait_signature(returned, timeout=60)
    if status.get("err") is not None:
        raise SystemContractError(f"ProtocolConfig initialize transaction failed: {status['err']}")
    return returned


def main() -> int:
    args = parse_args()
    program_id = b58decode_exact(args.program_id, 32, "program ID")
    deployment_id = decode_hex_exact(args.deployment_id_hex, 32, "deployment ID")
    station_pubkey = decode_hex_exact(args.station_pubkey_hex, 33, "Station public key")
    validate_station_key(station_pubkey)

    authority = WalletActor.from_solana_keypair(Path(args.authority_keypair)) if args.authority_keypair else None
    rpc = SolanaRpcClient(args.rpc_url)
    address_bytes, bump = find_program_address([b"config", deployment_id], program_id)
    address = b58encode(address_bytes)
    existing = rpc.account(address)

    if existing is None:
        if args.validate_only:
            raise SystemContractError("ProtocolConfig does not exist for the requested deployment")
        if authority is None:
            raise SystemContractError("--authority-keypair is required when ProtocolConfig must be initialized")
        signature = initialize(
            rpc,
            program_id=args.program_id,
            config_address=address,
            deployment_id=deployment_id,
            station_pubkey=station_pubkey,
            authority=authority,
        )
        result = validate_config(
            rpc,
            address,
            program_id=args.program_id,
            deployment_id=deployment_id,
            station_pubkey=station_pubkey,
            bump=bump,
            authority=authority,
        )
        result["status"] = "initialized"
        result["transactionSignature"] = signature
    else:
        result = validate_config(
            rpc,
            address,
            program_id=args.program_id,
            deployment_id=deployment_id,
            station_pubkey=station_pubkey,
            bump=bump,
            authority=authority,
        )
        result["status"] = "validated"

    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
