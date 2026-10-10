#!/usr/bin/env python3
"""Register (or validate) one participant wallet in a Lastro v2 deployment's party registry.

Only the deployment authority may call `register_party`. The PartyRecord binds a wallet to a
role; the web login reads it to choose the participant workspace. The party ID defaults to
SHA-256("LASTRO_PARTY\\0" || wallet), so running this twice for the same wallet validates the
existing record instead of creating a second one. This helper never stores private keys.

Roles (programs/lastro-v2/src/constants.rs): 1 producer, 2 custodian, 3 seller, 4 buyer,
5 transporter, 6 slaughterhouse, 7 processing facility, 8 distributor, 9 retailer,
10 auditor, 11 official source, 12 exporter.

A registered role cannot be changed. To move a wallet to another role, register a new party with
`--party-id-hex` and retire the old one with `--set-status 3` (REVOKED is terminal); both stay
on-chain as an auditable history.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import struct
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from solana_tools import (  # noqa: E402
    SolanaRpcClient,
    SystemContractError,
    WalletActor,
    anchor_discriminator,
    b58decode_exact,
    b58encode,
    decode_account_data,
    decode_hex_exact,
    find_program_address,
)
from initialize_protocol_config import meta, send  # noqa: E402

SYSTEM_PROGRAM_ID = "11111111111111111111111111111111"
PARTY_LENGTH = 76
VALID_ROLES = range(1, 13)
VALID_STATUSES = range(1, 5)  # 1 active, 2 suspended, 3 revoked (terminal), 4 expired


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--rpc-url", required=True)
    parser.add_argument("--program-id", required=True)
    parser.add_argument("--deployment-id-hex", required=True)
    parser.add_argument("--wallet", required=True, help="base58 address of the participant wallet")
    parser.add_argument("--role", type=int, required=True)
    parser.add_argument("--party-id-hex", help="defaults to SHA-256('LASTRO_PARTY\\0' || wallet)")
    parser.add_argument("--authority-keypair", help="deployment authority; omit to only validate")
    parser.add_argument(
        "--set-status", type=int, help="change an existing party's status (1..4) instead of registering"
    )
    return parser.parse_args()


def default_party_id(wallet: bytes) -> bytes:
    return hashlib.sha256(b"LASTRO_PARTY\0" + wallet).digest()


def global_discriminator(name: str) -> bytes:
    return hashlib.sha256(f"global:{name}".encode()).digest()[:8]


def main() -> int:
    args = parse_args()
    if args.role not in VALID_ROLES:
        raise SystemContractError("role must be between 1 and 12")
    program_id = b58decode_exact(args.program_id, 32, "program ID")
    deployment_id = decode_hex_exact(args.deployment_id_hex, 32, "deployment ID")
    wallet = b58decode_exact(args.wallet, 32, "participant wallet")
    party_id = (
        decode_hex_exact(args.party_id_hex, 32, "party ID") if args.party_id_hex else default_party_id(wallet)
    )
    rpc = SolanaRpcClient(args.rpc_url)

    party_address, party_bump = find_program_address([b"party", deployment_id, party_id], program_id)
    party = b58encode(party_address)
    transaction = None
    if args.set_status is not None:
        if args.set_status not in VALID_STATUSES:
            raise SystemContractError("status must be between 1 and 4")
        if not args.authority_keypair or rpc.account(party) is None:
            raise SystemContractError("--set-status needs an existing party and --authority-keypair")
        authority = WalletActor.from_solana_keypair(Path(args.authority_keypair))
        instruction = {
            "programId": args.program_id,
            "accounts": [
                meta(authority.address, True, False),
                meta(b58encode(find_program_address([b"config-v2", deployment_id], program_id)[0])),
                meta(b58encode(find_program_address([b"party-registry", deployment_id], program_id)[0])),
                meta(party, writable=True),
            ],
            "dataBase64": base64.b64encode(
                global_discriminator("set_party_status") + bytes([args.set_status])
            ).decode(),
        }
        transaction = send(rpc, authority, [instruction], "set_party_status")
    elif rpc.account(party) is None:
        if not args.authority_keypair:
            raise SystemContractError("party is not registered; pass --authority-keypair to register it")
        authority = WalletActor.from_solana_keypair(Path(args.authority_keypair))
        data = global_discriminator("register_party") + party_id + wallet + struct.pack("<H", args.role)
        instruction = {
            "programId": args.program_id,
            "accounts": [
                meta(authority.address, True, True),
                meta(b58encode(find_program_address([b"config-v2", deployment_id], program_id)[0])),
                meta(b58encode(find_program_address([b"party-registry", deployment_id], program_id)[0])),
                meta(party, writable=True),
                meta(SYSTEM_PROGRAM_ID),
            ],
            "dataBase64": base64.b64encode(data).decode(),
        }
        transaction = send(rpc, authority, [instruction], "register_party")

    record = decode_account_data(rpc.account(party), args.program_id, PARTY_LENGTH)
    if record[:8] != anchor_discriminator("PartyRecord") or record[75] != party_bump:
        raise SystemContractError("PartyRecord layout does not match the program")
    if record[8:40] != party_id or record[40:72] != wallet:
        raise SystemContractError("PartyRecord is registered for a different party ID or wallet")
    role = struct.unpack("<H", record[72:74])[0]
    if role != args.role:
        raise SystemContractError(f"party already registered with role {role}, not {args.role}")
    if args.set_status is not None and record[74] != args.set_status:
        raise SystemContractError(f"party status is {record[74]} after set_party_status")
    print(
        json.dumps(
            {
                "party": party,
                "partyIdHex": party_id.hex(),
                "wallet": args.wallet,
                "role": role,
                "status": record[74],
                "transaction": transaction,
            },
            sort_keys=True,
            separators=(",", ":"),
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
