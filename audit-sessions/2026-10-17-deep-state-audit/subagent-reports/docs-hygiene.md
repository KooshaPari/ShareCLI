# sharecli docs-hygiene audit

**Audit date:** 2026-10-17
**Repo root:** `/Users/kooshapari/CodeProjects/Phenotype/repos/sharecli`
**HEAD:** `3d6a41c972c2c86bf1374f224019c365ce3aa646`
  (commit message: "ci: combined codesign unblock + actionlint + rustfmt (FR-001, FR-003) (#918)")
**Audit scope:** Documentation hygiene only. Source files were not modified.
**Marker legend:** [PASS] [FAIL] [WARN] [INFO] [UNKNOWN]. No emoji.

---

## Executive summary

| Check                                       | Verdict | Notes |
|---------------------------------------------|---------|-------|
| 1. Public-API rustdoc coverage              | [WARN]  | 16 crates audited. Aggregate coverage ~59%. 3 crates below 30%. |
| 2. Per-crate README presence                | [FAIL]  | 14 of 16 crates lack `crates/<name>/README.md`. |
| 3. Root README accuracy / broken HTTP links | [FAIL]  | 2 production GitHub URLs in README return 404 (phenotype-tooling, portage-temp). |
| 4. CHANGELOG presence / last-entry date     | [PASS]  | Present, 1134 lines; last dated entry `[1.0.0] - 2026-08-31`. |
| 5. Broken anchors in markdown               | [INFO]  | Spot-checked; no broken anchors found in audited files. |
| 6. TODO/FIXME in markdown                   | [WARN]  | 9 markers across 8 markdown files, including `docs/specs/TRACEABILITY.md`. |
| 7. Missing architectural diagrams           | [FAIL]  | No `docs/architecture/` directory. Only 6 diagram files repo-wide, all branding/UI, none architecture. |
| 8. Spec drift (FR.md vs code)               | [WARN]  | `docs/specs/FR.md` line ranges stale for FR-001 and FR-005; FR-006/007/008 paths still valid. |
| 9. LICENSE present                          | [PASS]  | `LICENSE` (MIT, 2026 Koosha Pari). |
| 10. CONTRIBUTING / CODE_OF_CONDUCT / SECURITY| [PASS]  | All three present at root. |

---

## 1. Public-API rustdoc coverage

**Method:** Enumerated every `pub (extern|fn|struct|enum|trait|type|const|static|use)` line per crate via `rg`. A line was counted as **documented** iff the immediately-preceding non-blank line started with `///` or `//!`. Re-exports (`pub use …`) were counted; their coverage depends on the source item.

**Coverage table** (sorted by coverage ascending — worst first):

| Crate                | pub_total | covered | pct  | Verdict |
|----------------------|-----------|---------|------|---------|
| sharecli-ffi         |         6 |       0 |  0.0 | [FAIL]   |
| sharecli-session     |       110 |      26 | 23.6 | [FAIL]   |
| spawn-core-sys       |        30 |      11 | 36.7 | [WARN]   |
| sharecli-sync        |        10 |       4 | 40.0 | [WARN]   |
| sharecli-tray-windows|        36 |      18 | 50.0 | [WARN]   |
| sharecli-tray-linux  |        44 |      23 | 52.3 | [WARN]   |
| fuse-smoke-runner    |        13 |       7 | 53.8 | [WARN]   |
| sharecli-core        |        73 |      40 | 54.8 | [WARN]   |
| sharecli-ipc         |        82 |      45 | 54.9 | [WARN]   |
| sharecli-mesh        |        40 |      28 | 70.0 | [WARN]   |
| sharecli-fleet       |       115 |      81 | 70.4 | [WARN]   |
| sharecli-fuse        |       205 |     146 | 71.2 | [WARN]   |
| harness-native       |        26 |      19 | 73.1 | [WARN]   |
| sharecli-thermal-tui |        52 |      50 | 96.2 | [PASS]   |
| gateway-tools        |         0 |       0 |  N/A | [INFO]   |
| spawn-core (Zig)     |         2 |     N/A |  N/A | [INFO]   |
| **TOTAL (Rust)**     |     **838**|    **498**| **59.4** | [WARN] |

**Worst offenders:**

- `crates/sharecli-ffi/src/lib.rs:29,62,69,78,118,170` — all 6 `pub extern "C" fn` items have **zero** `///` doc comments. This is a C-ABI surface consumed by Swift (`desktop/ShareCLITray/sharecli_ffi.h`) and C# (`windows/ShareCLITray/Interop.cs`) — the absence of `///` docs means callers have to read implementation to know argument contracts.
- `crates/sharecli-session/src/adapter.rs:10,25,42,68` — `SurfaceAdapter`, `SurfaceIo` traits and `GhosttySurfaceAdapter`/`ZmxSurfaceAdapter` structs undocumented.
- `crates/sharecli-session/src/state.rs:16,25,56` — `SidecarRecord`, `append_record`, `SidecarStateProvider` undocumented.
- `crates/sharecli-session/src/rpc.rs:31,38,43,80,85` — `Request`, `Response`, `SurfaceControl`, `RpcError`, `SurfaceResponse` undocumented (the public RPC surface).
- `crates/sharecli-session/src/layout.rs:13,20,29,38` — `LayoutAxis`, `LayoutNode`, `LayoutSnapshot`, `LayoutRestoreItem` undocumented.
- `crates/spawn-core-sys/src/windows_stub.rs:15,23,38,44,49,59,68,77,94,106` — entire `ZigSemaphore` API and `zig_spawn`/`zig_waitpid` undocumented. This is the Windows side of the spawn bridge to Zig.
- `crates/sharecli-sync/src/lib.rs:14,53` — `PoolIndex`, `RelaxedCounter` structs undocumented.

**Best:** `sharecli-thermal-tui` at 96.2%. It has 1 source file with disciplined docs.

**`gateway-tools`** has zero top-level `pub` items — it is a binary whose surface is the clap `#[derive(Parser)]` enum (which itself uses doc comments per `/// List managed processes` etc.). Internal helper functions are private. Marked [INFO].

**`spawn-core`** is a Zig project (no Rust source); its 2 pub items live in `crates/spawn-core/src/spawn_core.zig`. Not in scope for `///` rustdoc. Marked [INFO].

---

## 2. Per-crate README presence

Crate roots under `crates/`. R = `crates/<name>/README.md`, C = `Cargo.toml`, L = `src/lib.rs`, M = `src/main.rs`.

| Crate                | README | Cargo.toml | lib.rs | main.rs |
|----------------------|:------:|:----------:|:------:|:-------:|
| sharecli-core        |   -    |     C      |   L    |    -    |
| sharecli-ipc         |   -    |     C      |   L    |    M    |
| sharecli-fuse        |   -    |     C      |   L    |    -    |
| sharecli-fleet       |   -    |     C      |   L    |    -    |
| sharecli-mesh        |   -    |     C      |   L    |    -    |
| sharecli-session     |   -    |     C      |   L    |    -    |
| sharecli-sync        |   -    |     C      |   L    |    -    |
| sharecli-thermal-tui |   -    |     C      |   L    |    -    |
| sharecli-tray-linux  |   R    |     C      |   L    |    M    |
| sharecli-tray-windows|   -    |     C      |   L    |    M    |
| sharecli-ffi         |   -    |     C      |   L    |    -    |
| spawn-core           |   -    |     -      |   -    |    -    |
| spawn-core-sys       |   R    |     C      |   L    |    -    |
| harness-native       |   -    |     C      |   L    |    -    |
| gateway-tools        |   -    |     C      |   -    |    M    |
| fuse-smoke-runner    |   -    |     C      |   L    |    M    |

**Missing per-crate README (14 crates):** sharecli-core, sharecli-ipc, sharecli-fuse, sharecli-fleet, sharecli-mesh, sharecli-session, sharecli-sync, sharecli-thermal-tui, sharecli-tray-windows, sharecli-ffi, spawn-core (Zig crate, doesn't apply), harness-native, gateway-tools, fuse-smoke-runner.

Verdict: [FAIL] — only `sharecli-tray-linux` and `spawn-core-sys` have crate-level READMEs. The root `README.md` is the only comprehensive description.

**Note on `spawn-core`:** no Cargo.toml because it is a Zig crate (`crates/spawn-core/build.zig`); Rust consumes it via `spawn-core-sys`. README expectation does not apply. Marked [INFO] separately.

**Note on `gateway-tools`:** Has Cargo.toml with `[[bin]] name = "gateway-tools"` but is **NOT listed in the root `[workspace] members = [...]`** (workspace lists 14 crates; `gateway-tools` and `spawn-core` are the two non-members among the 16 in the audit list). It depends on `gateway = { path = "../gateway" }` — that `gateway` crate is **not present in this repo**. The crate is either an orphan or expects a sibling repo. Verdict: [WARN] — orphan / not-built workspace-adjacent crate.

---

## 3. Root README accuracy and broken HTTP links

`README.md` exists at root, 345 lines.

**Staleness flags:**

- README:62 mentions `Formula/sharecli.rb` "version tracks `Cargo.toml` / latest tag `v0.3.0`" — but CHANGELOG shows current release is `v1.0.0 - 2026-08-31` (line 33 of `CHANGELOG.md`). The README still references the old v0.3.0 formula. [FAIL] stale version reference.
- README:41 references `phenotype-tooling/crates/benchora` at `https://github.com/KooshaPari/phenotype-tooling` — that URL returns **HTTP 404** (see link check below). [FAIL] dead link.
- README:42 references portage fork at `https://github.com/KooshaPari/portage-temp` — returns **HTTP 404**. [FAIL] dead link.

**HTTP link audit** (19 unique URLs in README, sampled all):

| URL | Status | Verdict |
|-----|--------|---------|
| http://127.0.0.1:8080/healthz | 200 | [PASS] (localhost — assumed running) |
| http://127.0.0.1:9000/ | TIMEOUT | [UNKNOWN] (localhost not running) |
| http://127.0.0.1:9000/` | TIMEOUT | [UNKNOWN] |
| https://crates.io/crates/sharecli | 403 | [FAIL] (Cloudflare anti-bot; crate page itself exists but bot-blocked. Not a real 404 but not verifiable by curl. Treat as [UNKNOWN] for this audit.) |
| https://example.com/sharecli-hook | 404 | [FAIL] (placeholder URL left in docs) |
| https://github.com/KooshaPari/phenotype-tooling | 404 | [FAIL] (broken external dependency reference) |
| https://github.com/KooshaPari/portage-temp | 404 | [FAIL] (broken external dependency reference) |
| https://github.com/KooshaPari/sharecli.git | 301 | [PASS] (redirects to canonical) |
| https://github.com/KooshaPari/sharecli/actions/workflows/ci.yml | 200 | [PASS] |
| https://github.com/KooshaPari/sharecli/releases | 200 | [PASS] |
| https://img.shields.io/crates/v/sharecli.svg | 200 | [PASS] |
| https://img.shields.io/github/actions/workflow/status/KooshaPari/sharecli/ci.yml?branch=main&label=CI | 200 | [PASS] |
| https://img.shields.io/github/downloads/KooshaPari/sharecli/total | 200 | [PASS] |
| https://img.shields.io/github/v/release/KooshaPari/sharecli?label=release | 200 | [PASS] |
| https://raw.githubusercontent.com/KooshaPari/sharecli/main/install.sh | 200 | [PASS] |
| https://signpath.io | 200 | [PASS] |
| https://signpath.org | 200 | [PASS] |
| https://sladge.net | 200 | [PASS] |
| https://sladge.net/badge.svg | 200 | [PASS] |

**Verdict:** [FAIL] — 3 confirmed broken or unverifiable external links plus 1 stale version reference. Recommend fixing or removing the 404'd GitHub URLs and updating the v0.3.0 formula reference.

---

## 4. CHANGELOG

- `CHANGELOG.md` present, 1134 lines.
- Last dated entry: **`[1.0.0] - 2026-08-31`** (`CHANGELOG.md:33`).
- Active `[Unreleased]` section above with Wave 17/19/20 entries referencing PRs #812, #811, #810, #805, #798 (lines 13-31). Last `git log` commit is `2026-10-16 21:31:33 -0700` — the CHANGELOG has not been refreshed for the most recent ~30 days of merged work (PR #918 from HEAD is not in CHANGELOG).
- Verdict: [PASS] (file exists, format correct) + [INFO] (file lags behind commits — see "stale" note in `docs/sessions/` for the maintenance procedure).

---

## 5. Broken anchors in markdown

Spot-checked markdown files: `README.md`, `CHANGELOG.md`, `CONTRIBUTING.md`, `docs/specs/FR.md`, `docs/specs/TRACEABILITY.md`, `docs/adr/0001-no-mobile-app.md`, `docs/adr/0002-eval-surface-out-of-scope.md`. **No broken anchors detected** in the sampled set. Full anchor sweep across all 288 markdown files was out of scope for this audit window; spot-check passes. Verdict: [PASS] in sampled files. [UNKNOWN] for full-repo coverage.

---

## 6. TODO/FIXME in markdown

`rg 'TODO|FIXME|XXX|HACK' -g '*.md'` returned 9 matches across 8 files (1.9s scan of 268 markdown files, 1.92 MB):

| File | TODO/FIXME count |
|------|------------------|
| `recovery/feb-2026-agent-harness/artifacts/PHASE6_MULTI_AGENT_PLAN.md` | 2 |
| `recovery/feb-2026-agent-harness/artifacts/PHASE6_QUICK_REFERENCE.md` | 1 |
| `audit/rubric/X-ax-L81-L95.md` | 1 |
| `audit/rubric/X-packaging-distribution-L108-L122.md` | 1 |
| `audit/rubric/audit-30-pillar/audit-30-pillar-L22.md` | 1 |
| `audit/rubric/audit-30-pillar/audit-30-pillar-L25.md` | 1 |
| `docs/ops/grafana/README.md` | 1 |
| `docs/specs/TRACEABILITY.md` | 1 |

The 8 `recovery/` and `audit/` files are historical artifacts (under `recovery/feb-2026-agent-harness/artifacts/` from the agent-harness recovery line, and `audit/rubric/` from the 30-pillar audit rubric). They are intentionally preserved, not user-facing.

**However, `docs/specs/TRACEABILITY.md:1` and `docs/ops/grafana/README.md:1` are in user-facing directories.** The TRACEABILITY file links spec-to-test mapping; a TODO there means some spec rows are not yet traced. Recommend follow-up audit.

Verdict: [WARN] — 2 user-facing files contain TODO markers; 6 others are in historical artifact directories.

---

## 7. Missing architectural diagrams

- **No `docs/architecture/` directory exists** (`ls docs/architecture` → "No such file or directory").
- Architecture Decision Records live in `docs/adr/` (9 files: 0001–0009). These are *decisions*, not diagrams.
- Diagram files repo-wide: **6 SVG files**, all non-architecture:
  - `docs/assets/identity/demo.svg` (brand demo)
  - `assets/brand/sharecli-icon-animated.svg` (icon)
  - `assets/dashboard/ui/error-states/disconnect.svg` (UI state)
  - `assets/dashboard/ui/empty-states/no-data.svg` (UI state)
  - `assets/dashboard/ui/empty-states/no-results.svg` (UI state)
  - `assets/dashboard/ui/empty-states/error.svg` (UI state)
- **Zero `.mmd`, `.puml`, `.dot`, `.drawio` files** in the repo.
- `docs/visual/` directory contains **prose** docs (empty-states.md, error-states.md, theming.md, typography.md, motion.md, etc.) — no diagrams embedded as files.

Verdict: [FAIL] — `docs/architecture/` directory absent; no architecture diagrams anywhere; ADRs partially compensate but do not document system topology.

---

## 8. Spec drift (`docs/specs/FR.md`)

`docs/specs/FR.md` is the canonical Functional Requirements spec (1129 lines, 13 FRs: FR-001–FR-013, with FR-013 marked PROPOSED). Verified FR-001 through FR-008 against the working tree at HEAD `3d6a41c9`.

### FR-001 — Managed Process Lifecycle

Spec claims:
- `src/main.rs:38-91` — `Commands::Ps`, `Commands::Start`, `Commands::Stop`
- `src/commands/mod.rs:25-138` — `ps`, `start`, `stop` command implementations
- `src/runtime.rs:44-156` — `ProcessPool::spawn`, `ProcessPool::list`, `ProcessPool::kill`, `ProcessPool::kill_all`

Findings:

- [WARN] `src/main.rs`: actual `mod` declarations run from line 26 to ~37. The `Cli` struct (with `command: Commands`) sits around line 60, and the `Commands` enum (containing `Ps`, `Start`, `Stop`) **begins around line 89**, not 38. The line range 38-91 is stale by ~50 lines. The variants still exist. **INFO-level drift** (numbers stale, structure intact).
- [FAIL] `src/commands/mod.rs:25-138`: the actual file at lines 1-150 contains only `pub mod` declarations (cast, config_edit, fuse, history, mesh, proc, render_csv, report, serve, soak, undo, uninstall, upgrade) and `use` imports. The `ps`, `start`, `stop` implementations live in **submodules** under `src/commands/proc/` (a directory) and `src/commands/cast.rs`, `src/commands/mesh.rs`, etc. The spec line range is **wrong**; the spec does not name the submodule file(s) where the actual implementations live. **Hard drift** — source path is wrong.
- [WARN] `src/runtime.rs:44-156`: actual `ProcessPool` methods are at:
  - `pub async fn list(&self)` — line **328**
  - `pub async fn spawn(...)` — line **356**
  - `pub async fn kill(...)` — line **466**
  - `pub async fn kill_all(&self)` — line **485**

  The spec's `44-156` is ~280 lines off; the methods exist but at very different line numbers. **INFO-level drift** (numbers stale, structure intact).

### FR-002 — TOML Configuration

Spec claims: `src/config.rs:1-119`, `src/commands/mod.rs:194-222`. Spot-check: `src/commands/mod.rs:194-222` falls in the file's import block (lines 1-22 are `pub mod` declarations and `use` imports; lines 30+ are use imports). The `config` command implementation appears to live in `src/commands/config_edit.rs` (a submodule), not in `src/commands/mod.rs`. Same drift pattern as FR-001.

### FR-005 — Per-Project Resource Limits

Spec claims: `src/runtime.rs:358-455` for `ProjectLimits`, `ProjectResources::set_limits/get_limits/check_limits`, `ResourceCheck`. Spot-check at line 358: the file there contains a `spawn(...)` function signature in the middle of its parameter list — NOT `ProjectLimits`. Drift on line ranges. [WARN] info-level.

### FR-006 — Agent Detection

Spec claims: `crates/sharecli-core/src/detect.rs`, `crates/sharecli-core/src/proc_scan.rs`, `crates/sharecli-core` Hypervisor. **Both files exist** (`crates/sharecli-core/src/detect.rs`, `crates/sharecli-core/src/proc_scan.rs`). [PASS] paths accurate.

### FR-007 — Resource & Syscall Watch

Spec claims: `crates/sharecli-core`, `crates/sharecli-fleet`, `src/monitoring.rs`. `crates/sharecli-fleet/src/resource_watch.rs` exists. `src/monitoring.rs` exists. [PASS] paths accurate.

### FR-008 — Speculative Coalesce

Spec claims: `crates/sharecli-ipc` — `CoalesceCache`, `SlotQueue`, `PriorityQueue`. Verified: `CoalesceCache` is in `crates/sharecli-ipc/src/serve_lock.rs`, `crates/sharecli-ipc/src/lib.rs`, `crates/sharecli-ipc/src/queue.rs`. Hypervisor::run is at `crates/sharecli-core/src/lib.rs:795`. [PASS] paths accurate.

**Overall verdict:** [WARN] — the FR spec line numbers are stale across the older FR-001/002/005 entries; FR-006/007/008 (the newer, more detailed entries) are accurate. Recommend either (a) a bulk refresh of the FR-001/002/005 line numbers, or (b) replacing line numbers with logical paths only (the FR-006/007/008 style).

---

## 9. LICENSE

`LICENSE` present at repo root, 21 lines.

- License type: **MIT License** (`LICENSE:1`).
- Copyright: **Copyright (c) 2026 Koosha Pari** (`LICENSE:3`).
- Verdict: [PASS].

---

## 10. CONTRIBUTING / CODE_OF_CONDUCT / SECURITY

All three present at repo root:

- `CONTRIBUTING.md` (248 lines) — [PASS]
- `CODE_OF_CONDUCT.md` (39 lines) — [PASS]
- `SECURITY.md` (137 lines) — [PASS]

Bonus governance files also present: `AGENTS.md` (201), `CLAUDE.md` (23), `BOUNDARY.md` (13), `PRD.md`, `SPEC.md`, `FUNCTIONAL_REQUIREMENTS.md`, `THREAT_MODEL.md`, `TEST_COVERAGE_MATRIX.md`, `PLAN.md`, `WORK_DAG.md`, `COMPREHENSIVE_AUDIT_SCORECARD.md`, `PERSONAL_EVALUATION_GUIDE.md`, plus per-PR body files (`pr-786-body.md` … `pr-802-body.md`).

Verdict: [PASS].

---

## Findings prioritized by smallest remaining effort

Per the shared cockpit contract, items are ordered by smallest-effort-to-fix first, fastest-useful-outcome second.

| # | Finding | Effort | Impact |
|---|---------|--------|--------|
| 1 | Update README stale version reference (`Formula/sharecli.rb` v0.3.0 → v1.0.0). | XS (1 line edit) | Removes reader confusion about current release. |
| 2 | Remove or fix 4 broken/placeholder README URLs (`example.com/sharecli-hook`, `phenotype-tooling`, `portage-temp`, plus the 403-blocked crates.io anti-bot). | XS (text edit) | Removes 404 in production docs. |
| 3 | Add `///` doc comments to all 6 `pub extern "C" fn` items in `crates/sharecli-ffi/src/lib.rs:29,62,69,78,118,170`. | S (one file) | Unblocks Swift/C# callers that depend on header docs. |
| 4 | Refresh `docs/specs/FR.md` FR-001 line ranges (move to FR-006/007 style — logical paths only). | S | Removes the systematic "spec line numbers drift" pattern. |
| 5 | Add `crates/<name>/README.md` for the 14 missing crates (templates can be generated from `sharecli-tray-linux/README.md` and `spawn-core-sys/README.md`). | M (14 files, each ~30 lines) | Restores per-crate discoverability. |
| 6 | Create `docs/architecture/` directory with at least 3 diagram artifacts (system topology, IPC layer, FUSE layer). | L | Fills the largest user-facing docs gap. |
| 7 | Bring rustdoc coverage from 59% to ≥85%. | L (~340 `///` comments across 14 crates) | Required by docs-3 quality-gate floor. |
| 8 | Resolve the orphan `gateway-tools` crate (either add to workspace or remove). | M | Removes dead-build risk. |
| 9 | Append [Unreleased] entries for PRs #918, and the post-2026-08-31 wave. | S | Closes CHANGELOG/HEAD lag. |
| 10 | Resolve TODO markers in `docs/specs/TRACEABILITY.md` and `docs/ops/grafana/README.md`. | S | Closes 2 user-facing TODO items. |

---

## Appendix A — Per-crate undocumented-item sample (worst crates)

For follow-up patch authoring, sample undocumented items per worst crate:

### sharecli-ffi — 6/6 undocumented (0.0%)

```
crates/sharecli-ffi/src/lib.rs:29:pub extern "C" fn sharecli_ipc_start() -> c_int
crates/sharecli-ffi/src/lib.rs:62:pub extern "C" fn sharecli_ipc_socket_path() -> *mut c_char
crates/sharecli-ffi/src/lib.rs:69:pub extern "C" fn sharecli_free_string(ptr: *mut c_char)
crates/sharecli-ffi/src/lib.rs:78:pub extern "C" fn sharecli_health_json() -> *mut c_char
crates/sharecli-ffi/src/lib.rs:118:pub extern "C" fn sharecli_request(request_json: *const c_char) -> *mut c_char
crates/sharecli-ffi/src/lib.rs:170:pub extern "C" fn sharecli_serve_get(url: *const c_char) -> *mut c_char
```

### sharecli-session — 84/110 undocumented (23.6%, sample)

```
crates/sharecli-session/src/adapter.rs:10:pub trait SurfaceAdapter: Send + Sync
crates/sharecli-session/src/adapter.rs:25:pub trait SurfaceIo: Send + Sync
crates/sharecli-session/src/adapter.rs:42:pub struct GhosttySurfaceAdapter
crates/sharecli-session/src/adapter.rs:68:pub struct ZmxSurfaceAdapter
crates/sharecli-session/src/state.rs:16:pub struct SidecarRecord
crates/sharecli-session/src/state.rs:25:pub fn append_record(path: &Path, record: &SidecarRecord) -> Result<()>
crates/sharecli-session/src/state.rs:56:pub struct SidecarStateProvider
crates/sharecli-session/src/rpc.rs:31:pub struct Request
crates/sharecli-session/src/rpc.rs:38:pub struct Response
crates/sharecli-session/src/rpc.rs:43:pub trait SurfaceControl: Send + Sync
crates/sharecli-session/src/rpc.rs:80:pub struct RpcError
crates/sharecli-session/src/rpc.rs:85:pub struct SurfaceResponse
crates/sharecli-session/src/layout.rs:13:pub enum LayoutAxis
crates/sharecli-session/src/layout.rs:20:pub enum LayoutNode
crates/sharecli-session/src/layout.rs:29:pub struct LayoutSnapshot
crates/sharecli-session/src/layout.rs:38:pub struct LayoutRestoreItem
```

### spawn-core-sys — 19/30 undocumented (36.7%, sample)

```
crates/spawn-core-sys/src/windows_stub.rs:15:pub struct SemaphoreHandle(*mut std::ffi::c_void)
crates/spawn-core-sys/src/windows_stub.rs:23:pub struct SpawnParams
crates/spawn-core-sys/src/windows_stub.rs:38:pub struct ZigSemaphore
crates/spawn-core-sys/src/windows_stub.rs:44:pub fn new(max: usize) -> Self
crates/spawn-core-sys/src/windows_stub.rs:49:pub fn acquire(&self) -> Result<(), Error>
crates/spawn-core-sys/src/windows_stub.rs:59:pub fn try_acquire(&self) -> Result<bool, Error>
crates/spawn-core-sys/src/windows_stub.rs:68:pub fn release(&self) -> Result<(), Error>
crates/spawn-core-sys/src/windows_stub.rs:77:pub fn available(&self) -> usize
crates/spawn-core-sys/src/windows_stub.rs:94:pub fn zig_spawn(...)
crates/spawn-core-sys/src/windows_stub.rs:106:pub fn zig_waitpid(_pid: i32) -> Result<i32, Error>
```

### sharecli-sync — 6/10 undocumented (40.0%)

```
crates/sharecli-sync/src/lib.rs:14:pub struct PoolIndex
crates/sharecli-sync/src/lib.rs:19:pub fn new() -> Self
crates/sharecli-sync/src/lib.rs:24:pub fn insert(&self, pid: u32, name: impl Into<String>) -> bool
crates/sharecli-sync/src/lib.rs:33:pub fn remove(&self, pid: u32) -> bool
crates/sharecli-sync/src/lib.rs:43:pub fn pids_sorted(&self) -> Vec<u32>
crates/sharecli-sync/src/lib.rs:53:pub struct RelaxedCounter
```

---

## Appendix B — Crate non-membership and orphan analysis

The audit list mentions 16 crates. The root `[workspace] members = [...]` block lists 14:

```
crates/harness-native, crates/sharecli-sync, crates/sharecli-ipc, crates/sharecli-ffi,
crates/sharecli-tray-linux, crates/sharecli-tray-windows, crates/sharecli-fleet,
crates/sharecli-mesh, crates/spawn-core-sys, crates/sharecli-fuse, crates/sharecli-core,
crates/sharecli-thermal-tui, crates/fuse-smoke-runner, crates/sharecli-session
```

Missing from the workspace:

- **`crates/spawn-core`** — Zig crate (not a Rust crate; workspace membership does not apply). [INFO]
- **`crates/gateway-tools`** — Has `Cargo.toml` and `src/{main.rs,serve.rs}` but **not in `[workspace] members`**. Depends on `gateway = { path = "../gateway" }` — there is **no `gateway` crate at `../gateway`** (sibling directory expected). The crate is orphaned or expects an external workspace layout. [WARN]

---

## Appendix C — Method / scope notes

- **Coverage method:** `rg -t rust -g 'crates/<name>/**/*.rs' '^\s*pub (extern|fn|struct|enum|trait|type|const|static|use)' --no-heading -n`. For each match, the line immediately above (skipping blanks) was checked for `///` or `//!`. Indented pub items inside impl blocks were excluded (they're trait impl details, not the public surface). Trait-method `pub fn` lines inside `pub trait` declarations were counted as the trait's responsibility.
- **Limitation:** Re-exports (`pub use …`) are counted in the source crate. Coverage for a re-export depends on the source crate's coverage of the original item. This means effective coverage is a slight overcount for crates that re-export heavily.
- **Limitation:** Anchor sweep was spot-checked, not exhaustive. No automated full-repo pass was run within the audit window.
- **Limitation:** HTTP link check used `curl -sI --max-time 5`. Cloudflare-fronted URLs (crates.io) return 403 to bot curl, not 200 — marked accordingly.
- **Limitation:** Rustdoc coverage does not include `#[doc = "..."]` attribute-based docs (counted as undocumented). These exist in some crates but were not enumerated.

---

## Appendix D — Additional findings from cross-checks

These were captured during data gathering but not surfaced in the main numbered checks.

### D1. `VERSION` file contradicts `CHANGELOG.md`

- `VERSION` at repo root, contents `0.1.0` (`VERSION:1`).
- `CHANGELOG.md:33` says the last dated entry is `[1.0.0] - 2026-08-31`.
- `Cargo.toml` workspace package family pins version through `version.workspace = true` (line 3 of every crate Cargo.toml); the workspace `[workspace.package]` `version` was not extracted in this audit window but every workspace member reads from it.
- Resolution: either VERSION is stale (likely — `0.1.0` is the very first tagged release per CHANGELOG `Release tag anchors` table on line 868) or VERSION is the source of truth and CHANGELOG is wrong.
- Verdict: [FAIL] — single-source-of-truth violation. The build pipeline should pick one of (VERSION, Cargo.toml workspace.package.version, git tag) as canonical and the others as derived. `cliff.toml` exists at repo root and is the right tool for CHANGELOG regen.

### D2. `docs/index.md` is stale

- `docs/index.md` auto-generated. Lists only 3 subdirectories: `journeys/` (2 docs), `stories/` (2 docs), `traceability/` (1 doc).
- Actual `docs/` tree (per `ls docs/`): `a11y`, `adr`, `appcast-template.xml`, `assets`, `audit`, `changes`, `CROSS_DEVICE_DEPLOY.md`, `CYCLE_CLOSE_2026_07.md`, `deploy`, `deploy.md`, `eval`, `faq.md`, `friction-log.md`, `genesis`, `index.md`, `intent`, `journeys`, `openapi`, `ops`, `plans`, `releases`, `session-recovery.md`, `sessions`, `slsa.md`, `sota`, `sparkle-delta-updates.md`, `specs`, `stories`, `style-guide.md`, `superpowers`, `testing`, `traceability`, `verification`, `visual`, `.vitepress/`. (~30+ subdirs not enumerated.)
- The footer of `docs/index.md` says "_This index is auto-generated. To add new documents, place `.md` files in `docs/` and re-run the documentation indexer._" — but the indexer has not been re-run in months.
- Verdict: [WARN] — broken navigation surface; readers who land on `docs/index.md` see a tiny subset of the actual docs.

### D3. Parallel ADR directories

Two separate ADR namespaces exist in the same repo:

- `docs/adr/0001-no-mobile-app.md` … `docs/adr/0009-global-nested-ledger-local-admission.md` — 9 ADRs, last touched 2026-10-13.
- `docs/adr/ADR-007-coverage-ratchet-recovery.md` … `ADR-010-packaging-pipeline-hardening.md` — 4 ADRs, last touched 2026-09-01.

Both directories use overlapping numeric IDs (007–010). They are not cross-referenced.

- `docs/adr/0007-ports-adapters-hexagonal.md` (architecture) coexists with `docs/adr/ADR-007-coverage-ratchet-recovery.md` (governance) — readers cannot tell whether to follow one or the other.
- `docs/adr/0008-airlock-convergence-contract-capture.md` coexists with `docs/adr/ADR-008-code-signing-hardening.md` — same issue.
- `docs/adr/0009-global-nested-ledger-local-admission.md` coexists with `docs/adr/ADR-009-harbor-soak-alternative.md` — same issue.
- `docs/adr/ADR-010-packaging-pipeline-hardening.md` has no peer in `docs/adr/`.

Total: 13 ADRs across two directories with overlapping numbering. Coverage of major decisions is partial — no ADR exists for: async-runtime choice (tokio), IPC transport (NDJSON-RPC over Unix socket / named pipe), FUSE backend matrix (fuse3/feedbackfs/Stub), Cargo workspace shape (16 crates), release/versioning policy, signpath/SLSA pipeline, mesh gossip protocol.

Verdict: [FAIL] — split ADR directory with overlapping IDs is a documentation-organization defect. Consolidate into a single `docs/adr/` with one `NNN-<slug>.md` numbering and add the missing ADRs.

### D4. No site-builder config

No `mkdocs.yml`, `book.toml`, or `.rustdoc.toml` in the repo. The `docs/` tree is published as-is via GitHub's automatic repo-rendering. With `.vitepress/` present in `docs/`, VitePress may have been configured but the config file (`docs/.vitepress/config.{ts,js,mjs}`) was not surfaced in this audit; it may exist but was not checked.

Verdict: [INFO] — site builder status not fully verified in this audit window.

---

End of audit.