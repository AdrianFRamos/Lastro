from __future__ import annotations

from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]


def _spec():
    return yaml.safe_load((ROOT / 'schemas/openapi.yaml').read_text(encoding='utf-8'))


def test_openapi_contains_exact_v2_surface_and_no_v1_or_compliance_endpoint():
    paths = set(_spec()['paths'])
    expected = {
        '/api/health',
        '/api/captures/authorization-challenge',
        '/api/captures',
        '/api/captures/{captureId}',
        '/api/agent/commands',
        '/api/agent/evidence',
        '/api/agent/evidence/{eventHash}',
        '/api/v2/agent/observations',
        '/api/v2/events/{eventHash}',
        '/api/v2/events/{eventHash}/transaction-data',
        '/api/v2/events/{eventHash}/submit',
        '/api/v2/events/{eventHash}/confirm',
        '/api/v2/assets/transaction-data',
        '/api/v2/assets/by-rfid/{rfidHash}',
        '/api/v2/assets/{assetId}',
        '/api/v2/assets/{assetId}/sync',
        '/api/v2/assets/{assetId}/evidence-package',
        '/api/v2/assets/{assetId}/timeline',
        '/api/v2/assets/{assetId}/lineage',
        '/api/v2/transformations',
        '/api/v2/transformations/{transformationId}',
        '/api/v2/transformations/{transformationId}/sync',
        '/api/v2/custody-transfers/{transferId}/transaction-data',
        '/api/v2/custody-transfers/{transferId}/accept',
    }
    assert paths == expected
    lowered = '\n'.join(paths).lower()
    assert '/api/animals' not in lowered and '/api/events' not in lowered
    assert 'eudr' not in lowered and 'compliance' not in lowered and 'credit' not in lowered


def test_openapi_transaction_contract_freezes_instruction_count_and_1232_byte_limit():
    tx = _spec()['components']['schemas']['TransactionData']['properties']
    instructions = tx['instructions']
    # One or two protocol instructions, plus at most two ComputeBudget priority-fee instructions.
    assert instructions['minItems'] == 1
    assert instructions['maxItems'] == 4
    assert tx['measuredSerializedBytes']['maximum'] == 1232
    assert tx['transactionVersion']['enum'] == ['legacy', 'v0']


def test_openapi_submission_lifecycle_is_explicit():
    """Confirmed transaction registration must be distinct from finalized canonical state."""
    spec = _spec()
    assert spec['components']['schemas']['DomainEventStatus']['enum'] == [
        'EVIDENCE_ACCEPTED', 'SUBMITTED', 'FINALIZED', 'REJECTED',
    ]
    for step in ('submit', 'confirm'):
        response = spec['paths'][f'/api/v2/events/{{eventHash}}/{step}']['post']['responses']['200']
        assert response['content']['application/json']['schema']['$ref'].endswith('/DomainEventAnchor')


def test_openapi_agent_command_cannot_supply_observed_rfid():
    """The Station must read the tag itself; the backend only states what the chain expects."""
    properties = _spec()['components']['schemas']['AgentCommand']['properties']
    assert 'expectedRfidHash' in properties
    assert not {'newRfidHash', 'observedRfid', 'observedRfidHex', 'payloadHash'} & set(properties)


def test_openapi_capture_actions_are_the_three_physical_v2_operations():
    schemas = _spec()['components']['schemas']
    expected = ['BIND_IDENTIFIER', 'REPLACE_IDENTIFIER', 'OBSERVE_PRESENCE']
    assert schemas['CaptureIntentRequest']['properties']['action']['enum'] == expected
    assert schemas['CreateCaptureRequest']['properties']['action']['enum'] == expected


def test_openapi_freezes_http_and_protocol_resource_bounds():
    """The public contract must match the application limits enforced before expensive work."""
    spec = _spec()
    schemas = spec['components']['schemas']
    assert '1024 bytes' in spec['info']['description']
    assert schemas['SolanaSignature']['minLength'] == 64
    assert schemas['SolanaSignature']['maxLength'] == 88
    # 220-byte domain envelope == 296 Base64 characters, everywhere it is transported.
    for envelope in (
        schemas['AgentEvidence']['properties']['envelopeBase64'],
        schemas['DomainEventAnchor']['properties']['envelopeBytesBase64'],
    ):
        assert envelope['minLength'] == envelope['maxLength'] == 296
    assert schemas['AgentEvidence']['properties']['observedRfidHex']['pattern'] == '^[0-9a-f]{16}$'
    assert schemas['EvidencePackage']['properties']['schema']['const'] == 'lastro.evidence-package.v2'
    assert spec['components']['responses']['PayloadTooLarge']['description'].startswith('JSON request exceeds')
