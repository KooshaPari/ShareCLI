// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-003 acceptance gates for C07 L67 — Fuzz harness
//!
//! These tests assert that the fuzz infrastructure is properly set up,
//! registered, and has seed corpus and CI coverage.

#[test]
fn fr003_fuzz_cargo_toml_exists_and_has_metadata() {
    let content = std::fs::read_to_string("fuzz/Cargo.toml").expect("fuzz/Cargo.toml must exist");
    assert!(
        content.contains("cargo-fuzz = true"),
        "fuzz/Cargo.toml must have cargo-fuzz = true in [package.metadata]"
    );
    assert!(content.contains("libfuzzer-sys"), "fuzz/Cargo.toml must depend on libfuzzer-sys");
}

#[test]
fn fr003_fuzz_has_six_registered_targets() {
    let content = std::fs::read_to_string("fuzz/Cargo.toml").expect("fuzz/Cargo.toml must exist");
    let targets = [
        "toml_lite",
        "dns_query_parser",
        "snmpv3_msg",
        "ssh_packet",
        "coap_option_parse",
        "ldap_filter",
    ];
    for target in &targets {
        assert!(
            content.contains(&format!("name = \"{}\"", target)),
            "fuzz/Cargo.toml must register target '{}'",
            target
        );
    }
}

#[test]
fn fr003_fuzz_target_sources_exist_with_fuzz_target_macro() {
    let targets = [
        "toml_lite",
        "dns_query_parser",
        "snmpv3_msg",
        "ssh_packet",
        "coap_option_parse",
        "ldap_filter",
    ];
    for target in &targets {
        let path = format!("fuzz/fuzz_targets/{}.rs", target);
        let content =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} must exist: {}", path, e));
        assert!(content.contains("fuzz_target!"), "{} must use the fuzz_target! macro", path);
    }
}

#[test]
fn fr003_fuzz_seed_corpus_directories_exist() {
    let targets = [
        "toml_lite",
        "dns_query_parser",
        "snmpv3_msg",
        "ssh_packet",
        "coap_option_parse",
        "ldap_filter",
    ];
    for target in &targets {
        let dir = format!("fuzz/corpora/{}", target);
        assert!(std::path::Path::new(&dir).is_dir(), "Seed corpus directory {} must exist", dir);
        let entries: Vec<_> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("Cannot read {}: {}", dir, e))
            .filter_map(|e| e.ok())
            .collect();
        assert!(
            !entries.is_empty(),
            "Seed corpus directory {} must have at least one seed file",
            dir
        );
    }
}

#[test]
fn fr003_fuzz_ci_workflow_exists_with_matrix_and_artifacts() {
    let path = ".github/workflows/fuzz-soft.yml";
    let content = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("CI workflow {} must exist: {}", path, e));
    assert!(content.contains("matrix:"), "fuzz-soft.yml must define a matrix strategy");
    assert!(content.contains("toml_lite"), "fuzz-soft.yml matrix must include toml_lite target");
    assert!(
        content.contains("upload-artifact"),
        "fuzz-soft.yml must upload crash/corpus artifacts"
    );
    assert!(content.contains("300"), "fuzz-soft.yml must run targets for at least 300s");
}

#[test]
fn fr003_fuzz_directory_structure_matches_expected() {
    let base = std::path::Path::new("fuzz");
    assert!(base.is_dir(), "fuzz/ directory must exist");
    assert!(base.join("Cargo.toml").is_file(), "fuzz/Cargo.toml must exist");
    assert!(base.join("fuzz_targets").is_dir(), "fuzz/fuzz_targets/ must exist");
    assert!(base.join("corpora").is_dir(), "fuzz/corpora/ must exist");
    let targets_dir = base.join("fuzz_targets");
    let target_count = std::fs::read_dir(&targets_dir)
        .expect("Cannot read fuzz_targets/")
        .filter(|e| {
            e.as_ref()
                .map(|f| f.path().extension() == Some(std::ffi::OsStr::new("rs")))
                .unwrap_or(false)
        })
        .count();
    assert!(
        target_count >= 6,
        "fuzz_targets/ must have at least 6 .rs files, found {}",
        target_count
    );
}
