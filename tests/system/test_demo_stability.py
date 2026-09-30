"""G5: the demo flow runs three consecutive times without manual state edits."""
from __future__ import annotations

import json
import os
from pathlib import Path

import pytest

from tests.system.support import (
    AgentProcess,
    LastroFlow,
    StationHarness,
    require_system_environment,
    run_full_flow,
    verify_package_locally,
    verify_package_on_chain,
)

pytestmark = pytest.mark.system


def test_g5_three_consecutive_complete_runs():
    """
    PURPOSE: the demo is repeatable, not a one-off success.
    ARRANGE: the same stack as G3; one Agent and Station for all runs.
    ACTION: run the complete flow three times with fresh assets and tags.
    ASSERT: every run finalizes and its package verifies locally and on-chain; public references
            are written to LASTRO_DEMO_ARTIFACT_DIR when set.
    FAILURE MEANS: the demo depends on luck, leftover state or manual repair.
    """
    env = require_system_environment("LASTRO_DEMO_STABILITY_TEST")
    runs = []
    with StationHarness(env.station_scalar_hex) as station, AgentProcess(env, station.path, station.public_key_hex):
        flow = LastroFlow(env, station)
        for _ in range(3):
            result = run_full_flow(env, flow)
            package = flow.api.get(f"/api/v2/assets/{result['asset_id']}/evidence-package")
            assert verify_package_locally(package) == []
            assert verify_package_on_chain(package, flow.rpc, env.program_id, env.authority) == []
            runs.append(
                {
                    "assetId": result["asset_id"],
                    "eventTransactions": [capture.tx_signature for capture in result["captures"]],
                    "custodyTransaction": result["custody_signature"],
                }
            )
    artifacts = os.getenv("LASTRO_DEMO_ARTIFACT_DIR")
    if artifacts:
        Path(artifacts).mkdir(parents=True, exist_ok=True)
        (Path(artifacts) / "stability.json").write_text(json.dumps(runs, indent=2) + "\n")
    assert len(runs) == 3
