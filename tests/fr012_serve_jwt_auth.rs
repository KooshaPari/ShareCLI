// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR: FR-012 — Serve HTTP Federated AuthN (JWT / JWKS resource server).
//!
//! AC-012.1 Valid RS256 JWT with matching iss/aud → authorized
//! AC-012.2 Expired JWT → rejected (`jwt_expired`)
//! AC-012.3 Wrong aud/iss rejected; probes stay public (unit coverage)

use std::sync::OnceLock;

use serde::Deserialize;
use sharecli::config::{ServeConfig, ServeJwtConfig};
use sharecli::serve_auth::ServeAuth;

#[derive(Deserialize)]
struct Fixture {
    jwks: serde_json::Value,
    tokens: Tokens,
}

#[derive(Deserialize)]
struct Tokens {
    fr012_valid: [String; 3],
    expired: [String; 3],
    bad_aud: [String; 3],
    bad_iss: [String; 3],
}

fn join_jwt(parts: &[String; 3]) -> String {
    format!("{}.{}.{}", parts[0], parts[1], parts[2])
}

fn fixture() -> &'static Fixture {
    static FIX: OnceLock<Fixture> = OnceLock::new();
    FIX.get_or_init(|| {
        serde_json::from_str(include_str!("fixtures/jwt_static.json")).expect("jwt_static.json")
    })
}

fn jwt_auth() -> ServeAuth {
    let cfg = ServeConfig {
        bearer_token: None,
        auth_mode: Some("jwt".into()),
        jwt: Some(ServeJwtConfig {
            issuer: "https://idp.example/".into(),
            audience: "sharecli-serve".into(),
            jwks_path: None,
            jwks: Some(fixture().jwks.to_string()),
        }),
        ..ServeConfig::default()
    };
    unsafe {
        std::env::remove_var("SHARECLI_SERVE_TOKEN");
        std::env::remove_var("SHARECLI_SERVE_AUTH_MODE");
    }
    ServeAuth::from_env_or_config(&cfg).expect("jwt config")
}

#[test]
fn fr012_valid_jwt_authorized() {
    // FR: FR-012 AC-012.1
    let auth = jwt_auth();
    assert_eq!(auth.mode_label(), "jwt");
    let token = join_jwt(&fixture().tokens.fr012_valid);
    let sub = auth.check_authorization(Some(&format!("Bearer {token}"))).expect("valid jwt");
    assert_eq!(sub.as_deref(), Some("fr012-user"));
}

#[test]
fn fr012_expired_jwt_rejected() {
    // FR: FR-012 AC-012.2
    let auth = jwt_auth();
    let token = join_jwt(&fixture().tokens.expired);
    let err = auth.check_authorization(Some(&format!("Bearer {token}"))).unwrap_err();
    assert_eq!(err, "jwt_expired");
}

#[test]
fn fr012_wrong_audience_and_issuer_rejected() {
    // FR: FR-012 AC-012.3
    let auth = jwt_auth();
    let bad_aud = join_jwt(&fixture().tokens.bad_aud);
    assert_eq!(
        auth.check_authorization(Some(&format!("Bearer {bad_aud}"))).unwrap_err(),
        "jwt_invalid_aud"
    );
    let bad_iss = join_jwt(&fixture().tokens.bad_iss);
    assert_eq!(
        auth.check_authorization(Some(&format!("Bearer {bad_iss}"))).unwrap_err(),
        "jwt_invalid_iss"
    );
}
