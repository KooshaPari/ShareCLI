// SPDX-License-Identifier: Apache-2.0 OR MIT
//! FR-005 — limits persist across processes and are checked against measurement.
//! FR: FR-005, FR-003
//!
//! Covers the two gaps this slice closed:
//!
//! * a limit set in one invocation is visible to the next (it lives in
//!   `limits.json`, not in an in-memory map), and
//! * `limits check` / `optimize` report *detected* usage and never present an
//!   unmeasured project as within budget (`NO_DATA`, not `OK`).
//!
//! Every scenario runs the real binary in an isolated HOME / config / limits
//! path, so nothing here depends on the developer's own `~/.sharecli`.

#![cfg(unix)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};

use serde_json::Value;

fn bin() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sharecli"));
    cmd.env_remove("SHARECLI_LIMITS_PATH");
    cmd
}

struct Sandbox {
    _dir: tempfile::TempDir,
    root: PathBuf,
    limits: PathBuf,
    config: PathBuf,
}

impl Sandbox {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path().to_path_buf();
        let limits = root.join("state").join("limits.json");
        let config = root.join("config").join("config.toml");
        std::fs::create_dir_all(root.join("config")).expect("config dir");
        std::fs::create_dir_all(root.join("state")).expect("state dir");
        std::fs::write(&config, "[projects]\nalpha = \"/tmp/alpha\"\nbeta = \"/tmp/beta\"\n")
            .expect("write config");
        Self { _dir: dir, root, limits, config }
    }

    fn cmd(&self) -> Command {
        let mut cmd = bin();
        cmd.env("HOME", &self.root)
            .env("XDG_CONFIG_HOME", self.root.join("config"))
            .env("XDG_STATE_HOME", self.root.join("state"))
            .env("XDG_DATA_HOME", self.root.join("data"))
            .env("SHARECLI_CONFIG_PATH", &self.config)
            .env("SHARECLI_LIMITS_PATH", &self.limits)
            .env("NO_COLOR", "1")
            .env("TERM", "dumb");
        cmd
    }

    fn run(&self, args: &[&str]) -> Output {
        self.cmd().args(args).output().expect("spawn sharecli")
    }

    fn ok(&self, args: &[&str]) -> (String, String) {
        let out = self.run(args);
        assert!(
            out.status.success(),
            "sharecli {:?} MUST succeed; status={:?}\nstdout={}\nstderr={}",
            args,
            out.status.code(),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        (
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
    }

    fn limits_json(&self) -> Value {
        let raw = std::fs::read_to_string(&self.limits).expect("limits.json exists");
        serde_json::from_str(&raw).expect("limits.json parses")
    }
}

/// Stage a process the detector recognises as a `claude` agent.
///
/// Detection is identity-first: a process is an agent when its own name is the
/// family. Shells are deliberately not launch wrappers (so
/// `bash -c "... kcode ..."` stays UNKNOWN), which means a `sh <dir>/claude`
/// script is no longer detectable evidence. The fixture therefore stages a real
/// process whose identity is `claude`: a copy of a rename-safe executable run
/// directly under that name. Coreutils is multi-call on NixOS and refuses an
/// unknown program name, so the copy is `bash`, kept alive with a sleep loop.
fn find_on_path(name: &str) -> PathBuf {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| panic!("{name} not found on PATH"))
}

fn spawn_staged_agent(dir: &Path) -> Child {
    let staged = dir.join("claude");
    std::fs::copy(find_on_path("bash"), &staged).expect("stage a blocking binary as `claude`");
    Command::new(&staged)
        .args(["-c", "while :; do sleep 1; done"])
        .spawn()
        .expect("spawn staged agent")
}

#[test]
fn fr005_set_survives_the_invocation_that_set_it() {
    let sandbox = Sandbox::new();
    let (stdout, _) =
        sandbox.ok(&["limits", "set", "--global", "--mem-mb", "512", "--max-procs", "4"]);
    assert!(stdout.contains("512 MB / 4 procs"), "set MUST echo the values; got: {stdout}");

    // A separate process reads it back.
    let (stdout, _) = sandbox.ok(&["limits"]);
    assert!(stdout.contains("(global)"), "bare limits MUST show the global row; got: {stdout}");
    assert!(
        stdout.contains("global-override"),
        "global MUST be reported as persisted; got: {stdout}"
    );
    assert!(stdout.contains("512"), "global memory limit MUST be read back; got: {stdout}");

    let stored = sandbox.limits_json();
    assert_eq!(stored["global"]["memory_limit_mb"], 512);
    assert_eq!(stored["global"]["max_processes"], 4);
}

#[test]
fn fr005_project_override_wins_over_global_in_a_fresh_process() {
    let sandbox = Sandbox::new();
    sandbox.ok(&["limits", "set", "--global", "--mem-mb", "2048", "--max-procs", "8"]);
    sandbox.ok(&["limits", "set", "alpha", "--mem-mb", "512", "--max-procs", "3"]);

    let (stdout, _) = sandbox.ok(&["limits"]);
    let alpha_line = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("alpha"))
        .unwrap_or_else(|| panic!("alpha row missing; got: {stdout}"));
    assert!(alpha_line.contains("512"), "project override MUST win; got: {alpha_line}");
    assert!(alpha_line.contains("project-override"), "got: {alpha_line}");
    let beta_line = stdout
        .lines()
        .find(|line| line.trim_start().starts_with("beta"))
        .unwrap_or_else(|| panic!("beta row missing; got: {stdout}"));
    assert!(beta_line.contains("2048"), "beta MUST inherit the global; got: {beta_line}");
    assert!(beta_line.contains("global-override"), "got: {beta_line}");

    // The single-project read-only view agrees.
    let (single, _) = sandbox.ok(&["limits", "alpha"]);
    assert!(single.contains("Source: project-override"), "got: {single}");
    assert!(single.contains("Memory limit: 512 MB"), "got: {single}");

    // Unrelated project keeps the config default (1024 / 10).
    let stored = sandbox.limits_json();
    assert_eq!(stored["projects"]["alpha"]["max_processes"], 3);
}

#[cfg(unix)]
#[test]
fn fr005_limits_file_and_dir_are_user_only() {
    use std::os::unix::fs::PermissionsExt as _;
    let sandbox = Sandbox::new();
    sandbox.ok(&["limits", "set", "--global", "--mem-mb", "256", "--max-procs", "2"]);

    let file_mode = std::fs::metadata(&sandbox.limits).expect("stat limits").permissions().mode();
    assert_eq!(file_mode & 0o777, 0o600, "limits.json MUST be 0600, got {file_mode:o}");
    let dir = sandbox.limits.parent().expect("parent");
    let dir_mode = std::fs::metadata(dir).expect("stat dir").permissions().mode();
    assert_eq!(dir_mode & 0o777, 0o700, "limits dir MUST be 0700, got {dir_mode:o}");
}

#[test]
fn fr005_check_json_reports_detection_and_verdicts() {
    let sandbox = Sandbox::new();
    sandbox.ok(&["limits", "set", "--global", "--mem-mb", "4096", "--max-procs", "32"]);

    let (stdout, _) = sandbox.ok(&["limits", "check", "--json"]);
    let parsed: Value = serde_json::from_str(&stdout).expect("check --json MUST emit JSON");
    assert_eq!(
        parsed["limits_file"].as_str().expect("limits_file path"),
        sandbox.limits.display().to_string(),
        "JSON MUST name the document it checked"
    );
    assert!(
        matches!(parsed["detection"].as_str(), Some("available") | Some("unavailable")),
        "detection state MUST be explicit; got: {}",
        parsed["detection"]
    );
    assert!(parsed["rows"].is_array(), "rows MUST be an array");
    assert!(parsed["any_over"].is_boolean(), "any_over MUST be a boolean");

    // Registered projects are all present, each with a real verdict.
    let rows = parsed["rows"].as_array().expect("rows");
    for name in ["alpha", "beta"] {
        let row = rows
            .iter()
            .find(|row| row["project"] == name)
            .unwrap_or_else(|| panic!("row for {name} missing; got: {rows:?}"));
        let verdict = row["verdict"].as_str().expect("verdict");
        assert!(matches!(verdict, "OK" | "OVER" | "NO_DATA"), "unexpected verdict {verdict}");
        assert!(row["source"].is_string(), "row MUST carry its limit source");
    }
}

#[test]
fn fr005_check_text_says_nodata_rather_than_implying_a_pass() {
    let sandbox = Sandbox::new();
    let (stdout, _) = sandbox.ok(&["limits", "check"]);
    assert!(stdout.contains("VERDICT"), "check MUST print a verdict column; got: {stdout}");
    assert!(
        stdout.contains("NO_DATA") || stdout.contains("OK") || stdout.contains("OVER"),
        "check MUST use the verdict vocabulary; got: {stdout}"
    );
    if stdout.contains("NO_DATA") {
        assert!(stdout.contains("it is not a pass"), "check MUST explain NO_DATA; got: {stdout}");
    }
}

#[test]
fn fr005_fail_on_over_tracks_a_measured_overage() {
    let sandbox = Sandbox::new();
    let mut child = spawn_staged_agent(&sandbox.root);

    // The staged process is detected as family `claude`; registering that family
    // as a project with a zero process cap makes the overage measured, not rated.
    sandbox.ok(&["limits", "set", "claude", "--mem-mb", "0", "--max-procs", "0"]);
    let over = sandbox.run(&["limits", "check", "--fail-on-over"]);
    let over_stdout = String::from_utf8_lossy(&over.stdout).into_owned();
    assert_eq!(
        over.status.code(),
        Some(1),
        "--fail-on-over MUST exit 1 on a measured overage; stdout={over_stdout}"
    );
    assert!(over_stdout.contains("OVER"), "the row MUST say OVER; got: {over_stdout}");

    // Raising the cap above the measurement clears the exit code in a new process.
    sandbox.ok(&["limits", "set", "claude", "--mem-mb", "100000", "--max-procs", "500"]);
    let within = sandbox.run(&["limits", "check", "--fail-on-over"]);
    assert!(
        within.status.success(),
        "a raised cap MUST clear the failure; stdout={}",
        String::from_utf8_lossy(&within.stdout)
    );

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn fr005_optimize_reports_measurement_and_applies_once() {
    let sandbox = Sandbox::new();
    let mut child = spawn_staged_agent(&sandbox.root);

    let (stdout, _) = sandbox.ok(&["optimize"]);
    assert!(stdout.contains("detection:"), "optimize MUST state its measurement; got: {stdout}");
    assert!(
        stdout.contains("no suggestions") || stdout.contains("\n- ") || stdout.starts_with("- "),
        "optimize MUST emit either `no suggestions` or actionable lines; got: {stdout}"
    );
    assert!(!sandbox.limits.exists(), "a dry-run optimize MUST NOT write the limits document");

    let (applied, _) = sandbox.ok(&["optimize", "--apply"]);
    assert!(applied.contains("applied:"), "apply MUST report what it wrote; got: {applied}");
    let stored = sandbox.limits_json();
    assert!(
        stored["global"]["max_processes"].as_u64().unwrap_or(0) >= 1,
        "applied global limit MUST come from the measurement; got: {stored}"
    );

    // Second apply has nothing left to write and says so.
    let (again, _) = sandbox.ok(&["optimize", "--apply"]);
    assert!(again.contains("nothing to write"), "got: {again}");

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn fr005_set_rejects_ambiguous_or_empty_targets() {
    let sandbox = Sandbox::new();
    let conflicting = sandbox.run(&["limits", "set", "alpha", "--global", "--mem-mb", "64"]);
    assert!(!conflicting.status.success(), "project + --global MUST be rejected");
    let err = String::from_utf8_lossy(&conflicting.stderr).to_lowercase();
    assert!(err.contains("global") || err.contains("target"), "got: {err}");

    let empty = sandbox.run(&["limits", "set", "alpha"]);
    assert!(!empty.status.success(), "set without a value MUST be rejected");
    let err = String::from_utf8_lossy(&empty.stderr).to_lowercase();
    assert!(err.contains("mem-mb") || err.contains("max-procs"), "got: {err}");

    assert!(!sandbox.limits.exists(), "a rejected set MUST NOT create a document");
}

// `SHARECLI_PROC_ROOT` is honoured only under `cfg(target_os = "linux")`
// (see crates/sharecli-fleet/src/proc_scan.rs::proc_root). On macOS/BSDs the
// detector scans live processes through sysinfo, so the stubbed-empty-proc
// expectation does not hold; gate the case to Linux rather than weaken it.
#[cfg(target_os = "linux")]
#[test]
fn fr003_optimize_says_no_suggestions_when_detection_is_empty() {
    let sandbox = Sandbox::new();
    let empty_proc = sandbox.root.join("empty-proc");
    std::fs::create_dir_all(&empty_proc).expect("empty proc dir");

    let out = sandbox
        .cmd()
        .env("SHARECLI_PROC_ROOT", &empty_proc)
        .arg("optimize")
        .output()
        .expect("run optimize");
    assert!(
        out.status.success(),
        "optimize MUST exit 0; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(
        stdout.lines().any(|line| line == "no suggestions"),
        "optimize MUST print `no suggestions` when nothing is measurable; got: {stdout}"
    );
    assert!(
        stdout.contains("detection: UNAVAILABLE"),
        "optimize MUST name the reason it has nothing to say; got: {stdout}"
    );

    // The same stub proves the verdict vocabulary: unmeasured is NO_DATA, never OK.
    let check = sandbox
        .cmd()
        .env("SHARECLI_PROC_ROOT", &empty_proc)
        .args(["limits", "check"])
        .output()
        .expect("run limits check");
    let stdout = String::from_utf8_lossy(&check.stdout).into_owned();
    assert!(stdout.contains("NO_DATA"), "got: {stdout}");
    assert!(!stdout.contains(" OK "), "an unmeasured row MUST NOT read OK; got: {stdout}");
}

/// The document path used by the tests is the one the binary printed, so the
/// override really is honoured rather than silently ignored.
#[test]
fn fr005_set_reports_the_document_it_wrote() {
    let sandbox = Sandbox::new();
    let (stdout, _) = sandbox.ok(&["limits", "set", "alpha", "--mem-mb", "768"]);
    assert!(
        stdout.contains(&sandbox.limits.display().to_string()),
        "set MUST name the file it wrote; got: {stdout}"
    );
    assert!(Path::new(&sandbox.limits).exists(), "the reported file MUST exist");
}
