// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C00 / FR-004 — unified HTTP error envelope for `sharecli serve`.
//!
//! Golden contract: `docs/ops/error-envelope.md`.
//! FR: FR-004

use sharecli::error_envelope::{auth_failure_message, ErrorEnvelope};

#[test]
fn fr004_auth_401_envelope_golden_bytes() {
    let body = ErrorEnvelope::unauthorized(auth_failure_message("missing_authorization"));
    let bytes = serde_json::to_vec(&body).expect("serialize envelope");
    assert_eq!(
        bytes,
        br#"{"error":{"type":"authentication_error","code":"unauthorized","message":"missing or invalid bearer token","request_id":null}}"#
    );
}

#[test]
fn fr004_internal_envelope_is_generic() {
    let body = ErrorEnvelope::internal();
    let json = serde_json::to_value(&body).unwrap();
    assert_eq!(json["error"]["type"], "internal_error");
    assert_eq!(json["error"]["code"], "internal_server_error");
    assert_eq!(json["error"]["message"], "an internal error occurred");
}

#[test]
fn fr004_not_found_envelope_for_disabled_pprof() {
    let body = ErrorEnvelope::not_found("profiling disabled; set SHARECLI_PPROF=1 to enable");
    let json = serde_json::to_value(&body).unwrap();
    assert_eq!(json["error"]["type"], "not_found_error");
    assert_eq!(json["error"]["code"], "not_found");
}
