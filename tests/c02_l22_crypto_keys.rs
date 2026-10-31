// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-003 acceptance gates for Audit-v38 C02 L22 (Cryptography & key
//! management) — score-3 commit.
//!
//! Asserts:
//!   1. docs/ops/crypto-keys.md exists and is the *committed* (not "soft")
//!      variant — no "(soft)" marker in the title.
//!   2. crypto-keys.md documents threat surface: Bearer token + JWT as the
//!      only product secret surfaces; audit/history JSONL store no secrets.
//!   3. crypto-keys.md documents the key lifecycle (provisioning, storage,
//!      rotation, disposal).
//!   4. crypto-keys.md enumerates the algorithm inventory: SHA-256 + RS256
//!      as product crypto; non-product util-crypto helpers (xxtea/hkdf/
//!      chacha20/x509_chain/pem_decode) explicitly listed as non-product.
//!   5. crypto-keys.md declares KMS/sealed secrets/hardware keys as
//!      out-of-scope.
//!   6. crypto-keys.md cross-references THREAT_MODEL.md, AUTH.md,
//!      secrets.md, and privacy-tenant.md.
//!   7. src/serve_auth.rs uses sha2::Sha256 for token digest (audited crate).
//!   8. Cargo.toml declares sha2 (and no toy crypto in product deps).
//!   9. Repo root contains THREAT_MODEL.md (referenced by crypto-keys.md).

use std::path::Path;

fn read_repo_file(rel: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {}", path.display(), e))
}

#[test]
fn fr003_c02_l22_crypto_keys_doc_is_committed_not_soft() {
    let doc = read_repo_file("docs/ops/crypto-keys.md");
    let first = doc.lines().next().unwrap_or("");
    assert!(
        !first.to_lowercase().contains("(soft)"),
        "crypto-keys.md title must not contain (soft) marker; got: {}",
        first
    );
}

#[test]
fn fr003_c02_l22_crypto_keys_doc_documents_threat_surface() {
    let doc = read_repo_file("docs/ops/crypto-keys.md");
    assert!(doc.contains("Threat surface"), "crypto-keys.md must include a Threat surface section");
    // Product secret surfaces
    assert!(doc.contains("SHARECLI_SERVE_TOKEN"), "crypto-keys.md must name SHARECLI_SERVE_TOKEN");
    assert!(doc.contains("JWKS"), "crypto-keys.md must name JWKS for JWT path");
    // JSONL stores must be flagged as no-secret surfaces
    assert!(
        doc.contains("Audit") && doc.contains("JSONL"),
        "crypto-keys.md must call out the audit JSONL surface"
    );
    assert!(doc.contains("history"), "crypto-keys.md must call out the history JSONL surface");
}

#[test]
fn fr003_c02_l22_crypto_keys_doc_documents_key_lifecycle() {
    let doc = read_repo_file("docs/ops/crypto-keys.md");
    assert!(doc.contains("Key lifecycle"), "crypto-keys.md must include a Key lifecycle section");
    let lower = doc.to_lowercase();
    for stage in ["provisioning", "storage", "rotation", "disposal"] {
        assert!(lower.contains(stage), "crypto-keys.md lifecycle section must mention {}", stage);
    }
}

#[test]
fn fr003_c02_l22_crypto_keys_doc_enumerates_algorithm_inventory() {
    let doc = read_repo_file("docs/ops/crypto-keys.md");
    assert!(
        doc.contains("Algorithm inventory"),
        "crypto-keys.md must include an Algorithm inventory section"
    );
    // Product crypto
    assert!(doc.contains("SHA-256"), "crypto-keys.md must call out SHA-256 as product crypto");
    assert!(doc.contains("RS256"), "crypto-keys.md must call out RS256 for JWT");
    // Non-product helpers must be explicitly listed
    for helper in ["xxtea", "hkdf", "chacha20", "x509_chain", "pem_decode"] {
        assert!(
            doc.contains(helper),
            "crypto-keys.md must enumerate non-product util helper {}",
            helper
        );
    }
    assert!(
        doc.contains("non-product") || doc.contains("Non-product"),
        "crypto-keys.md must label these helpers as non-product"
    );
}

#[test]
fn fr003_c02_l22_crypto_keys_doc_declares_kms_out_of_scope() {
    let doc = read_repo_file("docs/ops/crypto-keys.md");
    assert!(
        doc.contains("KMS") || doc.contains("Key Vault"),
        "crypto-keys.md must declare KMS/Key Vault as out-of-scope"
    );
    assert!(
        doc.contains("out of scope") || doc.contains("Out of scope"),
        "crypto-keys.md must use the 'out of scope' wording"
    );
    // Hardware keys too
    assert!(
        doc.contains("Hardware") || doc.contains("TPM") || doc.contains("YubiKey"),
        "crypto-keys.md must call out hardware-key paths as out-of-scope"
    );
}

#[test]
fn fr003_c02_l22_crypto_keys_doc_cross_references_threat_model_and_auth() {
    let doc = read_repo_file("docs/ops/crypto-keys.md");
    assert!(doc.contains("THREAT_MODEL.md"), "crypto-keys.md must cross-reference THREAT_MODEL.md");
    assert!(doc.contains("AUTH.md"), "crypto-keys.md must cross-reference AUTH.md");
    assert!(doc.contains("secrets.md"), "crypto-keys.md must cross-reference secrets.md");
    assert!(
        doc.contains("privacy-tenant.md"),
        "crypto-keys.md must cross-reference privacy-tenant.md"
    );
}

#[test]
fn fr003_c02_l22_serve_auth_uses_sha2_crate_for_token_digest() {
    let src = read_repo_file("src/serve_auth.rs");
    assert!(
        src.contains("use sha2") || src.contains("sha2::"),
        "src/serve_auth.rs must use the sha2 crate for token digest"
    );
    assert!(src.contains("Sha256"), "src/serve_auth.rs must name Sha256 explicitly");
}

#[test]
fn fr003_c02_l22_cargo_toml_declares_sha2_no_product_toy_crypto() {
    let cargo = read_repo_file("Cargo.toml");
    assert!(cargo.contains("sha2"), "Cargo.toml must declare sha2 (used by serve_auth)");
    // Toy crypto should not be promoted to a product surface.
    // We don't forbid them existing in optional features, but the
    // default `cargo build` must not link them.
    // The simplest invariant: no `xxtea =` in [dependencies].
    assert!(
        !cargo.lines().any(|l| l.trim_start().starts_with("xxtea =")),
        "Cargo.toml must not promote xxtea into [dependencies] (toy crypto)"
    );
    assert!(
        !cargo.lines().any(|l| l.trim_start().starts_with("chacha20 =")),
        "Cargo.toml must not promote chacha20 into [dependencies]"
    );
}

#[test]
fn fr003_c02_l22_threat_model_md_exists_at_repo_root() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("THREAT_MODEL.md");
    assert!(
        path.exists(),
        "THREAT_MODEL.md must exist at repo root (referenced by crypto-keys.md)"
    );
}
