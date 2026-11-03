// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Criterion microbench: JWT AuthN validate (FR-012 hot path).
//!
//! Target (draft SLO): `jwt_validate_rs256` p95 < 5 ms per request.
//! See docs/ops/SLO.md § Bench-linked targets and docs/eval/REPRO.md.

use std::hint::black_box;
use std::sync::OnceLock;

use criterion::{criterion_group, criterion_main, Criterion};
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
}

fn fixture() -> &'static Fixture {
    static FIX: OnceLock<Fixture> = OnceLock::new();
    FIX.get_or_init(|| {
        serde_json::from_str(include_str!("../tests/fixtures/jwt_static.json"))
            .expect("jwt_static.json")
    })
}

fn join_jwt(parts: &[String; 3]) -> String {
    format!("{}.{}.{}", parts[0], parts[1], parts[2])
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

fn jwt_validate(c: &mut Criterion) {
    let auth = jwt_auth();
    let token = join_jwt(&fixture().tokens.fr012_valid);
    let header = format!("Bearer {token}");

    c.bench_function("jwt_validate_rs256", |b| {
        b.iter(|| {
            let sub =
                auth.check_authorization(Some(black_box(header.as_str()))).expect("valid jwt");
            black_box(sub)
        });
    });
}

criterion_group!(benches, jwt_validate);
criterion_main!(benches);
