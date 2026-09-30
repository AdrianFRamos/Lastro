"""G3: the complete v2 flow against a local validator and the real Agent/API stack."""
from __future__ import annotations

import base64
import copy

import pytest

from tests.system.support import (
    AgentProcess,
    HttpStatusError,
    LastroFlow,
    StationHarness,
    fresh_rfid_hex,
    require_system_environment,
    rfid_hash,
    run_full_flow,
    verify_package_locally,
    verify_package_on_chain,
)

pytestmark = pytest.mark.system


@pytest.fixture(scope="module")
def stack():
    env = require_system_environment()
    with StationHarness(env.station_scalar_hex) as station, AgentProcess(env, station.path, station.public_key_hex):
        flow = LastroFlow(env, station)
        yield env, flow, run_full_flow(env, flow)


def test_g3_identity_and_custody_reach_finalized_canonical_state(stack):
    """
    PURPOSE: prove the v2 product flow end to end on a real cluster.
    ARRANGE: deployed v2 program, ProtocolConfigV2 + registered Station, API, real Agent, PTY Station,
             authority wallet A, custodian wallets B and C.
    ACTION: register (A) → bind tag A (B) → presence proof (B) → replace with tag B (B) → custody B→C.
    ASSERT: canonical AssetState ends with tag B active, custodian C and three finalized events;
            tag A still resolves to the same asset as RETIRED.
    FAILURE MEANS: the physical-evidence → wallet → chain pipeline does not work as a product.
    """
    env, flow, result = stack
    asset = flow.api.asset(result["asset_id"])
    assert asset["currentRfidHash"] == rfid_hash(bytes.fromhex(result["tag_b"])).hex()
    assert asset["custodian"] == env.wallet_c.public_key.hex()
    assert asset["lastEventHash"] == result["captures"][-1].event_hash

    retired = flow.api.get(f"/api/v2/assets/by-rfid/{rfid_hash(bytes.fromhex(result['tag_a'])).hex()}")
    assert retired["bindingStatus"] == "RETIRED"
    assert retired["asset"]["assetId"] == result["asset_id"]


def test_g3_exported_package_verifies_independently_and_detects_tampering(stack):
    """
    PURPOSE: a third party can check the history without trusting the API.
    ARRANGE: the finalized flow above.
    ACTION: export the EvidencePackage; verify it locally and against finalized RPC; flip one
            envelope byte and verify again.
    ASSERT: the genuine package has no problems; the tampered one is rejected.
    FAILURE MEANS: the evidence package is not independently verifiable proof.
    """
    env, flow, result = stack
    package = flow.api.get(f"/api/v2/assets/{result['asset_id']}/evidence-package")
    assert len(package["events"]) == 3
    assert len(package["custodyTransfers"]) == 1
    assert verify_package_locally(package) == []
    assert verify_package_on_chain(package, flow.rpc, env.program_id, env.authority) == []

    tampered = copy.deepcopy(package)
    envelope = bytearray(base64.b64decode(tampered["events"][1]["envelopeBytesBase64"]))
    envelope[150] ^= 1
    tampered["events"][1]["envelopeBytesBase64"] = base64.b64encode(bytes(envelope)).decode()
    assert verify_package_locally(tampered) != []


def test_g3_previous_custodian_can_no_longer_change_identity(stack):
    """
    PURPOSE: custody transfer revokes the old custodian's authority.
    ARRANGE: asset now held by C.
    ACTION: B asks to replace the RFID and signs the challenge.
    ASSERT: the challenge names C as signer and B's authorization is refused (401) before any
            Station work is reserved.
    FAILURE MEANS: a previous custodian can still rewrite the animal's identity.
    """
    env, flow, result = stack
    authorized = flow.authorize_capture("REPLACE_IDENTIFIER", result["asset_id"], env.wallet_b)
    assert authorized["challenge"]["requiredSigner"] == env.wallet_c.address
    assert authorized["status"] == 401


def test_g3_forged_station_evidence_is_rejected(stack):
    """
    PURPOSE: the API admits only envelopes signed by the registered Station for its capture.
    ARRANGE: a new capture by the current custodian C; the Station signs the genuine read.
    ACTION: before the Agent's copy is used, resend that evidence with one flipped envelope byte.
    ASSERT: the forged evidence is rejected with 4xx and the genuine capture still finalizes.
    FAILURE MEANS: altered physical evidence could enter the chain pipeline.
    """
    env, flow, result = stack
    capture = flow.capture("REPLACE_IDENTIFIER", result["asset_id"], env.wallet_c, fresh_rfid_hex())
    ready = capture.event_ready
    envelope = bytearray(ready[16:236])
    envelope[120] ^= 1
    forged = {
        "captureId": capture.capture_id,
        "envelopeBase64": base64.b64encode(bytes(envelope)).decode(),
        "observedRfidHex": ready[236:244].hex(),
        "stationPubkeyHex": ready[244:277].hex(),
        "stationSignatureHex": ready[277:341].hex(),
    }
    with pytest.raises(HttpStatusError) as error:
        flow.api.agent("/api/agent/evidence", forged)
    assert 400 <= error.value.status < 500
    assert flow.api.get(f"/api/v2/events/{capture.event_hash}")["status"] == "FINALIZED"
