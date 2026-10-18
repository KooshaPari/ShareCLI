# Architecture Consistency — sharecli @ 3d6a41c9

**Date:** 2026-10-17 (UTC)
**Scope:** Verifying the workspace follows its own design; boundary discipline, dep graph, orphan code, FR traceability, dead code, vendored crates.
**Method:** Read-only. `rg`, `ls`, `grep`, `cat`, `cargo metadata --no-deps`, `cargo +nightly udeps --workspace --all-targets`. No source modification.
**Authoritative docs reviewed:** `Cargo.toml` (workspace manifest), `BOUNDARY.md`, `AGENTS.md`, `CLAUDE.md`, `FUNCTIONAL_REQUIREMENTS.md`, `WORK_DAG.md`, `lib/teamcomm/Cargo.toml`.

---

## Executive summary (top 3 architectural issues)

| # | Issue | Severity | Evidence (file:line) |
|---|-------|----------|----------------------|
| **1** | **Two crates — `sharecli-ipc` (line 34) and `sharecli-ffi` (line 13) — declare `sharecli = { path = "../../" }` in their `Cargo.toml`**, depending on the root package's library face. This is a "library depends on the binary" inversion (Cargo allows it because the root produces both `sharecli::lib` and the `sharecli` binary from the same `name = "sharecli"` package, but it muddies layering and forces root lib API to grow as soon as FFI/IPC need anything new). | **HIGH** | `crates/sharecli-ipc/Cargo.toml:34`, `crates/sharecli-ffi/Cargo.toml:13`; root Cargo.toml has no `[lib]` section — defaults to `src/lib.rs` which exposes 41 `pub mod` (src/lib.rs:10-53). `sharecli-ffi/src/lib.rs:73` shows the real cost: FFI has to reach into `sharecli::otel::apply_traceparent_spawn_env`. |
| **2** | **Tray crates `sharecli-tray-linux` and `sharecli-tray-windows` are parallel implementations with zero shared abstraction.** Both expose identical `pub mod { ipc, operator_display, poll }` shapes, and inspect shows near-identical `pub fn` names in each (`resolve_gate_decision_class`, `format_bytes_compact`, `format_gate_tray_line`, `format_operator_status_summary`, …). There is **no trait** in `sharecli-core` / `sharecli-ffi` that both implementations `impl` for. The two wire modules don't `use` a common snapshot type from core. | **HIGH** | `crates/sharecli-tray-linux/src/lib.rs:5-8` (`pub mod ipc; pub mod operator_display; pub mod poll;`) vs `crates/sharecli-tray-windows/src/lib.rs:14-18` (identical). Export shape comparison: tray-linux `src/operator_display.rs:38,47,57,…` vs tray-windows `src/operator_display.rs:30,38,47,…`. Neither file's `Cargo.toml` declares a `sharecli-*-common` dep (Cargo.toml:1-30 of each). |
| **3** | **Three sources of vendored / orphan crates sit inside the sharecli tree but are unreachable / mis-wired from the workspace build.** Specifically: (a) `crates/gateway-tools/Cargo.toml:15` declares `gateway = { path = "../gateway" }` but no `crates/gateway/` exists; (b) `crates/fuse-smoke-runner/` is a workspace member (line 235) yet no code references it; (c) `lib/teamcomm/` is a fully independent 6-crate workspace (`Cargo.toml:1-9`) with no link to sharecli at all. | **HIGH** | `crates/gateway-tools/Cargo.toml:15`; `Cargo.toml:235` declares `fuse-smoke-runner` as member but `rg -l 'fuse-smoke-runner\|fuse_smoke_runner'` returns no callers; `lib/teamcomm/Cargo.toml:1-9` is its own `[workspace]` and `grep -n teamcomm Cargo.toml` returns no matches. |

---

## 1. Workspace membership vs filesystem

| Cargo.toml claim (line) | Filesystem | OK / Drift |
|---|---|---|
| `crates/harness-native` (223) | present + Cargo.toml | OK |
| `crates/sharecli-sync` (224) | present + Cargo.toml | OK |
| `crates/sharecli-ipc` (225) | present | OK |
| `crates/sharecli-ffi` (226) | present | OK |
| `crates/sharecli-tray-linux` (227) | present | OK |
| `crates/sharecli-tray-windows` (228) | present | OK |
| `crates/sharecli-fleet` (229) | present | OK |
| `crates/sharecli-mesh` (230) | present | OK |
| `crates/spawn-core-sys` (231) | present | OK |
| `crates/sharecli-fuse` (232) | present | OK |
| `crates/sharecli-core` (233) | present | OK |
| `crates/sharecli-thermal-tui` (234) | present | OK |
| `crates/fuse-smoke-runner` (235) | present | OK (but orphan — see §6) |
| `crates/sharecli-session` (236) | present | OK |

**Members NOT declared:** `crates/spawn-core` (a Zig crate with `build.zig` + `spawn_core.zig` 14KB) lives on disk but is NOT in `[workspace] members`. It IS implicitly used: `crates/spawn-core-sys/build.rs:50` does `manifest_dir.join("..").join("spawn-core")` and invokes `zig build` against it. This works only because the path is a sibling, not because it is a workspace member. Add or remove one — but document the relationship. *(See also §6.)*

**Members NOT declared:** `crates/gateway-tools/` exists with `Cargo.toml` + `src/{main.rs,serve.rs}`; NOT in `[workspace] members`. Add it, exclude it, or delete it.

---

## 2. Directed workspace dependency graph (members only)

Edges point "A depends on B" for path-style deps between members. From each member's `Cargo.toml`:

| Crate | Depends on (workspace path deps) |
|-------|----------------------------------|
| `harness-native` | `sharecli-core`, `sharecli-ipc` (`crates/harness-native/Cargo.toml:26-27`) |
| `sharecli-sync` | — (only `loom`) |
| `sharecli-ipc` | `sharecli-fleet`, `sharecli-session`, **`sharecli` (root, line 34)** |
| `sharecli-ffi` | **`sharecli` (root, line 13)** |
| `sharecli-tray-linux` | — (only `ksni`); no sharecli deps |
| `sharecli-tray-windows` | — (only `tray-icon`); no sharecli deps |
| `sharecli-fleet` | — (only `async-nats`, `sysinfo`) |
| `sharecli-mesh` | — (only `anyhow`, `serde`, `uuid`) |
| `spawn-core-sys` | — (only `anyhow`); consumes Zig from sibling `crates/spawn-core` at build time (`build.rs:50`) |
| `sharecli-fuse` | — (only `fuser`, `libc`, `xattr`, `winfsp`); no sharecli deps |
| `sharecli-core` | `sharecli-ipc`, `sharecli-fleet`, `sharecli-fuse` (`crates/sharecli-core/Cargo.toml:9-11`) |
| `sharecli-thermal-tui` | `sharecli-fleet`, `sharecli-fuse`, `sharecli-mesh` (`crates/sharecli-thermal-tui/Cargo.toml:9-11`) |
| `fuse-smoke-runner` | — (only `anyhow`, `clap`, `serde`, `thiserror`) |
| `sharecli-session` | — (only `rusqlite`, `anyhow`, `serde`, `chrono`) |
| `sharecli` (root, lib+bin) | `spawn-core-sys`, `sharecli-fleet`, `sharecli-fuse`, `sharecli-sync`, `sharecli-mesh`, `sharecli-session`, `sharecli-thermal-tui`, plus dev-deps `sharecli-core`, `sharecli-ipc`, `sharecli-tray-linux`, `sharecli-tray-windows`, `sharecli-fuse` |

**Cycles in cargo metadata:** none detected. The `sharecli_ipc` -> `sharecli` reverse is a Cargo-allowed back-edge to the root's LIBRARY face (root is both `[[bin]]` and `[lib]` via `src/lib.rs`).

---

## 3. Boundary violations

| Violation | Evidence | Severity |
|-----------|----------|----------|
| Library depends on the binary's library face | `crates/sharecli-ipc/Cargo.toml:34`, `crates/sharecli-ffi/Cargo.toml:13` | HIGH — cargo allows it but layering is upside down. FFI and IPC are supposed to be a stable C-ABI / wire surface; pinning them to the root lib means any root refactor ripples into their public API. |
| `sharecli-ffi` cannot access `sharecli-core`'s types directly | `crates/sharecli-ffi/src/lib.rs:73` calls `sharecli::otel::apply_traceparent_spawn_env` (root lib path). FFI's only sharecli-* dep is the root, not core. | HIGH — duplicate boundary. |
| `sharecli-core` (named as the foundation) imports infra crates `sharecli-ipc`, `sharecli-fleet`, `sharecli-fuse` (`crates/sharecli-core/Cargo.toml:9-11`). The crate's own doc-comment says it is the "central entry point for spawning managed processes" and "wires together the coalescing cache from sharecli-ipc…". So the name `core` is misleading; it is the engine tier, not a dependency-free foundation. | `crates/sharecli-core/Cargo.toml:9-11`, `crates/sharecli-core/src/lib.rs:1-12` | MEDIUM — naming inconsistency. |
| Tray crates have no shared abstraction with `sharecli-core`. They each talk directly to `sharecli-ipc` over a Unix socket (linux) or Win32 pipe (windows), but neither `pub use`s a common interface from core. | `crates/sharecli-tray-linux/src/lib.rs` (only `pub mod ipc/operator_display/poll`), `crates/sharecli-tray-windows/src/lib.rs` (same shape). `sharecli-tray-linux/Cargo.toml` no sharecli-* deps; `sharecli-tray-windows/Cargo.toml` no sharecli-* deps. | HIGH — see top-3 issue #2. |
| `deny.toml` does not enforce a `[graph]` layer rule against the workspace. | `deny.toml` declares `[bans]` only (file present, no `[graph]` section). | MEDIUM |
| `AGENTS.md` declares an external collaborator boundary (thegent-sharecli fork is archived) but the repo STILL has BOTH `BOUNDARY.md` AND a `Cargo.toml [workspace]` whose layering is undefined in the canonical docs. Only `lib-sprawl-plan.md` (a sibling-audit's find) describes the intended 5-plane model. | `AGENTS.md:60-77`, `BOUNDARY.md:1-15`, root has no `ARCHITECTURE.md`. | MEDIUM — orchestrator should author `ARCHITECTURE.md` and reconcile with ADR-0007 + scorecard. |

---

## 4. Duplicate dep listings (cargo semantics — not necessarily wrong)

| Crate | Cargo.toml line(s) | Notes |
|-------|-------------------|-------|
| `sharecli-thermal-tui` | main deps 71 + dev-deps 127 | udeps reported `sharecli-thermal-tui` not in "unused" list. This is a maintainability smell, not a bug. |
| `sharecli-fuse` | main deps 64 + dev-deps 133 | same. |

---

## 5. Pub-use surface drift (root `src/lib.rs`)

The root lib has 41 `pub mod` entries (`src/lib.rs:10-53`). Re-exports:
- `pub use runtime::{ManagedProcess, ProcessFilter, ProcessInfo, ProcessPool, ProjectLimits, ProjectResources, SharedRuntime};` (line 56)
- `pub use sharecli_sync::PoolIndex;` inside an inline module `pool_index` (line 32-34)
- 17 `pub use util::…` lines (61-78) for the `lib-sprawl-plan.md` Phase 1 compat shims

The only direct subcrate re-export is `sharecli_sync::PoolIndex`. The `sharecli-ipc`, `sharecli-ffi`, `sharecli-fleet`, `sharecli-fuse`, `sharecli-mesh`, `sharecli-session`, `sharecli-thermal-tui`, `spawn-core-sys`, `harness-native` types are NOT surfaced as `pub use` re-exports from the root lib. They are accessed only via `sharecli_*::` (snake_case crate name) at the consumer (e.g. `src/main.rs:15: use sharecli_thermal_tui as thermal_tui;`). This is fine, but the doc-comment claim "sharecli - Thin CLI wrapper around local process runtime" (`src/lib.rs:1`) undersells the 41 modules.

---

## 6. Vendored / orphan crates

| Path | In workspace? | Referenced from root Cargo.toml? | Verdict |
|------|--------------|----------------------------------|---------|
| `crates/spawn-core/` (Zig crate) | **NO** | implicit — `spawn-core-sys/build.rs:50` shells out to `zig build` against it | Live but invisible. Add as `members = ["crates/spawn-core"]` AND introduce a `[workspace.dependencies.zig-spawn-core]` path, OR add a note in Cargo.toml that the Zig crate is built on-demand by spawn-core-sys. |
| `crates/gateway-tools/` (Rust crate with main.rs + serve.rs) | **NO** | NO | **Broken.** `Cargo.toml:15` declares `gateway = { path = "../gateway" }` but `crates/gateway/` does not exist (`ls crates/` shows only the 14 listed). |
| `crates/fuse-smoke-runner/` | YES (line 235) | NO | Orphan workspace member. Nothing references `fuse-smoke-runner` or its `fuse-smoke` binary. Either invoke it from CI / `Containerfile.fuse-smoke`, or remove from `[workspace] members`. |
| `lib/teamcomm/` | independent workspace (own `[workspace]` line 1-9) | NO | 6-crate sub-workspace (`teamcomm-protocol`, `teamcomm-client`, `teamcomm-daemon`, `teamcomm-cli`, `teamcomm-mcp`, `teamcomm-smoke`) builds separately, has its own `Cargo.lock`. sharecli Cargo.toml does NOT mention it. Either fold into sharecli workspace or split into a sibling repo. Currently it sits in `lib/` (not `crates/`) with its own AGENTS.md / SPEC.md / WORKLOG.md. |

---

## 7. Tray parity — `sharecli-tray-linux` vs `sharecli-tray-windows`

| Aspect | sharecli-tray-linux | sharecli-tray-windows |
|---|---|---|
| Binary gating | `#[cfg(target_os = "linux")]` for `fn main` (`src/main.rs:11`); stub exits 1 elsewhere (lines 16-21) | `#[cfg(target_os = "windows")]` + `#[cfg(not(target_os = "windows"))]` for `creation_flags_win` trait shim (`src/main.rs:25-43`); main is always run, no stub |
| Library cross-platform compile | library is `pub mod ipc; pub mod operator_display; pub mod poll;` (lib.rs:5-8), each module cross-platform | identical (`lib.rs:14-17`) |
| Tray backend | `ksni::blocking::TrayMethods` (main.rs:33) | `tray_icon::{Icon, TrayIcon, TrayIconBuilder}` (main.rs:21) |
| IPC client | `sharecli_tray_linux::ipc` (main.rs:36) talking Unix socket | `sharecli_tray_windows::ipc` talking Win32 pipe |
| Shared interface | NONE found | NONE found |
| Shared test imports | `tests/fr007_tray_*` | `tests/fr007_tray_*` (parity suite) |

**Verdict:** Parallel implementations behind no shared trait. Drift risk: every new tray feature (`add_status_dot_color`, `add_ipc_action`, …) must be added to both crates identically. The sibling-audit `lib-sprawl-plan.md` apparently proposes a `sharecli-tray-core` common trait; it does not exist.

---

## 8. FR reference density (code-only; tests/prs/docs excluded)

| FR | Hits in `.rs` / `.toml` |
|----|-------------------------|
| FR-001 | 30 |
| FR-003 | 241 |
| FR-007 | **529** — most heavily referenced FR, centred on operator/IPC/monitoring/tray wire |
| FR-008 | 100 — coalesce/speculation/queue priority/harness-native strategies |

Source: `rg 'FR-NNN' --type rust -t toml --no-heading 2>/dev/null | wc -l` per FR.

FR-007 hotspots:
- `src/commands/mod.rs:17`, `src/commands/proc/mod.rs:12`, `src/commands/render_csv.rs:6`, `src/commands/report.rs:6`, `src/commands/serve.rs:4`
- `src/monitoring.rs:3`
- `Cargo.toml:1`
- `crates/sharecli-core/src/lib.rs:2`
- `crates/sharecli-thermal-tui/src/lib.rs:8`
- `tests/fr007_*`: 30+ distinct test files

FR-008 hotspots:
- `crates/sharecli-core/src/lib.rs:9`, `crates/sharecli-core/src/speculation.rs:1`
- `crates/harness-native/src/strategies/*`: 17 files (`hypervisor_lane.rs`, `coalesce.rs`, `debounce.rs`, `process.rs`, `retry.rs`, `circuit_breaker.rs`, `queue.rs`, `load_balance.rs`, `jobserver.rs`, `proactive_warm.rs`, `speculative.rs`, …)
- `tests/fr008_*`: `e2e_hypervisor_nocache.rs:5`, `fr008_coalesce_mesh.rs:18`, `fr008_coalesce_status.rs:2`, `fr008_queue_priority_operator.rs:4`

---

## 9. Dead / orphan code (`cargo +nightly udeps --workspace --all-targets`)

udeps reported the following workspace dep tree as having no in-tree callers (false-positives may exist for doc-test–only usage):

| Crate | Dep declared | Dep used? | Verdict |
|-------|-------------|-----------|---------|
| `sharecli` (root) | `phenotype-crypto` | NOT used anywhere in `src/` or `crates/` | Likely dead. Either wire into auth/observability paths or drop. |
| `sharecli` (root) | `phenotype-health` | NOT used | Likely dead. |
| `sharecli` (root) | `phenotype-observability` | NOT used | Likely dead. |
| `sharecli` (root) | `phenotype-state-machine` | NOT used | Likely dead. |
| `sharecli` (root) | `tracing-appender` | NOT used | Likely dead — root uses `tracing-subscriber` only. |
| `sharecli-fuse` | `serde` | NOT used in `.rs` (rg returns no matches in `crates/sharecli-fuse/src/`) | Dead. `serde` line 29 of Cargo.toml should be removed. |
| `harness-native` | `config` | NOT used | Dead. |
| `harness-native` | `directories` | NOT used | Dead. |
| `harness-native` | `itertools` | NOT used | Dead. |
| `fuse-smoke-runner` | `thiserror` | NOT used | Dead (the crate is in workspace but its `thiserror` dep is unused). |
| `sharecli-tray-linux` | `tracing` | NOT used | Dead. |
| `sharecli-tray-linux` | `tracing-subscriber` | NOT used | Dead. |
| `sharecli-sync` | `loom` (main + dev) | USED (`use loom::sync::RwLock;`); udeps likely false-positive | OK — keep. |

Source: `/tmp/udeps-out.log` blocks 1-30.

`dead_code` lint also flagged:
- `crates/sharecli-fuse/src/bin/mfmount-probe.rs:17` — variants `Success`, `UnsupportedOs`, `HelperToolsInstallationFailed`, `FileSystemExtensionNotFound`, `FileSystemExtensionRequiresApproval`, `UnexpectedFailure` are never constructed. The enum's discriminants drive a process exit code; if the binary is unused (orphan member), the variants stay dead until something constructs them.

---

## 10. Public API surface (`pub use` re-export audit per crate)

| Crate | `lib.rs` pub surface | Cross-crate callers |
|-------|--------------------|---------------------|
| `sharecli-core` | `pub use sharecli_fleet::{agent_contention, count_host_agents, gate_status_snapshot, …}` (lib.rs:67-83), `pub use sharecli_ipc::{command_key, CacheKeyMode, …}` (lib.rs:86-). | root binary `src/lib.rs` uses sharecli_core transitively via direct paths. |
| `sharecli-ipc` | `pub use sharecli_fleet::{…}` (lib.rs:58-) for coalesce meters; `pub use cache_key`, `pub use nocache`, `pub use queue` (lines 41-51). | core re-exports; root binary reaches the binary. |
| `sharecli-fleet` | All `pub use` to its own submodules (lib.rs:17-90). No outbound re-exports. | ipc re-exports; root uses directly. |
| `sharecli-fuse` | All `pub use` to its own submodules (lib.rs:34-…); some guarded by `#[cfg(any(target_os = "linux", target_os = "macos", windows))]`. | root binary & thermal-tui. |
| `sharecli-mesh` | `pub use` to its own submodules. | root binary & thermal-tui. |
| `sharecli-session` | Flat `pub mod`; no re-exports. | root binary. |
| `sharecli-sync` | `pub mod` only; `loom` cfg-guarded. | root lib via `pub use sharecli_sync::PoolIndex` (src/lib.rs:33). |
| `sharecli-thermal-tui` | 30+ `pub fn` (`is_quit_key`, `handle_key`, `apply_key_action`, `level_label`, `level_color`, `slot_ratio`, `slot_color`, `gate_panel_lines`, `pool_panel_lines`, `status_panel_lines`, `resource_watch_lines`, `host_agent_lines`) all cross-platform. No re-exports of sharecli-*. | root `src/main.rs:15: use sharecli_thermal_tui as thermal_tui;`. |
| `sharecli-tray-linux` | `pub mod ipc; pub mod operator_display; pub mod poll;` (lib.rs:5-7). The two other modules' pub fns are listed in §7. | integration tests only (`tests/fr007_tray_*`, `tests/fr007_ipc_pool_status_tray_wire.rs`). |
| `sharecli-tray-windows` | identical shape (lib.rs:14-17). | integration tests only. |
| `harness-native` | Per-binary features `harness-cache-key`, `harness-is-agent`, `helios-shield` (Cargo.toml:9-19); core lib `harness_native` (line 22). Strategy modules per file. | sharecli-ipc pulls `harness_native` (Cargo.lock). |
| `spawn-core-sys` | `links = "spawn_core"` static lib FFI (lib.rs / Cargo.toml:7-8). Direct callers: root binary uses `spawn_core_sys::*` (via Zig-generated symbols). | root binary. |
| `gateway-tools` | (broken) | nothing. |
| `fuse-smoke-runner` | `publish = false`; binary `fuse-smoke` (`main.rs`). | nothing — see §6. |

---

## 11. Boundary-violation summary one-liner table

| Boundary | Defined by | Observed | Drift |
|----------|-----------|---------|-------|
| Root lib depends on members | implied | core, ipc, fleet, fuse, mesh, sync, session, thermal-tui | nominal |
| `sharecli-core` is dependency-free | NONE IN CANONICAL DOCS (no ARCHITECTURE.md); expected by naming | core depends on ipc + fleet + fuse | the name lies |
| FFI depends only on core / wire types | implied | FFI depends on the root lib binary (`Cargo.toml:13`) | YES |
| IPC is leaf-ish (does not depend on tray / thermal / ffi) | implied | ipc depends on root lib binary (`Cargo.toml:34`) | partial |
| Trays share a trait | implied | no shared trait; parallel implementations | YES |
| Workspace excludes unbuilt crates | implied | gateway-tools & spawn-core live in `crates/` but are not members; lib/teamcomm is a separate workspace but lives in `lib/` | YES |

---

## 12. Missing or absent primary docs

| Document | Status |
|----------|--------|
| `ARCHITECTURE.md` (root) | DOES NOT EXIST (`ls /Users/kooshapari/CodeProjects/Phenotype/repos/sharecli/ARCHITECTURE.md` → not found; `find … -maxdepth 3 -name 'ARCHITECTURE*'` returns only `lib/teamcomm/ARCHITECTURE.md`). Task #2 of the audit brief therefore cannot be satisfied as stated. |
| `CLAUDE.md` | 16 lines, no architectural content beyond "stack = Rust, build = Cargo" and "Phenotype Org Rules". Not informative for the consistency check. |
| `BOUNDARY.md` | 15 lines, defines process orchestration ownership; references `thegent` (external) and `tehgent`. Useful but orthogonal to crate-layer architecture. |
| `AGENTS.md` | detailed, includes claim-lock protocol and worktree discipline. Documents the 14-lane shared-path ownership table (lines 90-101) — but uses lane names (`agent-c03`, `agent-c05`) that don't map 1:1 to the current orchestrator's lane labels. |
| `lib-sprawl-plan.md` | referenced in src/lib.rs:53 ("// C00 L1 Phase 1 — `docs/ops/lib-sprawl-plan.md`"); describes Phase 1–4 tier extraction (Phase 4 promises `sharecli-runtime`, `sharecli-serve`, `sharecli-config`). **None of those tiers exist on disk.** This is the silent mismatch at the heart of the contradiction. |

---

## 13. UNKNOWN / unproven items

- Whether `crates/spawn-core/zig-out/` ever succeeds without zig installed locally (the Rust stub on Windows masks this; `build.rs:83` returns an error if `zig` is missing, so a Linux dev without zig will fail).
- Whether `gateway-tools`'s missing `crates/gateway/` dep was a typo for an existing crate (none of the 14 workspace members looks like `gateway` — closest is `sharecli-mesh`/`sharecli-thermal-tui`). UNVERIFIED; recommend renaming or removal.
- The presence of a stale build cache lock on first `cargo check` (a `[lib]` block duplicate-key error from a transitive git dep `agileplus-cache/Cargo.toml:15` was observed in `cargo metadata --format-version 1`; this only blocks dep resolution, not `--no-deps`).

---

## 14. Recommendations (do not act — read-only audit)

1. Decide whether `sharecli-ffi` and `sharecli-ipc` should depend on the root library or on `sharecli-core` directly. If direct, the root lib's pub surface is no longer in FFI's path; the root lib can be reduced to a CLI-only thin wrapper.
2. Define a `tray::Snapshot` trait (or `tray_core` subcrate) that both `sharecli-tray-linux` and `sharecli-tray-windows` implement. Make `crates/sharecli-tray-{linux,windows}/src/operator_display.rs` consume the trait so the formatting parity is enforced, not copy-pasted.
3. Delete `crates/gateway-tools/`, `crates/fuse-smoke-runner/`, and `lib/teamcomm/` from the sharecli tree if they are not part of the sharecli workspace build. If kept, fold `gateway-tools` and `fuse-smoke-runner` into `[workspace] members` and link `lib/teamcomm` via a clear `[workspace.members]` extension or split it out.
4. Drop the 11 unused dependencies udeps flagged (Cargo.toml root + per-crate).
5. Write `ARCHITECTURE.md` (root) that explicitly resolves the contradiction with `lib-sprawl-plan.md`. Either extract `sharecli-runtime`, `sharecli-serve`, `sharecli-config` (the lib-sprawl plan) OR mark the plan as superseded.
6. Reconcile `sharecli-core`'s name with its actual role: rename to `sharecli-hypervisor` (matches its doc-comment "central entry point for spawning") and introduce a thin `sharecli-core` for genuinely leaf types if needed.
7. Remove the duplicate dev-dep listings of `sharecli-thermal-tui` (Cargo.toml:71, 127) and `sharecli-fuse` (Cargo.toml:64, 133).

---

## Appendix: command log (reproducibility)

```bash
# Recon
ls /Users/kooshapari/CodeProjects/Phenotype/repos/sharecli/
cat Cargo.toml
cat BOUNDARY.md AGENTS.md CLAUDE.md
cargo metadata --format-version 1 --no-deps > /tmp/sharecli-meta-no-deps.json

# Per-crate deps
for c in crates/harness-native crates/sharecli-sync crates/sharecli-ipc crates/sharecli-ffi \
         crates/sharecli-tray-linux crates/sharecli-tray-windows crates/sharecli-fleet \
         crates/sharecli-mesh crates/spawn-core-sys crates/sharecli-fuse crates/sharecli-core \
         crates/sharecli-thermal-tui crates/fuse-smoke-runner crates/sharecli-session; do
  test -f "$c/Cargo.toml" || echo "MISSING $c"
  grep -nE '\[dependencies\]|^\[dev-dependencies\]|^\[build-dependencies\]|^[a-z_-]+ ?=' "$c/Cargo.toml"
done

# FR density
rg 'FR-NNN' --type rust -t toml --no-heading 2>/dev/null | wc -l

# Dead deps
cargo +nightly udeps --workspace --all-targets 2>&1 > /tmp/udeps-out.log

# Vendored crates
grep -n 'teamcomm\|gateway-tools\|fuse-smoke-runner\|spawn-core' Cargo.toml
ls lib/teamcomm/crates/

# Tray parity
grep -nE '^pub mod|^pub fn|^pub use' crates/sharecli-tray-linux/src/*.rs crates/sharecli-tray-windows/src/*.rs
```
