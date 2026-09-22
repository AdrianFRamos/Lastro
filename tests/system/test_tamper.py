"""System tamper contracts against evidence exported from an actual full-stack run."""

import base64
import copy

import pytest

from .support import FullLocalHarness, SystemContractError, require_system_environment, verify_evidence_package

pytestmark = pytest.mark.system


def test_exported_real_package_fails_after_exactly_one_signed_byte_changes():
    """
    PURPOSE: prove independent tamper detection using evidence produced by the running stack.
    ARRANGE: export and independently verify a valid package from a completed system run.
    ACTION: flip exactly one non-framing byte inside one signed StationEvent while leaving signature/key/tx unchanged.
    ASSERT: original remains valid and the tampered package is rejected independently of backend or RPC verdicts.
    FAILURE MEANS: signed evidence can be modified without detection.
    """
    env = require_system_environment()
    with FullLocalHarness(env) as harness:
        result = harness.run_core_flow(stale_attack=False)
        verify_evidence_package(result.evidence_package, harness.rpc, env.program_id)
        tampered = copy.deepcopy(result.evidence_package)
        raw = bytearray(base64.b64decode(tampered["events"][1]["eventBytesBase64"], validate=True))
        raw[60] ^= 0x01
        tampered["events"][1]["eventBytesBase64"] = base64.b64encode(raw).decode()
        with pytest.raises(SystemContractError):
            verify_evidence_package(tampered, harness.rpc, env.program_id)


def test_observed_rfid_tamper_is_detected_even_when_signed_event_bytes_are_unchanged():
    """
    PURPOSE: protect the EvidencePackage statement about which RFID the Station observed.
    ARRANGE: independently verify a valid exported package and preserve every signed event byte/signature.
    ACTION: change only one observedRfidHex byte in one package event.
    ASSERT: independent verification rejects the package because RFID hash no longer matches signed new_rfid_hash.
    FAILURE MEANS: package metadata can misrepresent physical observation without detection.
    """
    env = require_system_environment()
    with FullLocalHarness(env) as harness:
        result = harness.run_core_flow(stale_attack=False)
        tampered = copy.deepcopy(result.evidence_package)
        observed = bytearray.fromhex(tampered["events"][0]["observedRfidHex"])
        observed[-1] ^= 0x01
        tampered["events"][0]["observedRfidHex"] = observed.hex()
        with pytest.raises(SystemContractError, match="RFID evidence"):
            verify_evidence_package(tampered, harness.rpc, env.program_id)
