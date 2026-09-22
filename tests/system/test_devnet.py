"""G4 Devnet contracts. These tests create public testnet transactions and are opt-in."""

import json
import os
from datetime import datetime, timezone
from pathlib import Path

import pytest

from .support import FullLocalHarness, require_system_environment

pytestmark = pytest.mark.system


def require_devnet_environment():
    if os.getenv("LASTRO_DEVNET_TEST") != "1":
        pytest.skip("Set LASTRO_DEVNET_TEST=1 only with the declared Devnet program/wallet configuration")
    return require_system_environment()


def artifact_directory() -> Path:
    raw = os.getenv("LASTRO_DEVNET_ARTIFACT_DIR")
    if not raw:
        raise RuntimeError("LASTRO_DEVNET_ARTIFACT_DIR is required when LASTRO_DEVNET_TEST=1")
    path = Path(raw).expanduser().resolve()
    path.mkdir(parents=True, exist_ok=True)
    return path


def test_g4_complete_flow_on_devnet_and_records_evidence_references():
    """
    PURPOSE: prove the G3 protocol against the real Solana Devnet execution path.
    ARRANGE: deployed program/config, funded Wallet A/B/C, running API, real Agent binary, and deterministic Station serial harness.
    ACTION: execute ORIGIN -> A→B -> REIDENTIFY -> B→C through normal capture/evidence/wallet/confirmation paths.
    ASSERT: four successful transaction signatures finalize; terminal state/evidence verify independently; a run artifact records public references.
    FAILURE MEANS: local-only assumptions do not hold on the target public Solana environment.
    """
    env = require_devnet_environment()
    with FullLocalHarness(env) as harness:
        result = harness.run_core_flow(stale_attack=False)
        statuses = [harness.rpc.wait_signature(signature, timeout=60) for signature in result.transaction_signatures]
        assert all(status.get("err") is None for status in statuses)
        assert result.final_projection["eventSequence"] == 4
        assert result.final_projection["identityRevision"] == 2
        assert result.final_projection["currentCustodian"] == harness.wallet_c.custodian_hex
        assert len(result.evidence_package["events"]) == 4

        artifact = {
            "recordedAt": datetime.now(timezone.utc).isoformat(),
            "cluster": "devnet",
            "programId": env.program_id,
            "deploymentId": env.deployment_id.hex(),
            "animalId": result.animal_id,
            "visualRecoveryId": result.visual_recovery_id,
            "transactionSignatures": result.transaction_signatures,
            "terminalProjection": result.final_projection,
        }
        destination = artifact_directory() / f"devnet-{result.animal_id}.json"
        destination.write_text(json.dumps(artifact, indent=2, sort_keys=True) + "\n")
        assert destination.is_file()


def test_g4_stale_custodian_attack_is_rejected_on_devnet_without_state_change():
    """
    PURPOSE: demonstrate stale-custodian rejection on the public target network rather than only unit tests.
    ARRANGE: complete A→B, then resubmit the already-consumed A-authorized transfer envelope with a fresh blockhash.
    ACTION: Wallet A signs and sends the stale event again while canonical custody belongs to B.
    ASSERT: the attack fails and the core flow still reaches the expected B→C terminal state without state corruption.
    FAILURE MEANS: the demo's canonical-authority rejection claim lacks a real-network proof.
    """
    env = require_devnet_environment()
    with FullLocalHarness(env) as harness:
        result = harness.run_core_flow(stale_attack=True)
        assert result.stale_attempt_signature is not None, "Devnet must record the failed stale transaction signature"
        status = harness.rpc.wait_signature(result.stale_attempt_signature, timeout=60)
        assert status.get("err") is not None
        assert result.final_projection["eventSequence"] == 4
        assert result.final_projection["currentCustodian"] == harness.wallet_c.custodian_hex
