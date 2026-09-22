"""G5 stability gate for three consecutive complete demo runs."""

import base64
import copy
import json
import os
from datetime import datetime, timezone
from pathlib import Path

import pytest

from .support import FullLocalHarness, SystemContractError, require_system_environment, verify_evidence_package

pytestmark = pytest.mark.system


def require_demo_environment():
    if os.getenv("LASTRO_DEMO_STABILITY_TEST") != "1":
        pytest.skip("Set LASTRO_DEMO_STABILITY_TEST=1 only in the final demo environment")
    return require_system_environment()


def test_g5_complete_demo_runs_three_times_without_manual_state_repair():
    """
    PURPOSE: prove the final filmed workflow is repeatable rather than a one-off successful state.
    ARRANGE: one declared release-candidate full stack with funded actors and no operator edits to DB/protocol/Solana state.
    ACTION: run the complete flow three times, including stale-custodian rejection and one-byte evidence tamper verification.
    ASSERT: every run reaches C/revision 2/sequence 4; stale A fails; valid evidence verifies; tampered evidence fails; artifacts identify each run.
    FAILURE MEANS: G5 is not stable enough for submission or recording.
    """
    env = require_demo_environment()
    artifact_root_raw = os.getenv("LASTRO_DEMO_ARTIFACT_DIR")
    if not artifact_root_raw:
        raise RuntimeError("LASTRO_DEMO_ARTIFACT_DIR is required when LASTRO_DEMO_STABILITY_TEST=1")
    artifact_root = Path(artifact_root_raw).expanduser().resolve()
    artifact_root.mkdir(parents=True, exist_ok=True)

    records = []
    with FullLocalHarness(env) as harness:
        for run_number in range(1, 4):
            result = harness.run_core_flow(stale_attack=True)
            assert result.final_projection["eventSequence"] == 4
            assert result.final_projection["identityRevision"] == 2
            assert result.final_projection["currentCustodian"] == harness.wallet_c.custodian_hex
            verify_evidence_package(result.evidence_package, harness.rpc, env.program_id)

            tampered = copy.deepcopy(result.evidence_package)
            raw = bytearray(base64.b64decode(tampered["events"][-1]["eventBytesBase64"], validate=True))
            raw[200] ^= 0x01
            tampered["events"][-1]["eventBytesBase64"] = base64.b64encode(raw).decode()
            with pytest.raises(SystemContractError):
                verify_evidence_package(tampered, harness.rpc, env.program_id)

            record = {
                "run": run_number,
                "recordedAt": datetime.now(timezone.utc).isoformat(),
                "animalId": result.animal_id,
                "visualRecoveryId": result.visual_recovery_id,
                "transactionSignatures": result.transaction_signatures,
                "staleAttemptSignature": result.stale_attempt_signature,
                "terminalProjection": result.final_projection,
            }
            (artifact_root / f"demo-run-{run_number}-{result.animal_id}.json").write_text(
                json.dumps(record, indent=2, sort_keys=True) + "\n"
            )
            records.append(record)

    assert len({record["animalId"] for record in records}) == 3
    assert all(len(record["transactionSignatures"]) == 4 for record in records)
