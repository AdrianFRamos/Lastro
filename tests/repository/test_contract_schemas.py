from __future__ import annotations

import re
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
        '/api/v2/sponsor/submit',
        '/api/v2/sessions',
        '/api/v2/sessions/{sessionId}',
        '/api/v2/sessions/{sessionId}/confirm',
        '/api/v2/sessions/{sessionId}/manifest',
        '/api/v2/sessions/{sessionId}/observations',
        '/api/v2/sessions/{sessionId}/batches',
        '/api/v2/sessions/{sessionId}/batches/{batchIndex}/confirm',
        '/api/v2/sessions/{sessionId}/revoke',
        '/api/v2/sessions/{sessionId}/sync',
        '/api/v2/facilities/{facilityId}/boundary',
        '/api/v2/facilities/{facilityId}/transaction-data',
        '/api/v2/stations/{stationId}/facility/transaction-data',
        '/api/v2/organizations',
        '/api/v2/organizations/{orgId}/users',
        '/api/v2/organizations/{orgId}/export',
        '/api/v2/organizations/{orgId}/users/{userId}/disable',
        '/api/v2/auth/logout',
        '/api/v2/auth/otp/request',
        '/api/v2/auth/otp/verify',
        '/api/v2/me',
        '/api/v2/org/sign',
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


# Operator-only routes that exist in the router but are not yet described in OpenAPI. This list
# may only shrink: a new route must be documented in schemas/openapi.yaml.
UNDOCUMENTED_OPERATOR_ROUTES = {
    '/api/v2/audit',
    '/api/v2/authority-grants',
    '/api/v2/authority-grants/{grantId}/revoke',
    '/api/v2/custody-transfers',
    '/api/v2/custody-transfers/{transferId}',
    '/api/v2/facilities',
    '/api/v2/facilities/{facilityId}',
    '/api/v2/facilities/{facilityId}/status',
    '/api/v2/lots',
    '/api/v2/lots/{lotId}',
    '/api/v2/parties',
    '/api/v2/parties/{partyId}',
    '/api/v2/parties/{partyId}/status',
    '/api/v2/processing',
    '/api/v2/processing/{operationId}',
    '/api/v2/processing/{operationId}/finalize',
    '/api/v2/processing/{operationId}/ready',
    '/api/v2/recalls',
    '/api/v2/recalls/{recallId}',
    '/api/v2/recalls/{recallId}/close',
    '/api/v2/shipments',
    '/api/v2/shipments/{shipmentId}',
    '/api/v2/shipments/{shipmentId}/status',
}


def test_openapi_and_the_router_describe_the_same_routes():
    """
    PURPOSE: the published contract and the served routes cannot drift apart silently.
    ASSERT: every OpenAPI path is served, and every served path is documented except the frozen
            operator backlog above (which may only shrink).
    FAILURE MEANS: clients integrate against routes that do not exist, or new routes ship
                   undocumented.
    """
    router = (ROOT / 'services/api/src/routes/mod.rs').read_text(encoding='utf-8')
    served = set(re.findall(r'\.route\(\s*"([^"]+)"', router))
    documented = set(yaml.safe_load((ROOT / 'schemas/openapi.yaml').read_text(encoding='utf-8'))['paths'])
    assert documented <= served, sorted(documented - served)
    assert served - documented == UNDOCUMENTED_OPERATOR_ROUTES, sorted(
        (served - documented) ^ UNDOCUMENTED_OPERATOR_ROUTES
    )
