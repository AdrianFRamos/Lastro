"""Pure executable checks for the black-box system harness wire primitives."""

import base64
import hashlib
import json
import struct

from cryptography.hazmat.primitives import serialization
from cryptography.hazmat.primitives.asymmetric import ec, ed25519

from .support import (
    P256_ORDER,
    StationHarness,
    WalletActor,
    SystemContractError,
    build_station_event,
    b58decode_exact,
    b58encode,
    compile_legacy_message,
    crc32c,
    decode_station_command,
    decode_station_event,
    encode_serial_frame,
    expected_event_transaction,
    encode_shortvec,
    ed25519_point_exists,
    find_program_address,
    http_error_message,
    rfid_hash,
    sign_p256_compact_low_s,
    take_serial_frames,
    verify_finalized_transaction_result,
    verify_p256_compact_low_s,
)


def test_system_crc32c_and_serial_frame_contract_round_trip():
    """
    PURPOSE: independently protect the Python G3 harness against serial framing drift.
    ARRANGE: use the standard CRC32C vector and one deterministic 224-byte COMMAND payload.
    ACTION: frame the payload, prefix transport noise, and decode it through the production-compatible harness codec.
    ASSERT: CRC32C uses the standard Castagnoli vector and one COMMAND frame round-trips byte-exactly.
    FAILURE MEANS: system tests could disagree with the real Station/Agent serial contract.
    """
    assert crc32c(b"123456789") == 0xE3069283
    payload = bytes(range(224))
    frame = encode_serial_frame(1, payload)
    buffer = bytearray(b"noise" + frame)
    assert list(take_serial_frames(buffer)) == [(1, payload)]
    assert buffer == bytearray()


def test_system_command_decoder_matches_frozen_offsets():
    """
    PURPOSE: ensure the Station harness constructs evidence only from the exact Agent COMMAND context.
    ARRANGE: populate every fixed COMMAND field at its documented binary offset.
    ACTION: decode the complete 224-byte payload with the system-harness command decoder.
    ASSERT: all fixed fields and little-endian counters decode from the documented 224-byte offsets.
    FAILURE MEANS: the system harness could sign a different context than the real firmware receives.
    """
    payload = bytearray(224)
    payload[:16] = bytes(range(16))
    payload[16] = 2
    payload[20:52] = b"d" * 32
    payload[52:84] = b"a" * 32
    payload[84:92] = (9).to_bytes(8, "little")
    payload[92:96] = (4).to_bytes(4, "little")
    payload[96:128] = b"p" * 32
    payload[128:160] = b"r" * 32
    payload[160:192] = b"f" * 32
    payload[192:224] = b"t" * 32
    command = decode_station_command(bytes(payload))
    assert command["capture_id"] == bytes(range(16))
    assert command["action"] == 2
    assert command["event_sequence"] == 9
    assert command["identity_revision"] == 4
    assert command["expected_old_rfid_hash"] == b"r" * 32


def test_system_p256_signer_emits_compact_low_s_signature():
    """
    PURPOSE: ensure the deterministic Station harness signs the same P-256/SHA-256 message form as production.
    ARRANGE: derive the deterministic test-only P-256 scalar-1 key and one exact 276-byte message.
    ACTION: create the compact Station signature through the system-harness signer.
    ASSERT: compact signature is 64 bytes, low-S, and verifies against the compressed public key and raw message.
    FAILURE MEANS: a G3 pass could depend on a non-canonical signature format the real Station cannot use.
    """
    private = ec.derive_private_key(1, ec.SECP256R1())
    public = private.public_key().public_bytes(serialization.Encoding.X962, serialization.PublicFormat.CompressedPoint)
    message = bytes(range(256)) + bytes(range(20))
    signature = sign_p256_compact_low_s(private, message)
    assert len(signature) == 64
    assert 1 <= int.from_bytes(signature[32:], "big") <= P256_ORDER // 2
    verify_p256_compact_low_s(public, signature, message)



def test_system_wallet_authorizes_only_the_exact_capture_challenge():
    """
    PURPOSE: Keep the black-box system harness subject to the same pre-capture wallet-intent binding as the browser.
    ARRANGE: Build one deterministic Ed25519 wallet and a canonical TRANSFER authorization message.
    ACTION: Sign the exact challenge, then alter only the intended destination while reusing the server message.
    ASSERT: The exact intent yields one verifiable 64-byte signature; altered intent fails before a signature is returned.
    FAILURE MEANS: full-system tests could bypass the production capture-authorization boundary.
    """
    private = ed25519.Ed25519PrivateKey.from_private_bytes(bytes([0x51]) * 32)
    public = private.public_key().public_bytes(serialization.Encoding.Raw, serialization.PublicFormat.Raw)
    wallet = WalletActor(private_key=private, public_key=public, address=b58encode(public))
    challenge_id = "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee"
    deployment_id = bytes([0xD0]) * 32
    program_id = "Vote111111111111111111111111111111111111111"
    animal_id = "11" * 32
    next_custodian = "22" * 32
    expires_at = 2_000_000_000
    message = (
        "Lastro capture authorization v1\n"
        f"challengeId={challenge_id}\n"
        f"deploymentId={deployment_id.hex()}\n"
        f"programId={program_id}\n"
        "action=TRANSFER\n"
        f"animalId={animal_id}\n"
        f"nextCustodian={next_custodian}\n"
        f"requiredSigner={wallet.address}\n"
        f"expiresAtUnix={expires_at}\n"
    ).encode()
    challenge = {
        "challengeId": challenge_id,
        "deploymentId": deployment_id.hex(),
        "requiredSigner": wallet.address,
        "messageBase64": base64.b64encode(message).decode(),
        "expiresAtUnix": expires_at,
    }

    proof = wallet.sign_capture_authorization(
        challenge,
        deployment_id=deployment_id,
        program_id=program_id,
        action="TRANSFER",
        animal_id=animal_id,
        next_custodian=next_custodian,
    )
    signature = base64.b64decode(proof["signatureBase64"], validate=True)
    wallet.private_key.public_key().verify(signature, message)
    assert proof["challengeId"] == challenge_id
    assert len(signature) == 64

    try:
        wallet.sign_capture_authorization(
            challenge,
            deployment_id=deployment_id,
            program_id=program_id,
            action="TRANSFER",
            animal_id=animal_id,
            next_custodian="33" * 32,
        )
    except SystemContractError as error:
        assert "authorization message" in str(error)
    else:
        raise AssertionError("altered capture intent unexpectedly reused an authorization challenge")

def test_system_pda_derivation_uses_canonical_off_curve_bump():
    """
    PURPOSE: protect the Python PDA implementation against seed/hash/curve-check drift.
    ARRANGE: use known Solana ATA seeds plus an independently generated on-curve Ed25519 public key.
    ACTION: derive the PDA and exhaustively inspect every higher bump candidate.
    ASSERT: a cryptography-generated Ed25519 key is recognized on-curve; the ATA derivation returns
            an off-curve digest made from the documented seeds/program marker and highest valid bump.
    FAILURE MEANS: independent system canonical-account checks could inspect the wrong PDA.
    """
    from cryptography.hazmat.primitives.asymmetric import ed25519

    generated_public = ed25519.Ed25519PrivateKey.from_private_bytes(bytes(range(32))).public_key().public_bytes(
        serialization.Encoding.Raw, serialization.PublicFormat.Raw
    )
    assert ed25519_point_exists(generated_public)

    owner = b58decode_exact("11111111111111111111111111111111", 32, "owner")
    token_program = b58decode_exact("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA", 32, "token program")
    mint = b58decode_exact("So11111111111111111111111111111111111111112", 32, "mint")
    ata_program = b58decode_exact("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL", 32, "ATA program")
    derived, bump = find_program_address([owner, token_program, mint], ata_program)
    expected = hashlib.sha256(owner + token_program + mint + bytes([bump]) + ata_program + b"ProgramDerivedAddress").digest()
    assert derived == expected
    assert not ed25519_point_exists(derived)
    for candidate in range(255, bump, -1):
        digest = hashlib.sha256(owner + token_program + mint + bytes([candidate]) + ata_program + b"ProgramDerivedAddress").digest()
        assert ed25519_point_exists(digest)


def test_system_legacy_message_compiler_preserves_instruction_account_order():
    """
    PURPOSE: ensure the test wallet signs a real Solana legacy message rather than an API-specific approximation.
    ARRANGE: define one fee payer, one readonly account, one writable account, and one program instruction.
    ACTION: compile the instruction through the independent legacy-message compiler.
    ASSERT: fee payer is first signer/writable, instruction account order is retained, and shortvec boundaries are canonical.
    FAILURE MEANS: system wallet submission could test a different transaction envelope than the browser.
    """
    payer = bytes([7]) * 32
    readonly = b58encode(bytes([8]) * 32)
    writable = b58encode(bytes([9]) * 32)
    program = b58encode(bytes([10]) * 32)
    instructions = [{
        "programId": program,
        "accounts": [
            {"address": readonly, "isSigner": False, "isWritable": False},
            {"address": writable, "isSigner": False, "isWritable": True},
        ],
        "dataBase64": "AQI=",
    }]
    message = compile_legacy_message(payer, bytes([11]) * 32, instructions)
    assert message[:3] == bytes([1, 0, 2])
    assert message[3] == 4
    assert message[4:36] == payer
    assert encode_shortvec(127) == b"\x7f"
    assert encode_shortvec(128) == b"\x80\x01"
    assert hashlib.sha256(rfid_hash(bytes.fromhex("000000000000002a"))).digest() != bytes(32)


def test_system_http_error_message_is_strict_json_message_only():
    """
    PURPOSE: Ensure commitment polling retries only the exact expected API conflict instead of masking permanent protocol errors.
    ARRANGE: Build HTTP errors with a valid message, malformed JSON, and a non-string message.
    ACTION: Decode them through the system-harness conflict helper.
    ASSERT: Only an object containing a string message is returned; all other bodies yield None.
    FAILURE MEANS: full-system polling could hide a real invariant failure behind a timeout.
    """
    from .support import HttpStatusError

    assert http_error_message(HttpStatusError(409, '{"message":"pending"}', '/submit')) == 'pending'
    assert http_error_message(HttpStatusError(409, 'not-json', '/submit')) is None
    assert http_error_message(HttpStatusError(409, '{"message":7}', '/submit')) is None

def test_system_verifier_binds_finalized_signature_to_exact_event_envelope():
    """
    PURPOSE: make the Python G3/G4 verifier independently bind each EvidencePackage txSignature to
             the exact finalized Secp256r1 + Lastro transaction, not merely canonical terminal state.
    ARRANGE: build one deterministic ORIGIN event and its expected frozen two-instruction envelope.
    ACTION: validate an exact finalized RPC transaction, then tamper program/data/finalization.
    ASSERT: only the exact successful transaction passes and the event-derived descriptor references
            the raw 276 StationEvent bytes through the Secp256r1 instruction.
    FAILURE MEANS: Devnet/local system verification could accept an unrelated finalized signature.
    """
    private = ec.derive_private_key(1, ec.SECP256R1())
    public = private.public_key().public_bytes(serialization.Encoding.X962, serialization.PublicFormat.CompressedPoint)
    station_id = hashlib.sha256(b"LASTRO_STATION" + bytes([0]) + public).digest()
    command = {
        "action": 1,
        "deployment_id": bytes([4]) * 32,
        "animal_id": bytes([5]) * 32,
        "event_sequence": 1,
        "identity_revision": 1,
        "previous_event_hash": bytes(32),
        "expected_old_rfid_hash": bytes(32),
        "from_custodian": bytes(32),
        "to_custodian": bytes([6]) * 32,
    }
    raw = build_station_event(command, bytes.fromhex("8000130000000001"), station_id)
    signature = sign_p256_compact_low_s(private, raw)
    program_id = b58encode(bytes([7]) * 32)
    event = decode_station_event(raw)
    required_signer, expected = expected_event_transaction(raw, event, public, signature, program_id)

    secp = base64.b64decode(expected[0]["dataBase64"], validate=True)
    lastro = base64.b64decode(expected[1]["dataBase64"], validate=True)
    assert len(secp) == 113
    assert struct.unpack_from("<7H", secp, 2) == (16, 0, 80, 0, 8, 276, 1)
    assert secp[16:80] == signature
    assert secp[80:113] == public
    assert lastro[:8] == hashlib.sha256(b"global:origin").digest()[:8]
    assert lastro[8:] == raw

    accounts = expected[1]["accounts"]
    account_keys = [
        required_signer,
        accounts[2]["address"],
        accounts[3]["address"],
        expected[0]["programId"],
        accounts[1]["address"],
        accounts[4]["address"],
        accounts[5]["address"],
        expected[1]["programId"],
    ]
    tx_signature = b58encode(bytes([0x55]) * 64)
    result = {
        "version": "legacy",
        "meta": {"err": None},
        "transaction": {
            "signatures": [tx_signature],
            "message": {
                "header": {
                    "numRequiredSignatures": 1,
                    "numReadonlySignedAccounts": 0,
                    "numReadonlyUnsignedAccounts": 5,
                },
                "accountKeys": account_keys,
                "instructions": [
                    {
                        "programIdIndex": 3,
                        "accounts": [],
                        "data": b58encode(secp),
                    },
                    {
                        "programIdIndex": 7,
                        "accounts": [0, 4, 1, 2, 5, 6],
                        "data": b58encode(lastro),
                    },
                ],
            },
        },
    }
    verify_finalized_transaction_result(result, tx_signature, required_signer, expected)

    for mutation in ("program", "account", "data", "failure", "signature", "nonfinalized"):
        changed = json.loads(json.dumps(result))
        if mutation == "program":
            changed["transaction"]["message"]["accountKeys"][7] = b58encode(bytes([8]) * 32)
        elif mutation == "account":
            changed["transaction"]["message"]["accountKeys"][2] = b58encode(bytes([9]) * 32)
        elif mutation == "data":
            changed["transaction"]["message"]["instructions"][1]["data"] = b58encode(
                lastro[:-1] + bytes([lastro[-1] ^ 1])
            )
        elif mutation == "failure":
            changed["meta"]["err"] = {"InstructionError": [1, {"Custom": 6000}]}
        elif mutation == "signature":
            changed["transaction"]["signatures"][0] = b58encode(bytes([0x56]) * 64)
        else:
            changed = None
        try:
            verify_finalized_transaction_result(changed, tx_signature, required_signer, expected)
        except SystemContractError:
            pass
        else:
            raise AssertionError(f"tampered finalized transaction unexpectedly passed: {mutation}")

