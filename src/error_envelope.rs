//! HTTP JSON error envelope for `sharecli serve` (`docs/ops/error-envelope.md`).

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

/// Wire shape: `{ "error": { "type", "code", "message", "request_id" } }`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ErrorBody {
    #[serde(rename = "type")]
    pub error_type: String,
    pub code: String,
    pub message: String,
    pub request_id: Option<String>,
}

impl ErrorEnvelope {
    pub fn new(error_type: &str, code: &str, message: &str) -> Self {
        Self {
            error: ErrorBody {
                error_type: error_type.to_string(),
                code: code.to_string(),
                message: message.to_string(),
                request_id: None,
            },
        }
    }

    pub fn authentication(code: &str, message: &str) -> Self {
        Self::new("authentication_error", code, message)
    }

    pub fn unauthorized(message: &str) -> Self {
        Self::authentication("unauthorized", message)
    }

    #[allow(dead_code)] // Windows pprof 501 + future validation handlers (FR-004)
    pub fn validation(code: &str, message: &str) -> Self {
        Self::new("validation_error", code, message)
    }

    pub fn not_found(message: &str) -> Self {
        Self::new("not_found_error", "not_found", message)
    }

    pub fn rate_limited(message: &str) -> Self {
        Self::new("rate_limit_error", "rate_limited", message)
    }

    #[allow(dead_code)] // Windows/non-unix pprof path (FR-004)
    pub fn not_implemented(message: &str) -> Self {
        Self::new("not_implemented_error", "not_implemented", message)
    }

    pub fn internal() -> Self {
        Self::new("internal_error", "internal_server_error", "an internal error occurred")
    }

    pub fn into_response(self, status: StatusCode) -> Response {
        (status, Json(self)).into_response()
    }
}

/// Map internal auth failure reasons to user-facing envelope messages.
pub fn auth_failure_message(reason: &str) -> &'static str {
    match reason {
        "missing_authorization" => "missing or invalid bearer token",
        "not_bearer" => "authorization header must use Bearer scheme",
        "invalid_bearer" => "invalid bearer token",
        "jwt_expired" => "jwt token expired",
        "jwt_invalid_iss" => "jwt issuer mismatch",
        "jwt_invalid_aud" => "jwt audience mismatch",
        "jwt_nbf" => "jwt not yet valid",
        "jwt_invalid_sig" => "jwt signature invalid",
        "jwt_no_matching_key" => "jwt signing key not found",
        _ if reason.starts_with("jwt_header:") => "jwt header invalid",
        _ if reason.starts_with("jwt_invalid:") => "jwt token invalid",
        _ if reason.starts_with("jwt_") => "jwt token invalid",
        _ => "missing or invalid bearer token",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unauthorized_envelope_matches_contract() {
        let body = ErrorEnvelope::unauthorized("missing or invalid bearer token");
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "error": {
                    "type": "authentication_error",
                    "code": "unauthorized",
                    "message": "missing or invalid bearer token",
                    "request_id": null
                }
            })
        );
    }

    #[test]
    fn internal_envelope_hides_details() {
        let body = ErrorEnvelope::internal();
        assert_eq!(body.error.error_type, "internal_error");
        assert_eq!(body.error.code, "internal_server_error");
        assert!(!body.error.message.contains("pprof"));
    }

    #[test]
    fn constructors_emit_the_documented_type_and_code_pairs() {
        let cases = [
            (
                ErrorEnvelope::validation("bad_field", "field required"),
                "validation_error",
                "bad_field",
            ),
            (ErrorEnvelope::not_found("no such process"), "not_found_error", "not_found"),
            (ErrorEnvelope::rate_limited("slow down"), "rate_limit_error", "rate_limited"),
            (
                ErrorEnvelope::not_implemented("windows only"),
                "not_implemented_error",
                "not_implemented",
            ),
        ];
        for (envelope, expected_type, expected_code) in cases {
            assert_eq!(envelope.error.error_type, expected_type);
            assert_eq!(envelope.error.code, expected_code);
            assert!(envelope.error.request_id.is_none());
        }
    }

    #[test]
    fn into_response_sets_the_status_and_serialises_the_envelope() {
        let response = ErrorEnvelope::not_found("missing").into_response(StatusCode::NOT_FOUND);
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response.headers().get(axum::http::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()),
            Some("application/json")
        );
    }

    #[test]
    fn auth_failure_message_covers_every_published_reason() {
        let cases = [
            ("missing_authorization", "missing or invalid bearer token"),
            ("not_bearer", "authorization header must use Bearer scheme"),
            ("invalid_bearer", "invalid bearer token"),
            ("jwt_expired", "jwt token expired"),
            ("jwt_invalid_iss", "jwt issuer mismatch"),
            ("jwt_invalid_aud", "jwt audience mismatch"),
            ("jwt_nbf", "jwt not yet valid"),
            ("jwt_invalid_sig", "jwt signature invalid"),
            ("jwt_no_matching_key", "jwt signing key not found"),
            ("jwt_header:bad", "jwt header invalid"),
            ("jwt_invalid:detail", "jwt token invalid"),
            ("jwt_other", "jwt token invalid"),
            ("something_else", "missing or invalid bearer token"),
        ];
        for (reason, expected) in cases {
            assert_eq!(auth_failure_message(reason), expected, "reason {reason}");
        }
    }

    #[test]
    fn auth_failure_message_maps_known_reasons() {
        assert_eq!(
            auth_failure_message("missing_authorization"),
            "missing or invalid bearer token"
        );
        assert_eq!(auth_failure_message("jwt_expired"), "jwt token expired");
    }
}
