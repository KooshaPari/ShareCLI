# ShareCLI inventory and provenance

Observed 2026-10-14 Pacific. Read-only local repository: `/Users/kooshapari/CodeProjects/Phenotype/repos/sharecli`.

| Item | Observed state |
|---|---|
| HEAD | `fec57c5ce45935203c63e5b4c487fddf642ff18c` |
| Origin | `git@github.com:KooshaPari/ShareCLI.git` |
| Additional remote | Local no-mistakes preservation remote; not changed |
| Root package | `sharecli` 0.8.0, Rust edition 2021, rust-version 1.89 |
| Distribution metadata | aarch64-apple-darwin, x86_64-unknown-linux-gnu |
| Dirty state | 3,449 preexisting status entries at initial check, predominantly tracked `lib/teamcomm/target` artifact deletions; untouched |
| Scope | Local source + documentation + test inventory; no runtime or performance qualification |
| Placement | All on device; bounded text/file inspection, no heavy compute |
| GitHub state | PRs, issues and Dependabot UNKNOWN: CLI network connection failed |

## Components present

Root `src/`: CLI commands, API, process/runtime, spawn policy, configuration, dashboard HTML/JS/assets, HTTP serve/auth/rate limits, telemetry, audit and scheduling support.

Crates with Cargo manifests: `fuse-smoke-runner`, `gateway-tools`, `harness-native`, `sharecli-core`, `sharecli-ffi`, `sharecli-fleet`, `sharecli-fuse`, `sharecli-ipc`, `sharecli-mesh`, `sharecli-session`, `sharecli-sync`, `sharecli-thermal-tui`, `sharecli-tray-linux`, `sharecli-tray-windows`, `spawn-core-sys`. Zig `crates/spawn-core/src/spawn_core.zig` is a separate native core. Desktop and Windows directories also exist.

## Documents available in repo

`AGENTS.md`, `BOUNDARY.md`, `PRD.md`, `SPEC.md`, `FUNCTIONAL_REQUIREMENTS.md`, `PLAN.md`, `WORK_DAG.md`, `TEST_COVERAGE_MATRIX.md`, `THREAT_MODEL.md`, `COMPREHENSIVE_AUDIT_SCORECARD.md`, `docs/specs/`, `docs/ops/`, `audit/` and historical wave documents. These are evidence locations, not current acceptance certificates. Applicable repo AGENTS was read. Atlas ShareCLI/Fabric dossier lookup under the provided downloads path produced no match; no dossier gate is claimed satisfied.

## Representative verification files (present, not run)

- `tests/fr006_agent_detection.rs`, `fr006_agent_pid_watch.rs`, `fr006_ps_agent_column.rs`, `fr006_agent_rss_gate.rs`, `fr006_thermal_tui_agent_tree.rs`.
- `tests/fr007_dashboard_ws_operator_envelope.rs`, `c10_dashboard_gfx_pack.rs`.
- `tests/fr007_tray_windows_harness.rs`, `fr007_tray_windows_kill.rs`, `fr007_tray_windows_poll_interval.rs`, `fr007_tray_windows_monitoring_report_consume.rs`.
- `tests/spawn_audit.rs`, `agent_call_policy.rs`, `fr011_agent_thermal_gate.rs`.
- `crates/harness-native/tests/native_harness_contract.rs` and in-module unit tests.

Tests with graphics-oriented names were only listed; none were executed or inspected using screen capture. No screenshot or recording tools were used.

## Recent history at snapshot

`fec57c5c` rustls advisory upgrade; `a25441f1` CI dependency-policy adjustment; `0358ee99` actor-neutral nested transaction admission guards (#892); `86e08b55` and `8e11e00b` Mergify config fixes; `4f01d019` RAII waiter/rank aging; `4600ceea` lock acquisition deadline; `475b7a5f` failed-run cache exclusion; `acd268bd` UDS mode/peer checks; `704a85a3` dependency auto-merge policy.

These titles show active hardening work; they do not prove successful CI or installed-runtime adoption.

## Tool limitations

The PATH `rg` shim returned no search matches even for known present files; after confirming this, standard grep/find were used. `gh` was absent from PATH but available at `/opt/homebrew/bin/gh`; read-only remote queries failed to connect. No workaround changed configuration, credentials or network policy.
