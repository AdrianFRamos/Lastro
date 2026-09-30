"""G4: the same v2 flow against the deployed Devnet program and configuration."""
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


def test_g4_devnet_flow_finalizes_and_verifies():
    """
    PURPOSE: prove the product flow on the deployed Devnet program, not only a local validator.
    ARRANGE: Devnet program + ProtocolConfigV2 + registered Station, API pointed at Devnet, funded
             wallets A (deployment authority), B and C.
    ACTION: run the complete flow once and export the evidence package.
    ASSERT: every step finalizes on Devnet and the package verifies locally and on-chain. Only
            public references (asset, transaction signatures) are written as artifacts.
    FAILURE MEANS: the deployment is not usable as the canonical source of identity and custody.
    """
    env = require_system_environment("LASTRO_DEVNET_TEST")
    with StationHarness(env.station_scalar_hex) as station, AgentProcess(env, station.path, station.public_key_hex):
        flow = LastroFlow(env, station)
        result = run_full_flow(env, flow)
        package = flow.api.get(f"/api/v2/assets/{result['asset_id']}/evidence-package")
    assert verify_package_locally(package) == []
    assert verify_package_on_chain(package, flow.rpc, env.program_id, env.authority) == []

    artifacts = os.getenv("LASTRO_DEVNET_ARTIFACT_DIR")
    if artifacts:
        Path(artifacts).mkdir(parents=True, exist_ok=True)
        summary = {
            "cluster": "devnet",
            "programId": env.program_id,
            "assetId": result["asset_id"],
            "eventTransactions": [capture.tx_signature for capture in result["captures"]],
            "custodyTransaction": result["custody_signature"],
        }
        (Path(artifacts) / "devnet-flow.json").write_text(json.dumps(summary, indent=2) + "\n")
