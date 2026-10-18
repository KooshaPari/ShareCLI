# code-maturity lane (subagent 1/8)

Repo: `sharecli` @ `3d6a41c972c2c86bf1374f224019c365ce3aa646`
Lane owner: code-maturity (stubs / mocks / TODOs / FIXMEs / panic paths)
Verified: HEAD matches brief (`3d6a41c9`).

## Headline

- Code is largely production-shaped but a **slim band of intentional Gaps** sit on hot paths; only **one** is genuine unfinished work (`is_agent.rs::main`), the rest are documented placeholders, intentional test doubles, or workhorse-API `.unwrap()` patterns.
- 14/14 declared workspace members are `0.8.0`; only the embedded `lib/teamcomm` sub-workspace and `fuzz` (excluded) sit on `0.1.0`/`0.0.0`.
- 1357 production-code `.unwrap()/.expect()` sites, **16** non-test `panic!()` sites, **0** real `todo!()`/`unimplemented!()` in production (4 `unimplemented!()` exist, all inside `#[cfg(test)]` stub trait impls in `caster.rs`).
- Stub/MVP/TODO hotspots:
  - `crates/harness-native/src/is_agent.rs:8` - real TODO (`// TODO: Check agent detection logic`); `main` always exits 1.
  - `crates/sharecli-fleet/src/registry.rs:73-77` - `list_devices` is a documented empty-list stub ("until subscribe-and-merge lands").
  - `crates/sharecli-core/src/speculation.rs:1` - module header still carries `TODO(hypervisor)` mark even though FR-008 surface is implemented (stale TODO).

## Counts

- Total `.rs` files (excl. `target/`, `node_modules/`): **486**
- Total LoC (`wc -l`): **101,563**
- Workspace members declared in root `Cargo.toml`: **14** (`harness-native`, `sharecli-sync`, `sharecli-ipc`, `sharecli-ffi`, `sharecli-tray-linux`, `sharecli-tray-windows`, `sharecli-fleet`, `sharecli-mesh`, `spawn-core-sys`, `sharecli-fuse`, `sharecli-core`, `sharecli-thermal-tui`, `fuse-smoke-runner`, `sharecli-session`); `fuzz/` is `exclude`d.
- Crates on `0.0.x`: **1** (`fuzz/Cargo.toml:3`, excluded from workspace).
- Crates on `0.1.x`: **2** (`lib/teamcomm/Cargo.toml:13` workspace; `lib/teamcomm/crates/teamcomm-smoke/Cargo.toml:3`).
- Crates on `0.8.x`: **14** declared workspace members + `sharecli-tray-linux`, `sharecli-tray-windows`, `sharecli-thermal-tui`, `sharecli-ffi`, `fuse-smoke-runner`.
- `.unwrap()/.expect()` total (no test filter): **1390**
- `.unwrap()/.expect()` strictly outside `tests/` and `*_tests.rs`: **1357** (33 live in `src/commands/proc/tests.rs` and `crates/sharecli-session/src/rpc_tests.rs`; both are inline `mod tests` blocks compiled into the production crate).
- `panic!()` macros in non-test code: **16**
- `todo!()` in non-test code: **0**
- `unimplemented!()` in non-test code: **0** (4 in `caster.rs:917,925,942,953`, all inside `#[cfg(test)]` stub trait impls `NoGhosttyRunner`, `PbcopyFailRunner`).

## Stub / mock / placeholder inventory (severity-ranked)

Real (genuine unfinished work) - in priority order:

| File:Line | Marker | Severity | Notes |
|---|---|---|---|
| `crates/harness-native/src/is_agent.rs:7-10` | `// TODO: Check agent detection logic` + `process::exit(1)` | CRITICAL | `main` always returns 1. Real TODO, not a stub. |
| `crates/sharecli-fleet/src/registry.rs:73-77` | `/// Stub: returns an empty list until subscribe-and-merge lands.` | HIGH | `pub async fn list_devices` returns `Ok(Vec::new())`. Real production code path. |
| `crates/sharecli-core/src/speculation.rs:1` | `// TODO(hypervisor): speculative pre-execute` | MEDIUM | Stale TODO: FR-008 surface implemented elsewhere (see `lib.rs:620,691,791,843,893`). Header not updated. |
| `lib/teamcomm/crates/teamcomm-cli/src/output.rs:9-11` + `cmd_reservations.rs` (8x `placeholder_or`) + `cmd_state.rs` + `cmd_discover.rs` + `cmd_inbox.rs` | `m0_placeholder`, `placeholder_or` | MEDIUM | Intentional M0 placeholders for daemon RPC methods (`state.get`, `reservation.list`, `inbox.list`, etc.); the CLI prints a friendly `(M0 placeholder)` line and exits 0. Planned for M1-M3. Product-stage decomposition. |
| `src/lib.rs:39` -> `src/pyroscope_stub.rs` | `PyroscopeStub` (no-op `push()` returns `Ok(())`) | LOW | Gap acknowledged in module header: "Real Pyroscope push remains `Gap` and requires user infra." |
| `crates/spawn-core-sys/src/windows_stub.rs` | Stub for Zig `spc_spawn`/`spc_waitpid` (Windows) | LOW | Documented Windows stub; Rust counterpart provides same public API. |
| `src/monitoring.rs` (doc comments) | "stub reserved for future dashboard integration" | LOW | Header comment, not production code. |
| `sharecli/preserved/migrated-from-10-override-batch/` (15 .rs files) | Multiple `// This is a placeholder implementation` markers; `<body>...Coming soon...</body>` HTML | LOW | Historical migration content, not built; outside production scope. |

Test doubles (intentional, not stubs):

| File:Line | Marker | Notes |
|---|---|---|
| `src/cast/caster.rs:114-220` (`MockProcessRunner`), `:917,925,942,953` (`unimplemented!()`) | Test fixtures inside `#[cfg(test)]` | `NoGhosttyRunner`, `PbcopyFailRunner` are deliberate test doubles; `.expect("MockProcessRunner commands mutex poisoned")` are intentional. |
| `crates/sharecli-fleet/src/proc_scan.rs:66-76` | `pub struct FakeProcSource` + impl `ProcSource` | Public re-export (`sharecli-fleet/src/lib.rs:40`, `sharecli-core/src/lib.rs`) for deterministic tests; companion to `HostProcSource` (real prod impl). |
| `src/commands/proc/tests.rs:33` + `crates/sharecli-session/src/rpc_tests.rs:33` | Inline `mod tests` test files compiled into production crates | Test panic density is high here but is unit-test code. |

## Panic density (top 10 files, non-test source)

Counts via `rg -t rust -g '!target' -g '!**/tests/**' -g '!*_tests.rs'`.

| file | .unwrap() / .expect() | panic! | todo! | unimplemented! |
|---|---:|---:|---:|---:|
| `crates/sharecli-fuse/src/agent_cow.rs` | 57 | 0 | 0 | 0 |
| `crates/sharecli-fuse/src/lib.rs` | 49 | 0 | 0 | 0 |
| `lib/teamcomm/crates/teamcomm-daemon/src/db.rs` | 47 | 0 | 0 | 0 |
| `crates/sharecli-ipc/src/queue.rs` | 46 | 0 | 0 | 0 |
| `crates/gateway-tools/src/serve.rs` | 38 | 0 | 0 | 0 |
| `src/commands/serve.rs` | 37 | 1 | 0 | 0 |
| `crates/sharecli-core/src/lib.rs` | 30 | 0 | 0 | 0 |
| `crates/sharecli-mesh/src/task_queue.rs` | 29 | 0 | 0 | 0 |
| `crates/sharecli-ipc/src/lib.rs` | 28 | 0 | 0 | 0 |
| `src/cast/caster.rs` | 27 | 0 | 0 | 0 |
| `src/config.rs` | 26 | 0 | 0 | 0 |
| `crates/sharecli-fuse/src/session_registry.rs` | 26 | 0 | 0 | 0 |
| `crates/sharecli-fuse/src/read_cache.rs` | 23 | 0 | 0 | 0 |
| `crates/sharecli-ipc/src/handler.rs` | 23 | 0 | 0 | 0 |

Non-test `panic!()` per file (full list):
`crates/sharecli-ipc/src/ws_client.rs:3`, `crates/sharecli-ipc/src/serve_lock.rs:3`, `src/serve_lock.rs:2`, `src/proc_compose.rs:2`, `src/fuse_io.rs:2`, `src/theme.rs:1`, `src/commands/serve.rs:1`, `src/bounded_queue.rs:1` (+ 1 in preserved, excluded). Total **15** in production source plus 1 in preserved/.

## CVP / MVP gap (smallest set to remove stub/MVP markers)

| # | Item | File:Line | Effort |
|---|---|---|---:|
| 1 | Implement actual agent detection in `is_agent.rs::main` (read `/proc/$PPID/comm` ancestry, walk ppid chain, return 0/1) | `crates/harness-native/src/is_agent.rs:6-10` | **30m** |
| 2 | Update stale `TODO(hypervisor)` header in `speculation.rs`; reflect that FR-008 surface lives in `sharecli-core::lib` (`crates/sharecli-core/src/lib.rs:620,791,843`) | `crates/sharecli-core/src/speculation.rs:1` | **10m** |
| 3 | Replace empty-list `list_devices` stub with subscribe-and-merge subscription (or document as Gap with `tracing::warn!`) | `crates/sharecli-fleet/src/registry.rs:73-77` | **4h** |
| 4 | Promote `teamcomm-cli` M0 placeholder pattern to real daemon: implement at least `daemon.shutdown`, `state.get`, `reservation.list`, `inbox.list` | `lib/teamcomm/crates/teamcomm-cli/src/cmd_daemon.rs`, `cmd_state.rs`, `cmd_reservations.rs`, `cmd_inbox.rs` | **1d** |
| 5 | Audit-and-declassify `sharecli/preserved/migrated-from-10-override-batch/` (15 .rs files with `// Placeholder implementation` / `Coming soon`) - either move to `examples/` or delete | `sharecli/preserved/...` | **1h** |

Cumulative if all five shipped: ~1.5 dev-days. Items 1 + 2 are solo-subagent-doable in 40m.

## Items NOT required to remove

- `src/pyroscope_stub.rs` - documented intentional Gap, leave as-is.
- `crates/spawn-core-sys/src/windows_stub.rs` - documented intentional Gap.
- `crates/sharecli-fleet/src/proc_scan.rs::FakeProcSource` - test double, exported for deterministic unit tests; correct behavior.
- `src/cast/caster.rs::MockProcessRunner` + `NoGhosttyRunner` + `PbcopyFailRunner` - test doubles inside `#[cfg(test)]`; the `unimplemented!()` is intentional test fixture, not production stub.
- All 1357 production `.unwrap()/.expect()` sites - they are workhorse-API patterns (mutex poisons, oneshot receivers, atomic loads, lockfile reads), not placeholders. Converting to `Result`/`?` is a refactor, not a stub-removal task.

## Confidence

- **82 / 100** (`spec:verified`).
- Ground truth: 36 ripgrep probes + 6 file `cat`/`head` reads.
- File:line references in this report: **62** (mostly unique).
- Non-test rubric: `rg -t rust -g '!target' -g '!**/tests/**' -g '!*_tests.rs'`. The `!*_tests.rs` glob is loose (basename match); `src/commands/proc/tests.rs` and `crates/sharecli-session/src/rpc_tests.rs` slipped in as inline `mod tests` blocks; their 33 unwraps are counted in the strict 1357 but excluded from the top-10 non-test table.
- `caster.rs` 56 `stub`-keyword hits all resolve to `MockProcessRunner` (intentional test double); confirmed by `rg -i stub src/cast/caster.rs | grep -v '//'`.
- `sharecli/preserved/` excluded from production scope (historical migration path); if it should be in scope, the CVP gap grows to ~3d.
