"""G3 full-local contracts through Station harness, Agent, API, wallet, Solana, and verifier."""

import pytest

from .support import (
    FullLocalHarness,
    HttpStatusError,
    fresh_rfid_hex,
    require_system_environment,
)

pytestmark = pytest.mark.system


def test_full_local_origin_transfer_reidentify_transfer():
    """
    PURPOSE: prove the complete core without Devnet/network variability.
    ARRANGE: declared full local stack, deterministic Station harness, and funded Wallet A/B/C keypairs.
    ACTION: create animal -> ORIGIN(A/RFID-A) -> A→B -> stale A replay -> REIDENTIFY to RFID-B -> B→C.
    ASSERT: one AnimalID persists; sequence is 1..4; revision is 1,1,2,2; stale A fails;
            projection equals canonical RPC state after each success; exported evidence independently verifies.
    FAILURE MEANS: G3 core architecture does not work end-to-end in the controlled local environment.
    """
    env = require_system_environment()
    with FullLocalHarness(env) as harness:
        result = harness.run_core_flow(stale_attack=True)

        assert [item["animalId"] for item in result.projections] == [result.animal_id] * 4
        assert [item["eventSequence"] for item in result.projections] == [1, 2, 3, 4]
        assert [item["identityRevision"] for item in result.projections] == [1, 1, 2, 2]
        assert result.projections[0]["currentCustodian"] == harness.wallet_a.custodian_hex
        assert result.projections[1]["currentCustodian"] == harness.wallet_b.custodian_hex
        assert result.projections[2]["currentCustodian"] == harness.wallet_b.custodian_hex
        assert result.projections[3]["currentCustodian"] == harness.wallet_c.custodian_hex
        assert result.projections[0]["currentRfidHash"] == result.rfid_a_hash
        assert result.projections[1]["currentRfidHash"] == result.rfid_a_hash
        assert result.projections[2]["currentRfidHash"] == result.rfid_b_hash
        assert result.projections[3]["currentRfidHash"] == result.rfid_b_hash
        assert result.evidence_package["animalId"] == result.animal_id
        assert len(result.evidence_package["events"]) == 4


def test_full_local_database_never_advances_before_rpc_confirmation():
    """
    PURPOSE: prove PostgreSQL is a projection, not the authority.
    ARRANGE: registered animal plus valid Station evidence accepted by the API, with transaction preparation complete.
    ACTION: withhold Solana submission, attempt confirmation with an invalid signature, and reread projection/RPC state.
    ASSERT: projection remains unoriginated and canonical AnimalState remains absent until a real finalized transaction exists.
    FAILURE MEANS: backend can create canonical-looking state that Solana never accepted.
    """
    env = require_system_environment()
    with FullLocalHarness(env) as harness:
        visual = "preconfirm-" + __import__("uuid").uuid4().hex[:12]
        animal = harness.api.post("/api/animals", {"visualRecoveryId": visual})
        animal_id = animal["animalId"]
        harness.station.queue_observation(1, fresh_rfid_hex())
        capture = harness.reserve_capture(
            action="ORIGIN",
            animal_id=animal_id,
            wallet=harness.wallet_a,
            next_custodian=harness.wallet_a.custodian_hex,
        )
        accepted = harness.wait_capture(capture["captureId"])
        harness.station.wait_ack(capture["captureId"])
        descriptor = harness.api.get(f"/api/events/{accepted['eventHash']}/transaction-data")

        before = harness.api.get(f"/api/animals/{animal_id}")
        assert before["eventSequence"] == 0
        assert before["identityRevision"] == 0
        assert before["currentRfidHash"] is None
        assert before["currentCustodian"] is None
        animal_address = descriptor["instructions"][1]["accounts"][2]["address"]
        assert harness.rpc.account(animal_address) is None

        with pytest.raises(HttpStatusError):
            harness.api.post(f"/api/events/{accepted['eventHash']}/confirm", {"txSignature": "not-a-solana-signature"})

        after = harness.api.get(f"/api/animals/{animal_id}")
        assert after == before
        assert harness.rpc.account(animal_address) is None
