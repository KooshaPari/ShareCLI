//! FR-004 NFR — C09 L81.3 keyboard Tab-cycle + L81.8 design-system doc.
//! FR: FR-004

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use sharecli_thermal_tui::{apply_key_action, App, KeyAction, PanelFocus};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn fr004_l81_3_thermal_tui_keyboard_matrix() {
    let doc =
        fs::read_to_string(repo_root().join("docs/a11y/keyboard.md")).expect("read keyboard.md");
    for needle in ["Tab", "Shift-Tab", "`r`", "`?`", "handle_key"] {
        assert!(doc.contains(needle), "keyboard.md must document {needle}");
    }

    let design = fs::read_to_string(repo_root().join("docs/a11y/design-system.md"))
        .expect("read design-system.md");
    assert!(design.contains("handle_key"), "design-system must reference handle_key");

    let mut app = App::new(4);
    assert_eq!(app.focus, PanelFocus::Gate);
    apply_key_action(&mut app, KeyAction::FocusNext);
    assert_eq!(app.focus, PanelFocus::Pool);
    apply_key_action(&mut app, KeyAction::FocusPanel(PanelFocus::Agents));
    assert_eq!(app.focus, PanelFocus::Agents);
    apply_key_action(&mut app, KeyAction::ToggleHelp);
    assert!(app.show_help_overlay);
}

#[test]
fn fr004_l81_8_design_system_doc_present() {
    let path = repo_root().join("docs/a11y/design-system.md");
    let body = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(body.contains("tokens.css"), "design-system must reference token SoT");
    assert!(body.contains("tabindex=\"-1\"") || body.contains("main-content"));
    assert!(body.contains("stop"), "terminology table must document stop");
}

#[test]
fn fr004_l81_3_playwright_keyboard_script_present() {
    let path = repo_root().join("scripts/a11y/playwright_keyboard.mjs");
    let body = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    assert!(body.contains("skip-link"), "keyboard script must assert skip link focus");
    assert!(body.contains("main-content"), "keyboard script must assert skip target");
}

#[test]
fn fr004_dashboard_main_is_skip_target() {
    let html =
        fs::read_to_string(repo_root().join("src/dashboard.html")).expect("read dashboard.html");
    assert!(
        html.contains(r#"id="main-content" tabindex="-1""#),
        "main must be focusable skip-link target"
    );
}

#[test]
fn fr004_a11y_keyboard_npm_script_registered() {
    let pkg = fs::read_to_string(repo_root().join("package.json")).expect("read package.json");
    assert!(pkg.contains(r#""a11y:keyboard"#), "package.json must expose a11y:keyboard for CI");
}

/// Smoke: axe dashboard still passes after keyboard/doc changes (no browser required).
#[test]
fn fr004_axe_dashboard_still_passes() {
    // Skip gracefully when npm is not installed (CI Ubuntu runners may not have it).
    let npm_available = Command::new("npm").arg("--version").output().map(|o| o.status.success()).unwrap_or(false);
    if !npm_available {
        eprintln!("skipping fr004_axe_dashboard_still_passes: npm not installed");
        return;
    }
    let out = Command::new("npm")
        .args(["run", "a11y:dashboard"])
        .current_dir(repo_root())
        .output()
        .expect("npm run a11y:dashboard");
    assert!(out.status.success(), "axe dashboard: {}", String::from_utf8_lossy(&out.stderr));
}
