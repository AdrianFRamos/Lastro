#!/usr/bin/env python3
"""Initialize or validate a Lastro v2 deployment: ProtocolConfigV2 plus its registered Station.

This helper never creates a program identity and never stores private keys. It either validates
the existing accounts or, when an authority keypair is supplied, submits:

1. `initialize_v2` (only the program upgrade authority may call it), then
2. `register_station_v2` for the Station public key the API and Agent are configured with.

Both steps are idempotent: existing accounts are validated instead of recreated.
`--extend-station-days N` renews an existing Station registration (`extend_station_validity`)
so it stays valid for N more days; a hardware Station key cannot be rotated, so renew before
`stationValidUntil`.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import struct
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from cryptography.hazmat.primitives.asymmetric import ec  # noqa: E402

from solana_tools import (  # noqa: E402
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
BPF_LOADER_UPGRADEABLE_ID = "BPFLoaderUpgradeab1e11111111111111111111111"
COMPUTE_BUDGET_PROGRAM_ID = "ComputeBudget111111111111111111111111111111"
INITIALIZE_COMPUTE_UNIT_LIMIT = 1400000
SCHEMA_VERSION = 1
CONFIG_LENGTH = 221
STATION_LENGTH = 155
REGISTRIES = (b"station-registry", b"facility-registry", b"party-registry", b"document-registry")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rpc-url", required=True)
    parser.add_argument("--program-id", required=True)
    parser.add_argument("--deployment-id-hex", required=True)
    parser.add_argument("--station-pubkey-hex", required=True)
    parser.add_argument("--authority-keypair")
    parser.add_argument("--validate-only", action="store_true")
    parser.add_argument("--max-asset-weight-grams", type=int, default=2_000_000)
    parser.add_argument("--mass-tolerance-basis-points", type=int, default=500)
    parser.add_argument("--max-event-age-seconds", type=int, default=86_400)
    parser.add_argument("--station-valid-days", type=int, default=365)
    parser.add_argument("--firmware-hash-hex", default="00" * 32)
    parser.add_argument("--extend-station-days", type=int)
    return parser.parse_args()


def station_id_of(pubkey: bytes) -> bytes:
    try:
        ec.EllipticCurvePublicKey.from_encoded_point(ec.SECP256R1(), pubkey)
    except ValueError as error:
        raise SystemContractError("station public key is not a valid compressed P-256 key") from error
    return hashlib.sha256(b"LASTRO_STATION\0" + pubkey).digest()


def global_discriminator(name: str) -> bytes:
    return hashlib.sha256(f"global:{name}".encode()).digest()[:8]


def meta(address: str, signer: bool = False, writable: bool = False) -> dict:
    return {"address": address, "isSigner": signer, "isWritable": writable}


def send(rpc: SolanaRpcClient, authority: WalletActor, instructions: list[dict], label: str) -> str:
    # A program deployed moments ago becomes executable one slot later; retry only that case.
    deadline = time.monotonic() + 60
    while True:
        message = compile_legacy_message(authority.public_key, rpc.latest_blockhash(), instructions)
        signature = authority.private_key.sign(message)
        try:
            returned = rpc.send_transaction(encode_shortvec(1) + signature + message)
            break
        except SystemContractError as error:
            if "Program is not deployed" not in str(error) or time.monotonic() >= deadline:
                raise
            time.sleep(1)
    if returned != b58encode(signature):
        raise SystemContractError(f"RPC returned a different signature for {label}")
    status = rpc.wait_signature(returned, timeout=60)
    if status.get("err") is not None:
        raise SystemContractError(f"{label} transaction failed: {status['err']}")
    return returned


def wait_for_finalized_funding(rpc: SolanaRpcClient, address: str, timeout: float = 30.0) -> None:
    """Do not simulate against an unfunded finalized bank."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        balance = rpc.call("getBalance", [address, {"commitment": "finalized"}])
        if isinstance(balance, dict) and isinstance(balance.get("value"), int) and balance["value"] > 0:
            return
        time.sleep(0.25)
    raise SystemContractError("authority funding did not finalize before the deadline")


def wait_for_finalized_program(rpc: SolanaRpcClient, program_id: str, timeout: float = 90.0) -> None:
    """A freshly deployed program is only `confirmed`; simulation at finalized would not find it."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        result = rpc.call("getAccountInfo", [program_id, {"commitment": "finalized", "encoding": "base64"}])
        value = result.get("value") if isinstance(result, dict) else None
        if isinstance(value, dict) and value.get("executable") is True:
            return
        time.sleep(0.5)
    raise SystemContractError("the Lastro program is not deployed at finalized commitment")


def initialize(rpc, args, program_id: bytes, deployment_id: bytes, authority: WalletActor) -> str:
    program = args.program_id
    config = b58encode(find_program_address([b"config-v2", deployment_id], program_id)[0])
    registries = [b58encode(find_program_address([seed, deployment_id], program_id)[0]) for seed in REGISTRIES]
    program_data, _ = find_program_address(
        [program_id], b58decode_exact(BPF_LOADER_UPGRADEABLE_ID, 32, "upgradeable loader ID")
    )
    data = (
        global_discriminator("initialize_v2")
        + deployment_id
        + struct.pack("<HQHQ", SCHEMA_VERSION, args.max_asset_weight_grams,
                      args.mass_tolerance_basis_points, args.max_event_age_seconds)
    )
    budget = {
        "programId": COMPUTE_BUDGET_PROGRAM_ID,
        "accounts": [],
        "dataBase64": base64.b64encode(bytes([2]) + INITIALIZE_COMPUTE_UNIT_LIMIT.to_bytes(4, "little")).decode(),
    }
    instruction = {
        "programId": program,
        "accounts": [
            meta(authority.address, True, True),
            meta(config, writable=True),
            *[meta(registry, writable=True) for registry in registries],
            meta(program),
            meta(b58encode(program_data)),
            meta(SYSTEM_PROGRAM_ID),
        ],
        "dataBase64": base64.b64encode(data).decode(),
    }
    # ComputeBudget may only follow the Lastro instruction in v2 transactions.
    return send(rpc, authority, [instruction, budget], "initialize_v2")


def register_station(rpc, args, program_id: bytes, deployment_id: bytes, pubkey: bytes, authority: WalletActor) -> str:
    station_id = station_id_of(pubkey)
    now = int(time.time())
    data = (
        global_discriminator("register_station_v2")
        + station_id
        + station_id  # key_id: the key is identified by its StationId until a rotation scheme exists
        + pubkey
        + struct.pack("<qq", now, now + args.station_valid_days * 86_400)
        + decode_hex_exact(args.firmware_hash_hex, 32, "firmware hash")
    )
    instruction = {
        "programId": args.program_id,
        "accounts": [
            meta(authority.address, True, True),
            meta(b58encode(find_program_address([b"config-v2", deployment_id], program_id)[0])),
            meta(b58encode(find_program_address([b"station-registry", deployment_id], program_id)[0])),
            meta(b58encode(find_program_address([b"station-v2", deployment_id, station_id], program_id)[0]), writable=True),
            meta(SYSTEM_PROGRAM_ID),
        ],
        "dataBase64": base64.b64encode(data).decode(),
    }
    return send(rpc, authority, [instruction], "register_station_v2")


def extend_station(rpc, args, program_id: bytes, deployment_id: bytes, pubkey: bytes, authority: WalletActor) -> str:
    station_id = station_id_of(pubkey)
    valid_until = int(time.time()) + args.extend_station_days * 86_400
    instruction = {
        "programId": args.program_id,
        "accounts": [
            meta(authority.address, True, False),
            meta(b58encode(find_program_address([b"config-v2", deployment_id], program_id)[0])),
            meta(b58encode(find_program_address([b"station-registry", deployment_id], program_id)[0])),
            meta(b58encode(find_program_address([b"station-v2", deployment_id, station_id], program_id)[0]), writable=True),
        ],
        "dataBase64": base64.b64encode(global_discriminator("extend_station_validity") + struct.pack("<q", valid_until)).decode(),
    }
    return send(rpc, authority, [instruction], "extend_station_validity")


def validate(rpc, program: str, program_id: bytes, deployment_id: bytes, pubkey: bytes, authority) -> dict:
    config_address, config_bump = find_program_address([b"config-v2", deployment_id], program_id)
    config = decode_account_data(rpc.account(b58encode(config_address)), program, CONFIG_LENGTH)
    if config[:8] != anchor_discriminator("ProtocolConfigV2"):
        raise SystemContractError("ProtocolConfigV2 discriminator is invalid")
    if config[40:72] != deployment_id or config[220] != config_bump:
        raise SystemContractError("ProtocolConfigV2 does not match the requested deployment")
    if authority is not None and config[8:40] != authority.public_key:
        raise SystemContractError("ProtocolConfigV2 authority does not match the supplied authority keypair")
    station_id = station_id_of(pubkey)
    station_address, station_bump = find_program_address([b"station-v2", deployment_id, station_id], program_id)
    station = decode_account_data(rpc.account(b58encode(station_address)), program, STATION_LENGTH)
    if station[:8] != anchor_discriminator("StationRecord") or station[72:105] != pubkey or station[154] != station_bump:
        raise SystemContractError("StationRecord does not match the configured Station public key")
    return {
        "config": b58encode(config_address),
        "authority": b58encode(config[8:40]),
        "deploymentId": deployment_id.hex(),
        "station": b58encode(station_address),
        "stationIdHex": station_id.hex(),
        "stationStatus": station[105],
        "stationValidUntil": struct.unpack("<q", station[114:122])[0],
    }


def main() -> int:
    args = parse_args()
    program_id = b58decode_exact(args.program_id, 32, "program ID")
    deployment_id = decode_hex_exact(args.deployment_id_hex, 32, "deployment ID")
    pubkey = decode_hex_exact(args.station_pubkey_hex, 33, "Station public key")
    station_id = station_id_of(pubkey)
    authority = WalletActor.from_solana_keypair(Path(args.authority_keypair)) if args.authority_keypair else None
    rpc = SolanaRpcClient(args.rpc_url)

    config_address = b58encode(find_program_address([b"config-v2", deployment_id], program_id)[0])
    station_address = b58encode(find_program_address([b"station-v2", deployment_id, station_id], program_id)[0])
    transactions: dict[str, str] = {}
    for label, address, create in (
        ("initialize_v2", config_address, initialize),
        ("register_station_v2", station_address, None),
    ):
        if rpc.account(address) is not None:
            continue
        if args.validate_only:
            raise SystemContractError(f"{label} has not been executed for the requested deployment")
        if authority is None:
            raise SystemContractError(f"--authority-keypair is required to run {label}")
        wait_for_finalized_funding(rpc, authority.address)
        wait_for_finalized_program(rpc, args.program_id)
        transactions[label] = (
            create(rpc, args, program_id, deployment_id, authority)
            if create
            else register_station(rpc, args, program_id, deployment_id, pubkey, authority)
        )

    if args.extend_station_days:
        if authority is None or args.validate_only:
            raise SystemContractError("--extend-station-days requires --authority-keypair")
        transactions["extend_station_validity"] = extend_station(
            rpc, args, program_id, deployment_id, pubkey, authority
        )

    result = validate(rpc, args.program_id, program_id, deployment_id, pubkey, authority)
    result["status"] = "initialized" if transactions else "validated"
    result["transactions"] = transactions
    print(json.dumps(result, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
