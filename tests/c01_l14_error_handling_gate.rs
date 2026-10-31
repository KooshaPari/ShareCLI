// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C01 L14 — thiserror domain errors + CLI exit codes (FR-003).
//!
//! FR: FR-003

use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// FR-003 / C01 L14 — typed error module with stable exit codes.
#[test]
fn c01_l14_error_rs_thiserror_and_exit_codes() {
    let root = repo_root();
    let error_rs = fs::read_to_string(root.join("src/error.rs")).expect("read src/error.rs");
    let main_rs = fs::read_to_string(root.join("src/main.rs")).expect("read src/main.rs");
    let cargo = fs::read_to_string(root.join("Cargo.toml")).expect("read Cargo.toml");

    assert!(error_rs.contains("thiserror::Error"), "src/error.rs must derive thiserror::Error");
    assert!(error_rs.contains("pub enum SharecliError"), "src/error.rs must define SharecliError");
    assert!(
        error_rs.contains("SHARECLI_ERROR_CODE"),
        "src/error.rs must print SHARECLI_ERROR_CODE for operators"
    );
    for constant in ["EXIT_CONFIG", "EXIT_USAGE", "EXIT_NOT_FOUND", "EXIT_AUTH", "EXIT_INTERNAL"] {
        assert!(error_rs.contains(constant), "src/error.rs must define exit constant {constant}");
    }

    assert!(main_rs.contains("mod error;"), "main.rs must wire mod error");
    assert!(
        main_rs.contains("SharecliError::from") || main_rs.contains("downcast::<SharecliError>"),
        "main.rs must map errors to SharecliError before exit"
    );
    assert!(
        main_rs.contains("exit_code()"),
        "main.rs must map domain errors to process exit codes"
    );
    assert!(cargo.contains("thiserror"), "Cargo.toml must depend on thiserror");
}

/// FR-003 / C01 L14 — config validation uses the config exit code.
#[test]
fn c01_l14_config_validator_exit_code() {
    let src = fs::read_to_string(repo_root().join("src/config_validator.rs"))
        .expect("read config_validator.rs");
    assert!(
        src.contains("crate::error::EXIT_CONFIG"),
        "config_validator must exit with EXIT_CONFIG (78)"
    );
}
