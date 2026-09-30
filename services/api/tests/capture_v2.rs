//! End-to-end API contracts for the v2 physical capture flow (PostgreSQL-backed).

mod common;

use std::sync::Arc;

use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use ed25519_dalek::Signer;
use lastro_api::solana::rpc::CanonicalV2EventAnchor;
use lastro_protocol::{
    crypto::derive_station_id,
    rfid::{canonical_rfid_from_u64, hash_canonical_rfid},
    v2::{CaptureCommand, EventType, capture_envelope, capture_event_id},
};
use serde_json::{Value, json};
use tower::ServiceExt;

use common::{
    AGENT_TOKEN, DEPLOYMENT_ID, STATION_PUBKEY, TestDb, TestRpc, app_state, station_sign,
    untagged_asset, wallet,
};

const ASSET: [u8; 32] = [0x11; 32];

async fn call(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
    agent: bool,
) -> (StatusCode, Value) {
    let mut builder = Request::builder().method(method).uri(uri);
    if agent {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {AGENT_TOKEN}"));
    }
    let request = match body {
        Some(value) => builder
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(value.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 256 * 1024).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

/// Challenge + wallet signature + capture creation, as the browser does it.
async fn create_capture(app: &Router, action: &str, signer_seed: u8) -> (StatusCode, Value) {
    let intent = json!({"action": action, "assetId": hex::encode(ASSET)});
    let (status, challenge) = call(
        app,
        "POST",
        "/api/captures/authorization-challenge",
        Some(intent),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{challenge}");
    let message = BASE64
        .decode(challenge["messageBase64"].as_str().unwrap())
        .unwrap();
    let signature = wallet(signer_seed).sign(&message);
    let body = json!({
        "action": action,
        "assetId": hex::encode(ASSET),
        "authorization": {
            "challengeId": challenge["challengeId"],
            "signatureBase64": BASE64.encode(signature.to_bytes()),
        }
    });
    call(app, "POST", "/api/captures", Some(body), false).await
}

fn command_from(json: &Value) -> CaptureCommand {
    let capture_id = *uuid::Uuid::parse_str(json["captureId"].as_str().unwrap())
        .unwrap()
        .as_bytes();
    let hex32 = |key: &str| -> [u8; 32] {
        hex::decode(json[key].as_str().unwrap())
            .unwrap()
            .try_into()
            .unwrap()
    };
    CaptureCommand {
        capture_id,
        event_type: match json["eventType"].as_str().unwrap() {
            "IDENTIFIER_BOUND" => EventType::IdentifierBound,
            "IDENTIFIER_REPLACED" => EventType::IdentifierReplaced,
            _ => EventType::ObservationRecorded,
        },
        deployment_id: hex32("deploymentId"),
        asset_id: hex32("assetId"),
        event_id: capture_event_id(&capture_id),
        state_version: json["stateVersion"].as_u64().unwrap(),
        previous_event_hash: hex32("previousEventHash"),
        expected_rfid_hash: hex32("expectedRfidHash"),
        observed_at: json["observedAt"].as_i64().unwrap(),
        expires_at: json["expiresAt"].as_i64().unwrap(),
    }
}

/// What the Station would emit for `command` after reading `tag`.
fn evidence(command: &CaptureCommand, tag: u64) -> (Value, [u8; 32]) {
    let rfid = canonical_rfid_from_u64(tag);
    let envelope = capture_envelope(command, &rfid, derive_station_id(&STATION_PUBKEY).unwrap())
        .expect("tag satisfies the capture rule");
    let bytes = envelope.encode().unwrap();
    let body = json!({
        "captureId": uuid::Uuid::from_bytes(command.capture_id),
        "envelopeBase64": BASE64.encode(bytes),
        "observedRfidHex": hex::encode(rfid),
        "stationPubkeyHex": hex::encode(STATION_PUBKEY),
        "stationSignatureHex": hex::encode(station_sign(&bytes)),
    });
    (body, envelope.event_hash().unwrap())
}

#[tokio::test]
async fn bind_identifier_capture_reaches_finalized_evidence_package() {
    // PURPOSE: the complete v2 physical flow — wallet-authorized capture, Station evidence,
    //   custodian transaction, finalized confirmation and a public evidence package.
    // ARRANGE: an untagged animal whose custodian is wallet(7).
    // ACTION: challenge → capture → Agent command → evidence → tx data → submit → confirm.
    // ASSERT: each step advances exactly once and the package carries the observed RFID.
    // FAILURE MEANS: the RFID binding of an animal could not be proven end to end.
    let db = TestDb::new().await;
    let rpc = Arc::new(TestRpc::configured());
    let custodian = wallet(7).verifying_key().to_bytes();
    rpc.set_asset(untagged_asset(ASSET, custodian));
    let app = lastro_api::routes::router(app_state(db.pool.clone(), rpc.clone()));

    let (status, capture) = create_capture(&app, "BIND_IDENTIFIER", 7).await;
    assert_eq!(status, StatusCode::CREATED, "{capture}");
    assert_eq!(capture["status"], "PENDING");
    assert_eq!(capture["stateVersion"], 1);

    let (status, command) = call(&app, "GET", "/api/agent/commands", None, true).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(command["eventType"], "IDENTIFIER_BOUND");
    assert_eq!(command["expectedRfidHash"], hex::encode([0u8; 32]));
    let command = command_from(&command);

    let (body, event_hash) = evidence(&command, 0x8000_1300_0000_0001);
    let (status, _) = call(
        &app,
        "POST",
        "/api/agent/evidence",
        Some(body.clone()),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = call(&app, "POST", "/api/agent/evidence", Some(body), true).await;
    assert_eq!(status, StatusCode::OK, "exact Agent retry is idempotent");

    let hash_hex = hex::encode(event_hash);
    let (status, tx) = call(
        &app,
        "GET",
        &format!("/api/v2/events/{hash_hex}/transaction-data"),
        None,
        false,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{tx}");
    assert_eq!(tx["requiredSigner"], bs58::encode(custodian).into_string());
    assert_eq!(tx["instructions"].as_array().unwrap().len(), 2);

    let signature = bs58::encode([7u8; 64]).into_string();
    let (status, submitted) = call(
        &app,
        "POST",
        &format!("/api/v2/events/{hash_hex}/submit"),
        Some(json!({"txSignature": signature})),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{submitted}");
    assert_eq!(submitted["status"], "SUBMITTED");

    // The chain now reflects the finalized bind.
    let tag_hash = hash_canonical_rfid(&canonical_rfid_from_u64(0x8000_1300_0000_0001));
    let mut bound = untagged_asset(ASSET, custodian);
    bound.state_version = 1;
    bound.event_sequence = 1;
    bound.last_event_hash = event_hash;
    bound.current_rfid_hash = tag_hash;
    rpc.set_asset(bound);
    let envelope = capture_envelope(
        &command,
        &canonical_rfid_from_u64(0x8000_1300_0000_0001),
        derive_station_id(&STATION_PUBKEY).unwrap(),
    )
    .unwrap();
    rpc.set_anchor(CanonicalV2EventAnchor {
        event_id: envelope.event_id,
        deployment_id: DEPLOYMENT_ID,
        subject_id: ASSET,
        source_id: envelope.source_id,
        event_type: envelope.event_type,
        state_version: 1,
        observed_at: envelope.observed_at,
        expires_at: envelope.expires_at,
        expected_previous_hash: [0; 32],
        payload_hash: envelope.payload_hash,
        event_hash,
    });
    let (status, finalized) = call(
        &app,
        "POST",
        &format!("/api/v2/events/{hash_hex}/confirm"),
        Some(json!({"txSignature": bs58::encode([7u8; 64]).into_string()})),
        false,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{finalized}");
    assert_eq!(finalized["status"], "FINALIZED");

    let (status, package) = call(
        &app,
        "GET",
        &format!("/api/v2/assets/{}/evidence-package", hex::encode(ASSET)),
        None,
        false,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(package["schema"], "lastro.evidence-package.v2");
    assert_eq!(package["asset"]["currentRfidHash"], hex::encode(tag_hash));
    assert_eq!(package["events"].as_array().unwrap().len(), 1);
    assert_eq!(package["events"][0]["observedRfidHex"], "8000130000000001");

    // A finalized capture is complete: the next transition on the asset must not be blocked by it.
    let (status, next) = create_capture(&app, "REPLACE_IDENTIFIER", 7).await;
    assert_eq!(status, StatusCode::CREATED, "{next}");
    assert_eq!(next["stateVersion"], 2);
    db.cleanup().await;
}

#[tokio::test]
async fn only_the_canonical_custodian_can_authorize_an_identity_capture() {
    // PURPOSE: the wallet that signs the challenge must be the on-chain custodian.
    // ARRANGE: custodian is wallet(7); an attacker signs with wallet(9).
    // ACTION: create the capture with the attacker's signature.
    // ASSERT: 401 and no command is dispatched to the Station.
    // FAILURE MEANS: anyone could trigger identity changes for animals they do not hold.
    let db = TestDb::new().await;
    let rpc = Arc::new(TestRpc::configured());
    rpc.set_asset(untagged_asset(ASSET, wallet(7).verifying_key().to_bytes()));
    let app = lastro_api::routes::router(app_state(db.pool.clone(), rpc));

    let (status, _) = create_capture(&app, "BIND_IDENTIFIER", 9).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (_, command) = call(&app, "GET", "/api/agent/commands", None, true).await;
    assert!(command.is_null());
    db.cleanup().await;
}

#[tokio::test]
async fn capture_rules_follow_the_canonical_rfid_state() {
    // PURPOSE: BIND needs an untagged asset; REPLACE/OBSERVE need an active tag.
    // ARRANGE: an untagged asset.
    // ACTION: request REPLACE, then OBSERVE.
    // ASSERT: both are refused with 409 before any challenge is issued.
    // FAILURE MEANS: the Station could be asked to sign transitions the chain must reject.
    let db = TestDb::new().await;
    let rpc = Arc::new(TestRpc::configured());
    rpc.set_asset(untagged_asset(ASSET, wallet(7).verifying_key().to_bytes()));
    let app = lastro_api::routes::router(app_state(db.pool.clone(), rpc));
    for action in ["REPLACE_IDENTIFIER", "OBSERVE_PRESENCE"] {
        let (status, _) = call(
            &app,
            "POST",
            "/api/captures/authorization-challenge",
            Some(json!({"action": action, "assetId": hex::encode(ASSET)})),
            false,
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT, "{action}");
    }
    db.cleanup().await;
}

#[tokio::test]
async fn tampered_or_foreign_evidence_is_rejected() {
    // PURPOSE: only the exact Station-signed envelope for the dispatched capture is admitted.
    // ARRANGE: a dispatched BIND capture.
    // ACTION: submit evidence with a forged signature, then with a different capture's context.
    // ASSERT: both are rejected and the capture remains open for genuine evidence.
    // FAILURE MEANS: forged or replayed physical evidence could reach the chain pipeline.
    let db = TestDb::new().await;
    let rpc = Arc::new(TestRpc::configured());
    rpc.set_asset(untagged_asset(ASSET, wallet(7).verifying_key().to_bytes()));
    let app = lastro_api::routes::router(app_state(db.pool.clone(), rpc));
    let (status, _) = create_capture(&app, "BIND_IDENTIFIER", 7).await;
    assert_eq!(status, StatusCode::CREATED);
    let (_, command) = call(&app, "GET", "/api/agent/commands", None, true).await;
    let command = command_from(&command);

    let (mut forged, _) = evidence(&command, 0x8000_1300_0000_0001);
    forged["stationSignatureHex"] = json!(hex::encode(station_sign(b"something else")));
    let (status, _) = call(&app, "POST", "/api/agent/evidence", Some(forged), true).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let mut other = command;
    other.state_version = 5;
    let (mut foreign, _) = evidence(&other, 0x8000_1300_0000_0001);
    foreign["captureId"] = json!(uuid::Uuid::from_bytes(command.capture_id));
    let (status, _) = call(&app, "POST", "/api/agent/evidence", Some(foreign), true).await;
    assert_eq!(status, StatusCode::CONFLICT);

    let (genuine, _) = evidence(&command, 0x8000_1300_0000_0001);
    let (status, _) = call(&app, "POST", "/api/agent/evidence", Some(genuine), true).await;
    assert_eq!(status, StatusCode::CREATED);
    db.cleanup().await;
}
