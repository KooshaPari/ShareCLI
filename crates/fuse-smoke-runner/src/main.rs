// SPDX-License-Identifier: Apache-2.0 OR MIT
//! `fuse-smoke` CLI — OS×arch privileged FUSE mount-smoke matrix (AC-009.22+).

use std::process::ExitCode;

use clap::Parser;
use fuse_smoke_runner::{default_cells_for_host, run_matrix, CellId};

#[derive(Debug, Parser)]
#[command(
    name = "fuse-smoke",
    about = "Run sharecli privileged FUSE mount-smoke matrix cells (AC-009.22+)"
)]
struct Args {
    /// Run a single cell (snake_case id). Repeatable.
    #[arg(long = "cell", value_name = "ID")]
    cells: Vec<String>,

    /// Run every matrix cell (including host-mismatched loud fails).
    #[arg(long)]
    all: bool,

    /// Emit JSON report to stdout (human summary still on stderr).
    #[arg(long)]
    json: bool,

    /// Exit 0 even when cells fail (report only). Default: non-zero if any fail.
    #[arg(long)]
    report_only: bool,
}

fn host_os() -> &'static str {
    if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unknown"
    }
}

fn host_arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else {
        "unknown"
    }
}

fn main() -> ExitCode {
    let args = Args::parse();
    let selected: Vec<CellId> = if args.all {
        CellId::ALL.to_vec()
    } else if !args.cells.is_empty() {
        match args.cells.iter().map(|s| s.parse::<CellId>()).collect::<Result<Vec<_>, _>>() {
            Ok(v) => v,
            Err(e) => {
                eprintln!("fuse-smoke: {e}");
                return ExitCode::from(2);
            }
        }
    } else {
        default_cells_for_host(host_os(), host_arch())
    };

    if selected.is_empty() {
        eprintln!(
            "fuse-smoke: no default cells for host {}/{}; pass --cell or --all",
            host_os(),
            host_arch()
        );
        return ExitCode::from(2);
    }

    let report = match run_matrix(&selected) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("fuse-smoke: {e:#}");
            return ExitCode::from(2);
        }
    };

    for cell in &report.cells {
        let status = if cell.ok { "PASS" } else { "FAIL" };
        let reason = cell.fail_reason.map(|r| format!(" ({r})")).unwrap_or_default();
        eprintln!(
            "[{status}] {}{} — {}",
            cell.cell.as_str(),
            reason,
            if cell.ok {
                cell.detail.lines().next().unwrap_or("").to_string()
            } else {
                // Loud failures: print full detail (multi-line) so operators see ENOSYS/hints.
                cell.detail.trim().to_string()
            }
        );
    }
    eprintln!("matrix: {} ({} cells)", if report.ok { "PASS" } else { "FAIL" }, report.cells.len());

    if args.json {
        match serde_json::to_string_pretty(&report) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("fuse-smoke: json encode failed: {e}");
                return ExitCode::from(2);
            }
        }
    }

    if report.ok || args.report_only {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
