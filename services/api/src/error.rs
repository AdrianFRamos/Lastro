//! Stable API error taxonomy.
//!
//! Route handlers should convert domain/repository/RPC errors into this type so
//! callers can distinguish malformed input (400), missing state (404), authentication
//! (401), canonical/state conflict (409), unavailable dependency (503) and unexpected
//! internal failure (500). Internal responses must never include tokens, DB URLs or keys.

use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("configuration error: {0}")]
    Config(String),
    #[error("invalid request: {0}")]
    Validation(String),
    #[error("unauthorized")]
    Unauthorized,
    #[error("not found: {0}")]
    NotFound(String),
    #[error("state conflict: {0}")]
    Conflict(String),
    #[error("dependency unavailable: {0}")]
    Unavailable(String),
    #[error("internal error")]
    Internal,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody<'a> { code: &'a str, message: &'a str }

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, safe_message) = match &self {
            ApiError::Config(_) | ApiError::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL", "internal error"),
            ApiError::Validation(message) => (StatusCode::BAD_REQUEST, "INVALID_REQUEST", message.as_str()),
            ApiError::Unauthorized => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED", "unauthorized"),
            ApiError::NotFound(message) => (StatusCode::NOT_FOUND, "NOT_FOUND", message.as_str()),
            ApiError::Conflict(message) => (StatusCode::CONFLICT, "CONFLICT", message.as_str()),
            ApiError::Unavailable(message) => (StatusCode::SERVICE_UNAVAILABLE, "UNAVAILABLE", message.as_str()),
        };
        (status, Json(ErrorBody { code, message: safe_message })).into_response()
    }
}
