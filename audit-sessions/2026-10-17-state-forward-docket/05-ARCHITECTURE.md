# 05 — Implementation, architecture and deployment reality

## 5.1 System context (C4 L1)

```
+----------------------------------------------------+
|              Operator (human)                       |
|  - reads sharecli ps/status                         |
|  - sets runtime caps via sharecli config            |
+--------+---------------------------+----------------+
         |                           |
         v                           v
+------------------+        +----------------------+
|  sharecli CLI    |        |  sharecli-tray        |
|  (clap derive)   |        |  (linux/win)          |
|  ps/start/stop/  |        |  live operator UI     |
|  config/project  |        +----------+-----------+
|  run/status      |                   |
+--------+---------+                   v
         |                  +----------------------+
         v                  | OS notifications /    |
+------------------+        | system tray           |
| sharecli-core    |        +----------------------+
|  detect          |
|  proc_scan       |
|  transaction     |
|  contract        |
+----+------+------+
     |      |
     v      v
+--------+ +--------+
|sharecli| |sharecli|
|-ipc    | |-fleet  |
|coalesce| |thermal |
|cache   | |governor|
|        | |registry|
+--------+ +--------+
     |      |
     +-+----+
       v
+------------------+      +-------------------+
|  FUSE layer      |      |  Zig bridge        |
|  sharecli-fuse   |<---->|  crates/spawn-core |
|  InterceptFs     |      |  (not in workspace)|
+------------------+      +-------------------+
       |
       v
+------------------+
|  /tmp/sharecli   |
|  mount point     |
+------------------+

[External adapters, not in sharecli binary]
- codex agent (separate process, supervised)
- claude agent (separate process, supervised)
- thegent (separate process, supervised)
```

## 5.2 Component map (C4 L2)

| Capability | Repository + path | Interface | Implementation state | Runtime evidence | Missing bridge |
|---|---|---|---|---|---|
| **CLI** | sharecli `src/main.rs:1-1808`, `src/commands/{mod,serve,proc,project,config,run,start,stop,status}.rs` | clap derive + sysexits-style exit codes | real | `sharecli --help` PASS; CI tests; live pool probe | none |
| **Configuration** | `src/config.rs:1-1166` | TOML `~/.config/sharecli/config.toml` | real | `cb37decc` validation test; Quality Gate Unit Tests | runtime cap enforcement measured at config layer only, not at process layer |
| **Process pool** | `src/runtime.rs:1-1358` + `sharecli-core::Hypervisor` | internal + `start/stop` cmd | real | `c01_climb2_core.rs` | `sharecli-runtime` crate not extracted (per `lib-sprawl-plan.md` Phase 4, deferred to v1.1) |
| **IPC** | `crates/sharecli-ipc/src/{handler,queue,lib}.rs` | UDS (Unix Domain Socket) on `0o600` mode + peer credentials | real | `c01_climb2_ipc.rs`; `acd268bd` UDS-mode test; `4f01d019` WaiterTicket RAII | `SpeculativeCoalescer` lands via PR #927 |
| **FUSE** | `crates/sharecli-fuse/src/{lib,backend,intercept}.rs` (1,709 LoC) | FUSE mount at `/tmp/sharecli-mnt` | real | `c01_fuse_mount_lifecycle.rs`; `c01_intercept_fs.rs` | `winfsp_mount.rs` Windows path not tested in this session |
| **Fleet** | `crates/sharecli-fleet/src/{lib,registry,resource_watch,thermal}.rs` | in-process pub/sub | real | `c01_climb2_fleet.rs` | `sharecli-fleet`'s 110 pub items flagged as facade-bloat; ADR-0007 reconciliation in PR #935 |
| **Mesh** | `crates/sharecli-mesh/src/{lib,smart_merge,task_queue,worktree_pool}.rs` | cross-agent worktree pool | real | `c01_climb2_mesh.rs`; `operator_status.rs` | rustdoc 74% |
| **Harness native** | `crates/harness-native/src/{is_agent,find_real,strategies/*}.rs` | strategy trait | real (post PR #929 / commit `878212aa` on `chore/cvp-1h-queue-2026-10-17`) | `is_agent` unit test on branch | not on main yet |
| **Tray (Linux)** | `crates/sharecli-tray-linux/src/{lib,ipc}.rs` | libappindicator | real (after tray-icon fix) | `ipc.rs` tests (PR #924) | ksni backend variant tested separately |
| **Tray (Windows)** | `crates/sharecli-tray-windows/src/{lib,ipc,Cargo.toml}.rs` | ksni / winfsp | real (after PR #938 lands) | `ipc.rs` tests (PR #925) | tray-icon 0.25.0 backend bug — uncommitted fix |
| **FFI** | `crates/sharecli-ffi/src/lib.rs` | 6 `pub extern "C" fn` (after `26d65067` `bf955e66` `3cd80326`) | real | PR #925 coverage | not used by any third-party today |
| **Session** | `crates/sharecli-session/src/{lib,adapter,discovery,events,layout,ledger,migration}.rs` | in-process + file store | real | integration test; `0358ee99` admission guard | `sharecli-session/Cargo.toml` updated in `5f2425ca` |
| **Teamcomm CLI** | `crates/teamcomm-cli/src/{output,connect,main}.rs` | sub-CLI binary | real | `output.rs` test (PR #924 / `1dfc820d`) | `default_socket_path` env test (lane 2 critical path) |
| **Gateway tools (off-workspace)** | `crates/gateway-tools/src/{main,serve}.rs` (1,025 + 557 LoC) | CLI binary | on-disk only | none | workspace membership via PR #923 |
| **Spawn core (Zig, off-workspace)** | `crates/spawn-core/` | Zig → Rust FFI | on-disk only | none | `zig build` not wired; ADR-0006 placement |
| **Docs / FR index** | `docs/specs/FR.md` (Phase 3 covers FR-001..FR-005; FR-006..FR-012 phased) | markdown | real | rendered on PRs | FR-006..FR-012 not all numbered yet (per `FR.md` §Conventions) |
| **CHANGELOG** | `CHANGELOG.md` | keep-a-changelog | partial (reconciled at `2a7a6267`) | rendered | cliff.toml template var `commit.link` is non-standard; needs custom template |

## 5.3 Runtime evidence table (per component)

| Component | Last successful test | Last failed test | Time since last | CI gate |
|---|---|---|---|---|
| CLI (clap derive) | `Lint & Format` 2026-10-17 07:05:31 PASS | `cargo bench (gate)` 2026-10-17 07:15:59 FAIL (perf regression, not functional) | 12h30m | required |
| sharecli-core | `Loom (sharecli-sync)` 2026-10-17 07:23:03 PASS; `Rust` 2026-10-17 07:23:11 PASS | `Cargo Deny (Advisories + Licenses)` 2026-10-17 07:24:30 FAIL (advisory) | 12h30m | required |
| sharecli-ipc | `Unit Tests` 2026-10-17 07:08:12 PASS (when green); `Cross-Platform ubuntu-latest` 2026-10-17 07:18:49 FAIL (cross platform) | — | 12h30m | required |
| sharecli-fuse | `Cross-Platform ubuntu-latest` 2026-10-17 07:18:49 FAIL (Linux build only) | — | 12h30m | required |
| sharecli-fleet | `88-Pillar Scorecard` 2026-10-17 07:12:12 PASS | — | 12h30m | soft |
| sharecli-mesh | `88-Pillar Scorecard` 2026-10-17 07:12:12 PASS | — | 12h30m | soft |
| sharecli-session | `AgilePlus Pillar Scorecard` 2026-10-17 07:11:28 PASS | — | 12h30m | soft |
| sharecli-ffi | `score` 2026-10-17 07:11:28 PASS | — | 12h30m | soft |
| sharecli-tray-linux | `Operator Display coverage` (PR #924) PASS | — | 11h45m | (PR-test) |
| sharecli-tray-windows | `C-ABI coverage` (PR #925) PASS | — | 11h45m | (PR-test) |
| gateway-tools | none | (off-workspace) | — | not exercised |
| spawn-core | none | (off-workspace; Zig not built) | — | not exercised |
| hermetic build | `Hermetic Build (Hard Gate)` 2026-10-17 07:12:04 PASS and 07:08:09 PASS | — | 12h30m | required (Hermetic Build) |
| reproducible build | `Repro Check (L52)` 2026-10-17 07:05:20 PASS | — | 12h30m | required |
| fuzz | `cargo fuzz (toml_lite, dns_query_parser, snmpv3_msg, ssh_packet, coap_option_parse, ldap_filter)` all PASS 2026-10-17 07:11:32 to 07:20:40 | — | 12h30m | soft |
| miri | `cargo miri (soft)` 2026-10-17 07:03:07 PASS | — | 12h30m | soft |
| codeql | `codeql` 2026-10-17 07:13:59 PASS | — | 12h30m | soft |
| semgrep | `semgrep-cloud-platform/scan` 2026-10-17 07:08:01 PASS | — | 12h30m | soft |
| trufflehog | `TruffleHog Scan` 2026-10-17 07:16:13 PASS | — | 12h30m | soft |
| secret detection | `Secret Detection` 2026-10-17 07:16:53 PASS | — | 12h30m | soft |
| dependency review | `Dependency Review` 2026-10-17 07:15:42 PASS | — | 12h30m | required |
| OSV / GHSA | `OSV / GHSA lockfile scan (required)` 2026-10-17 07:21:57 PASS | — | 12h30m | required |
| scorecard (88-pillar) | `88-Pillar Scorecard` 2026-10-17 07:12:12 PASS | — | 12h30m | soft |
| agileplus scorecard | `AgilePlus Pillar Scorecard` 2026-10-17 07:11:28 PASS | — | 12h30m | soft |
| accessibility | `axe dashboard (WCAG 2.x Level A)` 2026-10-17 07:08:14 PASS; `dashboard keyboard Tab-cycle` 2026-10-17 07:16:13 PASS; `vale inclusive-language (docs)` 2026-10-17 07:09:45 PASS | — | 12h30m | soft |
| visual regression | `dashboard PNG hard diff` 2026-10-17 07:13:20 PASS; `visual` 2026-10-17 07:19:19 FAIL (transient Zig download) | — | 12h30m | soft |
| flake-tracker | `flake-tracker / report` 2026-10-17 07:09:45 PASS | — | 12h30m | soft |
| soaks | `healthz soak (soft)` 2026-10-17 07:11:27 PASS; `bench run` 2026-10-17 07:20:22 FAIL | — | 12h30m | soft |
| bench (gate) | `cargo bench (gate)` 2026-10-17 07:15:59 FAIL (perf regression) | — | 12h30m | required (BLOCKING) |
| cargo deny | `cargo deny` 2026-10-17 08:02:35 FAIL | — | 11h45m | required (BLOCKING) |
| cross / x86_64-unknown-linux-musl | `cross / x86_64-unknown-linux-musl` 2026-10-17 07:15:59 FAIL | — | 12h30m | required |
| cross / x86_64-unknown-freebsd | `cross / x86_64-unknown-freebsd` 2026-10-17 07:20:26 FAIL | — | 12h30m | required |
| cross / wasm32-unknown-unknown | `cross / wasm32-unknown-unknown` 2026-10-17 07:17:08 FAIL | — | 12h30m | required |
| ci / lint | `ci / lint` 2026-10-17 07:28:38 PASS (after a 07:18:56 FAIL was retried) | — | 12h30m | required |
| ci / test | `ci / test` 2026-10-17 07:29:15 PASS (after a 07:08:58 FAIL was retried) | — | 12h30m | required |
| CI Success Gate | `CI Success Gate` 2026-10-17 07:30:35 PASS | — | 12h30m | required |

## 5.4 Implementation honesty inventory

| Surface | Honest about state | Code evidence |
|---|---|---|
| `monitoring.rs` | YES — module-level doc says "reserved for future integration" | `#![allow(dead_code)]` at top, comment "stub" |
| `pyroscope_stub.rs` | YES — module name self-documents | `#![allow(dead_code)]` |
| `crates/gateway-tools` | on disk but not in workspace; not part of `cargo build` | absent from `[workspace] members` |
| `crates/spawn-core` | on disk, not in workspace; Zig build not exercised | absent from `[workspace] members` |
| `crates/harness-native/src/is_agent.rs` (audit baseline) | NO — silent `process::exit(1)` | `878212aa` fix on `chore/cvp-1h-queue-2026-10-17` |
| `crates/sharecli-tray-windows/Cargo.toml` (tray-icon) | NO — silently broken on Linux | tray-icon 0.25.0 `pub(crate)` re-export bug; dirty worktree fix |
| 8 `unreachable!()` sites | YES (defensive; invariants hold today) | audit §4.1 |
| `sharecli-fleet`'s 110 pub items | ambiguous — facade bloat vs OS-layer breadth | audit §4.8 MEDIUM |
| 14 files > 1000 LoC | HONEST — not pretended to be at AGENTS.md 500L cap | AGENTS.md cap acknowledged in audit §4.4 |

## 5.5 Trust boundaries (C4 L3)

| Boundary | Direction | Protocol | Authentication | Encryption | Notes |
|---|---|---|---|---|---|
| Operator ↔ CLI | inbound | local TTY | (none — local user) | n/a | sysexits-style exit codes |
| CLI ↔ sharecli-core | internal | function calls | (same process) | n/a | strategy trait + impl |
| sharecli-core ↔ sharecli-ipc | internal | function calls + `pub use` re-export | (same process) | n/a | ADR-0007 §1: forbidden by ADR; in practice present |
| sharecli-core ↔ sharecli-fleet | internal | function calls | (same process) | n/a | same as above |
| sharecli-core ↔ sharecli-fuse | internal | function calls | (same process) | n/a | same as above |
| sharecli-ipc ↔ external IPC client | inbound | Unix Domain Socket, 0o600, peer credentials | SO_PEERCRED | n/a | `acd268bd` |
| sharecli-fuse ↔ OS kernel | inbound | FUSE protocol | (kernel) | n/a | libfuse / winfsp |
| sharecli-tray ↔ sharecli-ipc | outbound | UDS | SO_PEERCRED | n/a | local only |
| sharecli-ffi ↔ third-party | bidirectional | C-ABI | (caller's responsibility) | n/a | 6 `pub extern "C" fn` |
| sharecli-mesh ↔ sharecli-mesh (cross-process) | bidirectional | worktree_pool over UDS | SO_PEERCRED | n/a | single-host only |

## 5.6 Deployment reality

| Form | Available today | Where |
|---|---|---|
| Source build | yes | `git clone … && cargo build --release` |
| Prebuilt binary | no (no release artifact published) | — |
| Homebrew | not yet | — |
| apt | not yet | — |
| Docker | not yet (no Dockerfile in repo) | — |
| Windows MSI | not yet | — |
| macOS .app | not yet | — |
| Helm chart | n/a (single-host) | — |
| Codesign | macOS + Windows via `codesign.yml` (post #918) | SignPath Foundation |
| Distribution channels | GitHub Releases (planned but no v1.0 tag yet) | — |
| Production install | operator's host | 1 host = 1 install = 1 beneficiary |

**No prebuilt binary is published today.** All installs go through `cargo install` or `cargo build --release` from source.
