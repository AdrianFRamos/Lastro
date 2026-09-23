from __future__ import annotations

import json
from pathlib import Path

import jsonschema
import yaml

ROOT = Path(__file__).resolve().parents[2]


def test_evidence_package_schema_is_valid_draft_2020_12_and_accepts_both_structural_fixtures():
    """The tampered package must remain structurally valid so verifier failure is cryptographic, not JSON-shape failure."""
    schema = json.loads((ROOT / 'schemas/evidence-package.schema.json').read_text())
    jsonschema.Draft202012Validator.check_schema(schema)
    validator = jsonschema.Draft202012Validator(schema)
    for name in ('evidence-package.valid.json', 'evidence-package.tampered.json'):
        payload = json.loads((ROOT / 'test-vectors' / name).read_text())
        errors = list(validator.iter_errors(payload))
        assert not errors, f'{name}: {[e.message for e in errors]}'


def test_evidence_schema_freezes_protocol_lengths_in_transport_encoding():
    schema = json.loads((ROOT / 'schemas/evidence-package.schema.json').read_text())
    events = schema['properties']['events']
    event = events['items']['properties']
    assert event['observedRfidHex']['pattern'] == '^[0-9a-f]{16}$'       # 8 bytes
    assert event['stationPubkeyHex']['pattern'] == '^[0-9a-f]{66}$'     # 33 bytes
    assert event['stationSignatureHex']['pattern'] == '^[0-9a-f]{128}$' # 64 bytes
    assert event['eventBytesBase64']['minLength'] == 368
    assert event['eventBytesBase64']['maxLength'] == 368
    assert event['txSignature']['minLength'] == 64
    assert event['txSignature']['maxLength'] == 88
    assert 'maxItems' not in events


def test_openapi_contains_exact_hackathon_surface_and_no_compliance_endpoint():
    spec = yaml.safe_load((ROOT / 'schemas/openapi.yaml').read_text())
    paths = set(spec['paths'])
    expected = {
        '/api/health',
        '/api/animals',
        '/api/animals/{animalId}',
        '/api/animals/by-recovery/{visualRecoveryId}',
        '/api/animals/by-rfid/{rfidHash}',
        '/api/captures/authorization-challenge',
        '/api/captures',
        '/api/captures/{captureId}',
        '/api/agent/commands',
        '/api/agent/evidence',
        '/api/events/{eventHash}/transaction-data',
        '/api/events/{eventHash}/submit',
        '/api/events/{eventHash}/confirm',
        '/api/animals/{animalId}/evidence-package',
    }
    assert paths == expected
    lowered = '\n'.join(paths).lower()
    assert 'eudr' not in lowered and 'compliance' not in lowered and 'credit' not in lowered


def test_openapi_transaction_contract_freezes_two_instructions_and_1232_byte_limit():
    spec = yaml.safe_load((ROOT / 'schemas/openapi.yaml').read_text())
    tx = spec['components']['schemas']['TransactionData']['properties']
    instructions = tx['instructions']
    assert instructions['minItems'] == 2
    assert instructions['maxItems'] == 2
    assert instructions['items'] is False
    assert tx['measuredSerializedBytes']['maximum'] == 1232
    assert tx['transactionVersion']['enum'] == ['legacy', 'v0']


def test_openapi_submission_lifecycle_is_explicit_and_does_not_advance_projection():
    """Confirmed transaction registration must be distinct from finalized canonical projection."""
    spec = yaml.safe_load((ROOT / 'schemas/openapi.yaml').read_text())
    submission = spec['components']['schemas']['EventSubmission']['properties']
    assert submission['status']['enum'] == ['SUBMITTED', 'FINALIZED']
    assert '/api/events/{eventHash}/submit' in spec['paths']
    assert '/api/events/{eventHash}/confirm' in spec['paths']
    submit_response = spec['paths']['/api/events/{eventHash}/submit']['post']['responses']['200']
    confirm_response = spec['paths']['/api/events/{eventHash}/confirm']['post']['responses']['200']
    assert submit_response['content']['application/json']['schema']['$ref'].endswith('/EventSubmission')
    assert confirm_response['content']['application/json']['schema']['$ref'].endswith('/Animal')


def test_openapi_agent_command_cannot_supply_new_rfid_hash():
    """The Station must derive the new RFID hash from the physical reader, never receive it from backend context."""
    spec = yaml.safe_load((ROOT / 'schemas/openapi.yaml').read_text())
    properties = spec['components']['schemas']['AgentCommand']['properties']
    assert 'expectedOldRfidHash' in properties
    assert 'newRfidHash' not in properties
    assert 'observedRfid' not in properties


def test_openapi_freezes_http_and_transaction_identifier_resource_bounds():
    """The public contract must match the application limits enforced before expensive work."""
    spec = yaml.safe_load((ROOT / 'schemas/openapi.yaml').read_text())
    assert '1024 bytes' in spec['info']['description']
    signature = spec['components']['schemas']['SolanaSignature']
    assert signature['minLength'] == 64
    assert signature['maxLength'] == 88
    event = spec['components']['schemas']['AgentEvidence']['properties']['eventBytesBase64']
    assert event['minLength'] == 368
    assert event['maxLength'] == 368
    assert spec['components']['responses']['PayloadTooLarge']['description'].startswith('JSON request exceeds')
