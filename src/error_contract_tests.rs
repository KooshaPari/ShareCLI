//! FR: FR-003 / FR-004 — operator error codes and sanitized HTTP error contracts.

use std::error::Error;
use std::io;
use std::process::ExitCode;

use axum::body::to_bytes;
use axum::http::StatusCode;

use crate::error::{ErrorCode, SharecliError, EXIT_INTERNAL, EXIT_IO};
use crate::error_envelope::{auth_failure_message, ErrorEnvelope};

#[test]
fn operator_codes_remain_compatible_with_runbook_identifiers() {
    for (code, wire) in [
        (ErrorCode::ConfigInvalid, "config_invalid"),
        (ErrorCode::UserInput, "user_input"),
        (ErrorCode::NotFound, "not_found"),
        (ErrorCode::Auth, "auth"),
        (ErrorCode::Io, "io"),
        (ErrorCode::Spawn, "spawn"),
        (ErrorCode::Serve, "serve"),
        (ErrorCode::Internal, "internal"),
    ] {
        assert_eq!(format!("SHARECLI_ERROR_CODE={code}"), format!("SHARECLI_ERROR_CODE={wire}"));
    }
}

#[test]
fn io_conversion_preserves_actionable_source_and_exit_status() {
    let error =
        SharecliError::from(io::Error::new(io::ErrorKind::PermissionDenied, "fixture denied"));
    assert_eq!(error.code(), ErrorCode::Io);
    assert_eq!(error.exit_status(), ExitCode::from(EXIT_IO));
    assert_eq!(error.to_string(), "fixture denied");
    let source = error.source().expect("operator must retain the cause");
    assert_eq!(source.downcast_ref::<io::Error>().unwrap().kind(), io::ErrorKind::PermissionDenied);
}

#[tokio::test]
async fn internal_causes_remain_local_and_are_not_exposed_over_http() {
    let error = SharecliError::from(anyhow::anyhow!("private fixture detail"));
    assert_eq!(error.exit_code(), EXIT_INTERNAL);
    assert_eq!(error.source().unwrap().to_string(), "private fixture detail");

    let response = ErrorEnvelope::internal().into_response(StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(response.headers()["content-type"], "application/json");
    let bytes = to_bytes(response.into_body(), 4096).await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["message"], "an internal error occurred");
    assert!(!String::from_utf8_lossy(&bytes).contains("private fixture detail"));
}

#[tokio::test]
async fn client_error_responses_preserve_status_class_and_request_correlation() {
    for (status, mut envelope, class, code) in [
        (
            StatusCode::BAD_REQUEST,
            ErrorEnvelope::validation("invalid_config", "invalid configuration"),
            "validation_error",
            "invalid_config",
        ),
        (
            StatusCode::NOT_FOUND,
            ErrorEnvelope::not_found("resource missing"),
            "not_found_error",
            "not_found",
        ),
        (
            StatusCode::TOO_MANY_REQUESTS,
            ErrorEnvelope::rate_limited("retry later"),
            "rate_limit_error",
            "rate_limited",
        ),
        (
            StatusCode::NOT_IMPLEMENTED,
            ErrorEnvelope::not_implemented("unsupported platform capability"),
            "not_implemented_error",
            "not_implemented",
        ),
    ] {
        envelope.error.request_id = Some("request-fixture".into());
        let response = envelope.into_response(status);
        assert_eq!(response.status(), status);
        let body: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
        assert_eq!(body["error"]["type"], class);
        assert_eq!(body["error"]["code"], code);
        assert_eq!(body["error"]["request_id"], "request-fixture");
    }
}

#[test]
fn auth_failures_reveal_only_stable_public_diagnostics() {
    for (reason, public) in [
        ("not_bearer", "authorization header must use Bearer scheme"),
        ("invalid_bearer", "invalid bearer token"),
        ("jwt_invalid_iss", "jwt issuer mismatch"),
        ("jwt_invalid_aud", "jwt audience mismatch"),
        ("jwt_nbf", "jwt not yet valid"),
        ("jwt_invalid_sig", "jwt signature invalid"),
        ("jwt_no_matching_key", "jwt signing key not found"),
        ("jwt_header:private-fixture", "jwt header invalid"),
        ("jwt_invalid:private-fixture", "jwt token invalid"),
        ("jwt_unknown_private_fixture", "jwt token invalid"),
        ("unknown private fixture", "missing or invalid bearer token"),
    ] {
        assert_eq!(auth_failure_message(reason), public);
        assert!(!auth_failure_message(reason).contains("fixture"));
    }
}
