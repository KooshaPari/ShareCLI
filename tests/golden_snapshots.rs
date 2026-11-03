// SPDX-License-Identifier: Apache-2.0 OR MIT
//! Golden CLI/TUI snapshot suite (L30.7 / WORK_DAG T-250).
//!
//! Fixtures live under `tests/golden/`. Regenerate with:
//! `UPDATE_GOLDENS=1 cargo test --test golden_snapshots`
//!
//! FR: FR-001 (ps help), NFR CLI surfaces; thermal TUI is packaging/UX polish.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use sharecli_fleet::thermal::ThermalLevel;
use sharecli_fleet::CoalesceMeters;
use sharecli_fleet::ResourceWatchSample;
use sharecli_fleet::SlotQueueMeters;
use sharecli_fuse::{NegDentryMeters, ReadCacheMeters, WriteSerializeMeters};
use sharecli_mesh::MaildirStatus;
use sharecli_thermal_tui::{render, App};

const TUI_W: u16 = 80;
const TUI_H: u16 = 64;

const GOLDEN_WATCH: ResourceWatchSample = ResourceWatchSample {
    fd_count: 16,
    net_rx_bytes: 1024,
    net_tx_bytes: 512,
    mem_rss_bytes: 1_048_576,
    load_1m: 0.42,
};

const GOLDEN_FUSE: ReadCacheMeters = ReadCacheMeters { hits: 3, misses: 1 };
const GOLDEN_NEG: NegDentryMeters = NegDentryMeters { hits: 2, misses: 1 };
const GOLDEN_COALESCE: CoalesceMeters = CoalesceMeters { hits: 6, misses: 2, nocache_runs: 1 };
const GOLDEN_WRITE_SERIALIZE: WriteSerializeMeters =
    WriteSerializeMeters { passthrough_writes: 1, stages: 2, commits: 1, discards: 1 };
const GOLDEN_SLOT_QUEUE: SlotQueueMeters = SlotQueueMeters { acquires: 2, waits: 1, timeouts: 0 };

fn golden_mesh_maildir() -> MaildirStatus {
    MaildirStatus {
        path: std::path::PathBuf::from("/var/sharecli/mesh/queue"),
        ready: 1,
        in_flight: 0,
        pending: 1,
    }
}

fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("golden")
}

fn read_golden(name: &str) -> String {
    let path = golden_dir().join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

fn write_golden(name: &str, contents: &str) {
    let path = golden_dir().join(name);
    fs::create_dir_all(path.parent().unwrap()).expect("create golden dir");
    // Normalize newlines for cross-OS commits (LF).
    let normalized = contents.replace("\r\n", "\n");
    fs::write(&path, normalized).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

fn updating() -> bool {
    matches!(std::env::var("UPDATE_GOLDENS").as_deref(), Ok("1") | Ok("true"))
}

fn assert_or_update(name: &str, actual: &str) {
    let actual = actual.replace("\r\n", "\n");
    if updating() {
        write_golden(name, &actual);
        return;
    }
    let expected = read_golden(name).replace("\r\n", "\n");
    assert_eq!(
        actual, expected,
        "golden mismatch for {name}; re-run with UPDATE_GOLDENS=1 if intentional"
    );
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sharecli"))
}

fn normalize_cli(stdout: &str) -> String {
    stdout.replace("sharecli.exe", "sharecli").replace("\r\n", "\n").trim_end().to_string() + "\n"
}

fn render_thermal(level: ThermalLevel, slots: u32) -> String {
    let backend = TestBackend::new(TUI_W, TUI_H);
    let mut terminal = Terminal::new(backend).expect("terminal");
    let mut app = App::new(4)
        .with_operator_meters(
            Some(GOLDEN_WATCH),
            GOLDEN_FUSE,
            GOLDEN_NEG,
            GOLDEN_COALESCE,
            GOLDEN_SLOT_QUEUE,
            GOLDEN_WRITE_SERIALIZE,
        )
        .with_maildir_status(Some(golden_mesh_maildir()));
    app.update(level, slots);
    terminal.draw(|f| render(f, &app)).expect("draw");
    let buf = terminal.backend().buffer();
    let mut out = String::new();
    for y in 0..TUI_H {
        for x in 0..TUI_W {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

/// CLI `--help` golden (command inventory + Backbone theme flag).
#[test]
fn golden_cli_help() {
    let out = bin().arg("--help").output().expect("sharecli --help");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let actual = normalize_cli(&String::from_utf8_lossy(&out.stdout));
    assert_or_update("cli_help.txt", &actual);
    assert_or_update("help.txt", &actual);
    assert!(actual.contains("Usage: sharecli"), "help MUST name the binary");
    assert!(actual.contains("thermal"), "help MUST list thermal TUI");
}

/// CLI `ps --help` golden (FR-001 list surface).
#[test]
fn golden_cli_ps_help() {
    let out = bin().args(["ps", "--help"]).output().expect("ps --help");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let actual = normalize_cli(&String::from_utf8_lossy(&out.stdout));
    assert_or_update("cli_ps_help.txt", &actual);
    assert!(actual.to_lowercase().contains("project"), "ps help MUST document --project");
}

/// Thermal TUI headless goldens (green / yellow / red) — ≥3 fixtures for T-250.
#[test]
fn golden_thermal_tui_levels() {
    let cases = [
        ("thermal_green.txt", ThermalLevel::Green, 0u32),
        ("thermal_yellow.txt", ThermalLevel::Yellow, 2u32),
        ("thermal_red.txt", ThermalLevel::Red, 4u32),
    ];
    for (name, level, slots) in cases {
        let actual = render_thermal(level, slots);
        assert_or_update(name, &actual);
        match level {
            ThermalLevel::Green => {
                assert!(actual.contains("GREEN") && actual.contains("ADMIT"), "{name}");
            }
            ThermalLevel::Yellow => {
                assert!(actual.contains("YELLOW") && actual.contains("ADMIT"), "{name}");
            }
            ThermalLevel::Red => {
                assert!(actual.contains("RED") && actual.contains("DENY"), "{name}");
            }
        }
        assert!(actual.contains("sharecli thermal monitor"), "{name} MUST show TUI title");
        assert!(
            actual.contains("Host Resource Watch") && actual.contains("Hypervisor IO Meters"),
            "{name} MUST include operator watch panels"
        );
        assert!(
            actual.contains("Coalesce hits:"),
            "{name} MUST include Hypervisor coalesce meters"
        );
        assert!(
            actual.contains("Slot acquires:"),
            "{name} MUST include Hypervisor SlotQueue meters"
        );
        assert!(actual.contains("Mesh ready:"), "{name} MUST include mesh Maildir depth meters");
        assert!(actual.contains("Detected Agents"), "{name} MUST include proc-scan agent panel");
        assert!(
            actual.contains("Runtime Pool") || actual.contains(" pool unavailable"),
            "{name} MUST include runtime pool operator panel (AC-007.71)"
        );
        assert!(
            actual.contains("Proc Scan Status") || actual.contains(" status unavailable"),
            "{name} MUST include proc-scan status operator panel (AC-007.71)"
        );
        assert!(actual.contains("Neg hits:"), "{name} MUST include neg dentry meters");
    }
}
