//! Public API error-response contracts.

use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};
use lastro_api::error::ApiError;
use serde_json::Value;

async fn response_json(error: ApiError) -> (StatusCode, Value) {
    let response = error.into_response();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 4096)
        .await
        .expect("error response body");
    let body = serde_json::from_slice(&bytes).expect("error response JSON");
    (status, body)
}

#[tokio::test]
async fn validation_errors_are_400_with_stable_public_shape() {
    // PURPOSE: client-correctable input failures must remain distinct from dependency/internal failures.
    let (status, body) = response_json(ApiError::Validation("animalId must be lowercase hex".into())).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "INVALID_REQUEST");
    assert_eq!(body["message"], "animalId must be lowercase hex");

    // ASSERT: validation details are explicit while the response shape stays stable.
    // FAILURE MEANS: clients cannot reliably distinguish input correction from retryable failures.
}

#[tokio::test]
async fn state_conflicts_are_409_with_stable_public_shape() {
    // PURPOSE: canonical/projection conflicts must have deterministic HTTP semantics.
    let (status, body) = response_json(ApiError::Conflict("canonical predecessor changed".into())).await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "CONFLICT");
    assert_eq!(body["message"], "canonical predecessor changed");

    // ASSERT: state conflicts are 409 rather than being misclassified as transport failures.
    // FAILURE MEANS: clients may retry a stale transition as though the service were unavailable.
}

#[tokio::test]
async fn unavailable_dependencies_are_503_without_internal_details() {
    // PURPOSE: callers must distinguish retryable dependency failure from an application bug.
    let (status, body) = response_json(ApiError::Unavailable("Solana RPC request failed".into())).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["code"], "UNAVAILABLE");
    assert_eq!(body["message"], "Solana RPC request failed");

    // ASSERT: dependency failure maps to 503 with the stable dependency-safe message.
    // FAILURE MEANS: clients cannot apply deterministic retry behavior.
}

#[tokio::test]
async fn internal_and_configuration_errors_do_not_leak_secrets() {
    // PURPOSE: configuration/database/token material must never cross the public 500 boundary.
    let secrets = [
        "postgres://lastro:secret@db.internal/lastro",
        "super-secret-agent-token",
        "private-internal-diagnostic",
    ];

    for error in [
        ApiError::Config(format!("bad URL: {}", secrets[0])),
        ApiError::Config(format!("bad token: {}", secrets[1])),
        ApiError::Internal,
    ] {
        let (status, body) = response_json(error).await;
        let serialized = serde_json::to_string(&body).expect("serialize body");
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body["code"], "INTERNAL");
        assert_eq!(body["message"], "internal error");
        for secret in secrets {
            assert!(!serialized.contains(secret));
        }
    }

    // ASSERT: public 500 responses are fixed and contain no rejected configuration or credentials.
    // FAILURE MEANS: operational secrets can leak through an API response.
}
