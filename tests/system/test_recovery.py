"""System contracts for RFID and independent visual recovery behavior."""

import uuid

import pytest

from .support import (
    FullLocalHarness,
    HttpStatusError,
    b58decode_exact,
    b58encode,
    decode_binding_account,
    find_program_address,
    require_system_environment,
)

pytestmark = pytest.mark.system


def test_reidentify_recovers_by_visual_id_without_creating_new_animal():
    """
    PURPOSE: prove visual recovery preserves the existing logical identity when the previous RFID is unavailable.
    ARRANGE: complete the declared core flow with recovery immediately before REIDENTIFY.
    ACTION: resolve the visual recovery identifier after custody reaches B and complete REIDENTIFY with RFID B.
    ASSERT: recovery and every projected transition use the same AnimalID; revision advances only once at REIDENTIFY.
    FAILURE MEANS: recovery created a parallel identity or broke canonical identity continuity.
    """
    env = require_system_environment()
    with FullLocalHarness(env) as harness:
        result = harness.run_core_flow(stale_attack=False)
        recovered = harness.api.get(f"/api/animals/by-recovery/{result.visual_recovery_id}")
        assert recovered["animalId"] == result.animal_id
        assert [projection["animalId"] for projection in result.projections] == [result.animal_id] * 4
        assert [projection["identityRevision"] for projection in result.projections] == [1, 1, 2, 2]


def test_known_current_rfid_hash_resolves_exactly_one_active_animal():
    """
    PURPOSE: prove a known current RFID hash resolves uniquely and agrees with canonical RfidBinding.
    ARRANGE: complete the core flow so RFID B is current for one AnimalID.
    ACTION: query the API by current hash and independently derive/read the canonical binding PDA.
    ASSERT: API and ACTIVE binding resolve the same AnimalID and RFID hash.
    FAILURE MEANS: operational current-RFID lookup can diverge from canonical state or become ambiguous.
    """
    env = require_system_environment()
    with FullLocalHarness(env) as harness:
        result = harness.run_core_flow(stale_attack=False)
        resolved = harness.api.get(f"/api/animals/by-rfid/{result.rfid_b_hash}")
        assert resolved["animalId"] == result.animal_id

        program = b58decode_exact(env.program_id, 32, "LASTRO_PROGRAM_ID")
        binding_address, _ = find_program_address(
            [b"rfid", env.deployment_id, bytes.fromhex(result.rfid_b_hash)], program
        )
        binding = decode_binding_account(harness.rpc.account(b58encode(binding_address)), env.program_id)
        assert binding["animal_id"] == result.animal_id
        assert binding["rfid_hash"] == result.rfid_b_hash
        assert binding["status"] == 1


def test_retired_rfid_does_not_resolve_as_current_after_reidentify():
    """
    PURPOSE: prove REIDENTIFY retires the previous RFID from current lookup semantics.
    ARRANGE: complete REIDENTIFY from RFID A to RFID B in the full core flow.
    ACTION: query both hashes and independently read the old binding PDA.
    ASSERT: RFID A returns not-found, RFID B resolves the original AnimalID, and RFID A is RETIRED on-chain.
    FAILURE MEANS: a retired physical identifier can still represent current state.
    """
    env = require_system_environment()
    with FullLocalHarness(env) as harness:
        result = harness.run_core_flow(stale_attack=False)
        with pytest.raises(HttpStatusError) as error:
            harness.api.get(f"/api/animals/by-rfid/{result.rfid_a_hash}")
        assert error.value.status == 404
        assert harness.api.get(f"/api/animals/by-rfid/{result.rfid_b_hash}")["animalId"] == result.animal_id

        program = b58decode_exact(env.program_id, 32, "LASTRO_PROGRAM_ID")
        old_address, _ = find_program_address(
            [b"rfid", env.deployment_id, bytes.fromhex(result.rfid_a_hash)], program
        )
        old_binding = decode_binding_account(harness.rpc.account(b58encode(old_address)), env.program_id)
        assert old_binding["animal_id"] == result.animal_id
        assert old_binding["status"] == 2


def test_both_physical_identifiers_missing_remains_unresolved_in_hackathon_scope():
    """
    PURPOSE: prove the hackathon remains fail-closed when both supported physical recovery identifiers are unavailable.
    ARRANGE: existing canonical animal history and no usable RFID or visual recovery identifier.
    ACTION: issue lookup attempts with unrelated recovery/RFID values and then reread the known animal.
    ASSERT: both lookups return not-found and canonical/projected identity remains unchanged.
    FAILURE MEANS: the system infers identity without evidence permitted by the protocol.
    """
    env = require_system_environment()
    with FullLocalHarness(env) as harness:
        result = harness.run_core_flow(stale_attack=False)
        before = harness.api.get(f"/api/animals/{result.animal_id}")
        with pytest.raises(HttpStatusError) as recovery_error:
            harness.api.get(f"/api/animals/by-recovery/unrelated-{uuid.uuid4().hex}")
        with pytest.raises(HttpStatusError) as rfid_error:
            harness.api.get("/api/animals/by-rfid/" + "ff" * 32)
        assert recovery_error.value.status == 404
        assert rfid_error.value.status == 404
        assert harness.api.get(f"/api/animals/{result.animal_id}") == before
