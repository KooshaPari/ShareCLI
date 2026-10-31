// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-003 acceptance gates for Audit-v38 C02 L24 (Multi-tenant isolation
//! & data privacy) — score-3 commit.
//!
//! Asserts:
//!   1. docs/ops/privacy-tenant.md exists and is the *committed* (not "soft")
//!      variant — no "(soft)" marker in the title.
//!   2. privacy-tenant.md explicitly documents the single-tenant threat model.
//!   3. privacy-tenant.md cross-references BOUNDARY.md and THREAT_MODEL.md.
//!   4. privacy-tenant.md documents ProjectLimits as the only isolation primitive.
//!   5. privacy-tenant.md declares multi-tenant AuthZ as out-of-scope.
//!   6. BOUNDARY.md exists at repo root.
//!   7. THREAT_MODEL.md exists at repo root.
//!   8. ProjectLimits / ProjectLimitsConfig / max_memory_mb are defined in
//!      src/config.rs (the code-level resource isolation primitive).
//!   9. The repo's audit-log artifact (src/audit_log.rs) treats entries as
//!      single-trust-domain (no per-tenant partition key).

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(path: &str) -> String {
    fs::read_to_string(repo_root().join(path)).unwrap_or_else(|e| panic!("read {}: {}", path, e))
}

#[test]
fn fr003_c02_l24_privacy_tenant_doc_is_committed_not_soft() {
    let doc = read("docs/ops/privacy-tenant.md");
    let first_line = doc.lines().next().unwrap_or("");
    assert!(
        !first_line.to_lowercase().contains("(soft)"),
        "privacy-tenant.md first line still has '(soft)' marker: {:?}",
        first_line
    );
    assert!(
        first_line.to_lowercase().contains("privacy & tenancy")
            || first_line.to_lowercase().contains("privacy and tenancy"),
        "privacy-tenant.md title not recognised as Privacy & tenancy: {:?}",
        first_line
    );
}

#[test]
fn fr003_c02_l24_privacy_doc_explicitly_documents_single_tenant_threat_model() {
    let doc = read("docs/ops/privacy-tenant.md");
    let lower = doc.to_lowercase();
    assert!(
        lower.contains("single-tenant") || lower.contains("single operator"),
        "privacy-tenant.md does not document single-tenant / single-operator model"
    );
    assert!(
        lower.contains("not") && lower.contains("multi-tenant"),
        "privacy-tenant.md does not explicitly disclaim multi-tenant posture"
    );
    assert!(
        lower.contains("trust domain") || lower.contains("single trust domain"),
        "privacy-tenant.md does not reference single trust domain"
    );
}

#[test]
fn fr003_c02_l24_privacy_doc_cross_references_boundary_and_threat_model() {
    let doc = read("docs/ops/privacy-tenant.md");
    assert!(
        doc.contains("BOUNDARY.md"),
        "privacy-tenant.md missing cross-reference to BOUNDARY.md"
    );
    assert!(
        doc.contains("THREAT_MODEL.md"),
        "privacy-tenant.md missing cross-reference to THREAT_MODEL.md"
    );
}

#[test]
fn fr003_c02_l24_privacy_doc_documents_project_limits_as_only_primitive() {
    let doc = read("docs/ops/privacy-tenant.md");
    let lower = doc.to_lowercase();
    assert!(lower.contains("projectlimits"), "privacy-tenant.md does not reference ProjectLimits");
    assert!(
        lower.contains("per-project") || lower.contains("per project"),
        "privacy-tenant.md does not clarify ProjectLimits is per-project not per-tenant"
    );
}

#[test]
fn fr003_c02_l24_privacy_doc_declares_multi_tenant_authz_out_of_scope() {
    let doc = read("docs/ops/privacy-tenant.md");
    let lower = doc.to_lowercase();
    assert!(
        (lower.contains("out of scope") || lower.contains("out-of-scope"))
            && lower.contains("multi-tenant"),
        "privacy-tenant.md does not declare multi-tenant AuthZ out-of-scope"
    );
}

#[test]
fn fr003_c02_l24_boundary_md_exists_at_repo_root() {
    let path = repo_root().join("BOUNDARY.md");
    assert!(path.exists(), "BOUNDARY.md missing at repo root: {:?}", path);
}

#[test]
fn fr003_c02_l24_threat_model_md_exists_at_repo_root() {
    let path = repo_root().join("THREAT_MODEL.md");
    assert!(path.exists(), "THREAT_MODEL.md missing at repo root: {:?}", path);
}

#[test]
fn fr003_c02_l24_project_limits_primitive_in_src_config() {
    let config = read("src/config.rs");
    assert!(
        config.contains("ProjectLimitsConfig"),
        "src/config.rs missing ProjectLimitsConfig struct"
    );
    assert!(
        config.contains("max_memory_mb"),
        "src/config.rs ProjectLimitsConfig missing max_memory_mb field"
    );
    assert!(config.contains("project_limits"), "src/config.rs missing project_limits field");
}

#[test]
fn fr003_c02_l24_audit_log_treats_entries_as_single_trust_domain() {
    let audit_log = read("src/audit_log.rs");
    let lower = audit_log.to_lowercase();
    // No per-tenant partition key
    assert!(
        !lower.contains("tenant_id") && !lower.contains("tenant_key"),
        "src/audit_log.rs unexpectedly contains tenant partition key (would violate L24)"
    );
    // Single JSONL stream (single trust domain)
    assert!(
        lower.contains("jsonl") || lower.contains("append"),
        "src/audit_log.rs does not have single-stream append-only JSONL contract"
    );
}
