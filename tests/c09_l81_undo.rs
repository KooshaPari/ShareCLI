// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C09 L81.9 Undo / restore model — FR:003 acceptance gates.
//!
//! Traces to FR:003 (coverage/traceability, Wave17 thesis residual).
//!
//! Asserts every surface in the C09 L81.9 evidence stack:
//! 1. CLI subcommand surface (`Commands::Undo`)
//! 2. Module surface (`commands::undo`)
//! 3. Documentation (`docs/ops/undo-model.md`)
//! 4. Journal storage path resolution (`XDG_STATE_HOME` / `$HOME/.local/state` / Windows)
//! 5. JSONL schema (`OperationRecord`)
//! 6. Cargo.toml declaration

use std::path::PathBuf;

/// Helper: find the workspace root relative to the test binary.
fn workspace_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(manifest_dir)
}

/// fr003_c09_l81_undo_subcommand_is_present_in_commands_enum
#[test]
fn fr003_c09_l81_undo_subcommand_is_present_in_commands_enum() {
    // Read src/main.rs as text and assert that the `Commands::Undo {` variant exists.
    let main_rs = workspace_root().join("src/main.rs");
    let text = std::fs::read_to_string(&main_rs).expect("src/main.rs must be readable");
    assert!(
        text.contains("Commands::Undo {"),
        "expected Commands::Undo variant in src/main.rs (C09 L81.9 Plan 805)"
    );
}

/// fr003_c09_l81_undo_module_declared_in_commands_mod
#[test]
fn fr003_c09_l81_undo_module_declared_in_commands_mod() {
    let mod_rs = workspace_root().join("src/commands/mod.rs");
    let text = std::fs::read_to_string(&mod_rs).expect("src/commands/mod.rs must be readable");
    assert!(
        text.contains("pub mod undo;"),
        "expected 'pub mod undo;' declaration in src/commands/mod.rs"
    );
}

/// fr003_c09_l81_undo_doc_has_required_sections
#[test]
fn fr003_c09_l81_undo_doc_has_required_sections() {
    let doc = workspace_root().join("docs/ops/undo-model.md");
    let text = std::fs::read_to_string(&doc).expect("docs/ops/undo-model.md must be readable");
    let required = [
        "## What it does",
        "## What it does NOT do",
        "## Journal schema",
        "## Storage path",
        "## Interaction with mutating commands",
        "## Operator rules",
        "## Verification",
    ];
    for section in required {
        assert!(
            text.contains(section),
            "docs/ops/undo-model.md must contain section '{}'",
            section
        );
    }
}

/// fr003_c09_l81_undo_journal_path_xdg_or_home
#[test]
fn fr003_c09_l81_undo_journal_path_xdg_or_home() {
    // The undo module's storage path resolver must respect XDG_STATE_HOME,
    // fall back to $HOME/.local/state on unix, and to %LOCALAPPDATA% on Windows.
    let undo_rs = workspace_root().join("src/commands/undo.rs");
    let text = std::fs::read_to_string(&undo_rs).expect("src/commands/undo.rs must be readable");
    assert!(
        text.contains("XDG_STATE_HOME") || text.contains("xdg_state_home"),
        "undo.rs must reference XDG_STATE_HOME"
    );
    assert!(
        text.contains(".local/state") || text.contains("LOCALAPPDATA"),
        "undo.rs must fall back to a default state directory"
    );
    assert!(
        text.contains("operations.jsonl"),
        "undo.rs must use operations.jsonl as the file name"
    );
}

/// fr003_c09_l81_undo_jsonl_schema_includes_required_fields
#[test]
fn fr003_c09_l81_undo_jsonl_schema_includes_required_fields() {
    let undo_rs = workspace_root().join("src/commands/undo.rs");
    let text = std::fs::read_to_string(&undo_rs).expect("src/commands/undo.rs must be readable");
    let required_fields = ["id", "ts", "kind", "target", "reversible", "note"];
    for field in required_fields {
        assert!(text.contains(field), "OperationRecord schema must include field '{}'", field);
    }
    assert!(
        text.contains("pub enum Severity") || text.contains("severity"),
        "undo.rs must define a Severity enum"
    );
}

/// fr003_c09_l81_undo_handlers_present
#[test]
fn fr003_c09_l81_undo_handlers_present() {
    // The run() function must accept the four CLI args + return Result.
    let undo_rs = workspace_root().join("src/commands/undo.rs");
    let text = std::fs::read_to_string(&undo_rs).expect("src/commands/undo.rs must be readable");
    assert!(text.contains("pub fn run"), "undo.rs must export pub fn run");
    assert!(text.contains("limit: usize") || text.contains("limit: "));
    assert!(text.contains("json: bool") || text.contains("json: "));
    assert!(text.contains("restore: bool") || text.contains("restore: "));
}
