// SPDX-License-Identifier: Apache-2.0 OR MIT
//! C10 L105 — dashboard hex tokens stay locked to assets/tokens.css / VISUAL_SPEC.
//! FR: FR-003
//!
//! Fails when:
//! - dark `:root` `--bb2-*` hex values diverge between `assets/tokens.css` and
//!   `src/dashboard.html`
//! - `src/dashboard.html` uses a raw `#rrggbb` outside the `:root` token block

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn normalize_hex(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

/// Parse `--name: #rrggbb` assignments from the first `:root { ... }` block.
fn parse_root_bb2_hex(css: &str) -> BTreeMap<String, String> {
    let lower = css.to_ascii_lowercase();
    let start = lower.find(":root").expect("CSS must contain a :root rule");
    let after = &css[start..];
    let open = after.find('{').expect(":root rule must have an opening brace");
    let body = &after[open + 1..];
    let mut depth = 1usize;
    let mut end = 0usize;
    for (i, ch) in body.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    assert!(end > 0, ":root rule must close");
    let root_body = &body[..end];

    let mut out = BTreeMap::new();
    for line in root_body.lines() {
        let line = line.trim();
        if !line.starts_with("--bb2-") {
            continue;
        }
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let value = rest.trim().trim_end_matches(';').trim();
        if value.starts_with('#') {
            out.insert(name.trim().to_string(), normalize_hex(value));
        }
    }
    out
}

fn find_hex_literals(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' && i + 1 < bytes.len() {
            let mut j = i + 1;
            while j < bytes.len() && j < i + 1 + 8 && bytes[j].is_ascii_hexdigit() {
                j += 1;
            }
            let len = j - (i + 1);
            if len == 3 || len == 6 || len == 8 {
                out.push(normalize_hex(&text[i..j]));
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn hex_outside_root_block(html: &str) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let style_start = lower.find("<style>").expect("dashboard must have <style>");
    let style_end = lower[style_start..]
        .find("</style>")
        .map(|i| style_start + i)
        .expect("dashboard style must close");
    let style = &html[style_start..style_end];

    let start =
        style.to_ascii_lowercase().find(":root").expect("dashboard style must contain :root");
    let after = &style[start..];
    let open = after.find('{').expect(":root must open");
    let body = &after[open + 1..];
    let mut depth = 1usize;
    let mut end = 0usize;
    for (i, ch) in body.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    find_hex_literals(&body[end + 1..])
}

/// FR-003 / C10 L105 — dashboard `:root` `--bb2-*` hex lock to tokens.css.
#[test]
fn c10_l105_dashboard_root_matches_tokens_css() {
    let tokens = fs::read_to_string(repo_root().join("assets/tokens.css")).expect("tokens.css");
    let dash = fs::read_to_string(repo_root().join("src/dashboard.html")).expect("dashboard.html");

    let expected = parse_root_bb2_hex(&tokens);
    let actual = parse_root_bb2_hex(&dash);

    assert!(!expected.is_empty(), "tokens.css :root must define --bb2-* hex tokens");

    for (name, hex) in &expected {
        let got = actual.get(name).unwrap_or_else(|| {
            panic!("dashboard.html :root missing {name} (expected {hex} from tokens.css)")
        });
        assert_eq!(got, hex, "dashboard.html {name} drifted from tokens.css ({got} != {hex})");
    }
}

/// FR-003 / C10 L105 — no raw hex outside the dashboard token `:root` block.
#[test]
fn c10_l105_dashboard_no_hex_outside_root() {
    let dash = fs::read_to_string(repo_root().join("src/dashboard.html")).expect("dashboard.html");
    let leaked = hex_outside_root_block(&dash);
    assert!(
        leaked.is_empty(),
        "dashboard.html must not use raw hex outside :root; found: {leaked:?}"
    );
}

/// FR-003 / C10 L105 — VISUAL_SPEC documents the brand token set including error.
#[test]
fn c10_l105_visual_spec_lists_error_token() {
    let spec =
        fs::read_to_string(repo_root().join("docs/visual/VISUAL_SPEC.md")).expect("VISUAL_SPEC.md");
    assert!(
        spec.contains("--bb2-error"),
        "VISUAL_SPEC must list --bb2-error after hex-drift alignment"
    );
    assert!(spec.contains("#f85149"), "VISUAL_SPEC must pin --bb2-error hex");
}
