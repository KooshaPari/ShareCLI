// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C01 L18 — runtime secret contract for serve bearer + JWT (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

const SECRET_ENV_VARS: &[&str] = &[
    "SHARECLI_SERVE_TOKEN",
    "SHARECLI_SERVE_AUTH_MODE",
    "SHARECLI_SERVE_JWT_ISSUER",
    "SHARECLI_SERVE_JWT_AUDIENCE",
    "SHARECLI_SERVE_JWKS_PATH",
];

/// FR-003 / C01 L18 — secrets runbook documents serve bearer + JWT contract.
#[test]
fn c01_l18_secrets_md_runtime_contract() {
    let root = repo_root();
    let secrets =
        fs::read_to_string(root.join("docs/ops/secrets.md")).expect("read docs/ops/secrets.md");
    let serve_auth =
        fs::read_to_string(root.join("src/serve_auth.rs")).expect("read src/serve_auth.rs");

    assert!(
        secrets.contains("Runtime contract"),
        "secrets.md must document runtime contract section"
    );
    assert!(
        secrets.contains("src/serve_auth.rs"),
        "secrets.md must cite serve_auth implementation"
    );
    for var in SECRET_ENV_VARS {
        assert!(secrets.contains(var), "secrets.md must document env var {var}");
        assert!(serve_auth.contains(var), "serve_auth.rs must read env var {var}");
    }

    assert!(
        secrets.contains("SHA-256"),
        "secrets.md must document bearer digest comparison policy"
    );
    assert!(secrets.contains("HS"), "secrets.md must note HS* JWT rejection");
    assert!(secrets.contains("Never commit"), "secrets.md must forbid committing real secrets");
}

/// FR-003 / C01 L18 — example env stays placeholder-only for serve token.
#[test]
fn c01_l18_env_example_placeholder_only() {
    let example = fs::read_to_string(repo_root().join(".env.example")).expect("read .env.example");
    assert!(
        example.contains("SHARECLI_SERVE_TOKEN"),
        ".env.example must document SHARECLI_SERVE_TOKEN placeholder"
    );
    assert!(
        !example.contains("sk-") && !example.contains("Bearer eyJ"),
        ".env.example must not contain production-shaped secrets"
    );
}
