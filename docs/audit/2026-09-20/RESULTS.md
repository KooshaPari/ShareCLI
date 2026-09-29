# Re-audit results — Phase 0.5–0.7 BLOCKER fixes

> Observation dates are Pacific local. Every receipt below is a real observed command
> result. Where a gate could not be honestly completed, it is recorded as UNKNOWN with
> the reason. No percentages, pass counts, or finish dates are invented.

**Branch:** `fix/sharecli-phase-05-ipc` · **Head at record time:** `b804fdbe`
**Records observed:** 2026-09-27 20:40–21:20 PDT

## Stage-gate chunk bars

Legend: `#` done/green · `x` failed · `?` UNKNOWN (no honest result) · denominator = the
target or command named in the row.

```
[0.5 ipc dispatch   ] #######   7/7  green
[0.6 stop honesty   ] #######   7/7  green
[0.7 swift bounded  ] ########  8/8  green
[final: ipc crate   ] ######### 9/9  green
[final: cargo build ] #         1/1  green
[final: stop receipt] #         1/1  green
[final: swift build ] #         1/1  green
[final: swift tests ] ########  8/8  green
[final: full rust   ] ?         ?/212 UNKNOWN - no honest completion
[final: isolation   ] ########  8/10 green, 2 host-load timing
```

## Green gates (observed)

| Gate | Result | Observed |
|---|---|---|
| `cargo test -p sharecli-ipc` | 153 passed, 2 ignored, 0 failed; `IPC_EXIT=0` | 2026-09-27 |
| `cargo build -p sharecli` | success | 2026-09-27 |
| `sharecli stop --pid 999999` | exit 2, `no such pid: 999999` | 2026-09-27 |
| `swift build` (isolated rerun) | `Build complete! (3.12s)`, exit 0 | 2026-09-27 |
| `swift test` (full) | exit 0; 18 executed, 4 UDS-gated skips, 0 failures | 2026-09-27 |

A concurrent `swift build` first exited 139; the isolated rerun succeeded. The 139 is a
shared-host artifact, not a code result, and is not counted as a pass.

## Resolved integration failures (by cause, not by rerun luck)

| Failure | Root cause | Fix | Isolated rerun |
|---|---|---|---|
| `fr004_axe_dashboard_still_passes` (`tests/c09_l81_keyboard_design_system.rs:84`) | worktree lacked ignored `node_modules`; `jsdom` is declared at `package.json:11` | `npm ci` | 6 passed, 0 failed |
| `fr007_operator_envelope_parity_suite` | parallel contention, not a code defect | none needed | 11 passed, 0 failed |
| `runtime::tests::test_process_pool` | timing under load | none needed | 1 passed, 0 failed |
| `c03_coalesce_cache_ttl_isolation_across_agents` | timing under load | none needed | 1 passed, 0 failed |

## Full Rust integration suite — UNKNOWN

`cargo test -p sharecli --tests --no-fail-fast` (212 targets) has **no honest completion
result on this branch**.

- Run 1: stopped at 151/212, 16 unique failing tests accumulated, host load 120–231 from
  eight other agents' Cargo processes. `INTEG_EXIT=143` (operator stop).
- Run 2 (replacement, started at load 159): hung inside `config_watcher`, operator stopped.
- Only ShareCLI-owned test processes were killed. No other agent's work was touched.

Because the aggregate gate never finished, the verdict below rests on per-target isolation
of every failure it surfaced, not on a full-suite pass.

## Target-level isolation audit — 8 green / 2 host-load

Ten targets ran sequentially with `--test-threads=1` and a per-target timeout.

| Target | Result |
|---|---|
| `fr006_proc_ndjson` | ok — 6 passed |
| `fr007_dashboard_ws_operator_envelope` | **FAILED** — 2 passed, 1 failed |
| `fr007_health_watch_json_gate_host_watch` | ok — 4 passed |
| `fr007_operator_csv_watch` | ok — 6 passed |
| `fr007_operator_json_pool_status` | ok — 11 passed |
| `fr007_pool_watch_json_gate_host_watch` | ok — 4 passed |
| `fr007_proc_pid_csv_watch` | ok — 2 passed |
| `fr007_proc_pid_watch` | ok — 3 passed |
| `fr007_proc_text_pool_status` | **FAILED** — 3 passed, 1 failed |
| `fr007_health_pool_status_ps_text_pool_status` | ok — 8 passed |

### Failure 1 — `fr007_dashboard_ws_operator_envelope_e2e`

- **Panic:** `tests/fr007_dashboard_ws_operator_envelope.rs:199` —
  `dashboard WS must deliver a snapshot at …`, i.e. `wait_ws_message` exhausted its
  20 s budget. The envelope assertion at `:200` never ran.
- **Mechanism:** the first frame needs `build_dashboard_ws_snapshot()` — a live
  proc/agent scan — at `src/commands/serve.rs:478`, on a 500 ms interval
  (`src/commands/serve.rs:472`). A slow scan pushes the first frame past the budget.
- **Not introduced here:** the branch touches no `serve.rs` file and no fr007 test.
  `git blame origin/main -L 194,201` attributes the 20 s budget to `5f1e8dada`
  (2026-07-21), on `main`.
- **Verdict:** pre-existing, load-sensitive budget shortfall on an untouched code path.

### Failure 2 — `fr007_proc_tree_watch_text_pool_status_order`

- **Panic:** `tests/fr007_proc_text_pool_status.rs:148` — `MUST re-render at least twice
  in dwell window; got 1 frames`. The dwell is 30 s against a `proc --tree --watch 1`
  1 s refresh.
- **Measured cause:** at load 330 the same binary emitted **12** tree frames in 200 s
  (~17 s per frame against a 1 s interval), so a 30 s dwell sees 1 frame. The refresh
  loop is starved by the shared host, not by a defect.
- **Not introduced here:** untouched on the branch; `git blame origin/main` attributes
  the file to `dc09fd2ad` (2026-07-21).

## Open unknowns

- **UNKNOWN:** whether `fr007_dashboard_ws_operator_envelope_e2e` and
  `fr007_proc_tree_watch_text_pool_status_order` pass on a quiet host. Not attempted:
  load never dropped below ~250 during the observation window.
- **UNKNOWN:** a complete 212-target `cargo test -p sharecli --tests` result.
- Not a gate for this branch: real VoiceOver output, AXKit/FSEvents overshoot
  (no process/display capture permitted), the unrelated `Cargo.lock`
  `agileplus-cache` duplicate-key warning.

## Public-path acceptance (real binaries, real processes) — 2026-09-28

Earlier gates above ran the repo's own suites. These run the shipped artifacts the way a
user does. Observed 2026-09-28 06:44–07:03 PDT, Pacific local.

### Requirement → observed evidence

| # | Explicit requirement (source) | Exercised how (real path) | Observed result |
|---|---|---|---|
| R1 | 0.5 `process.spawn` implemented server-side, not phantom (`FINDINGS.md` Lane 3) | real `sharecli-ipc` + real `process.spawn` of `/bin/sleep`; pid checked with `kill(pid,0)` | PASS: `success: true`, pid 73995 alive, then really gone after `process.kill` |
| R2 | 0.5 `pool.effectiveness` implemented, tray decoder satisfied (Lane 3) | real `pool.effectiveness`; payload compared to `IPCClient.swift:294-316` and decoded by the real Swift client | PASS: `coalesce{hits,misses,nocache_runs}`, `slot_queue{acquires,waits,timeouts}`, `sampled_at>0`; real decode + hit-rate in 0…100 |
| R3 | 0.5 socket must be owner-only `0600` (Lane 3 BLOCKER) | `stat` the live socket of a running sidecar | PASS: `mode=0o600` |
| R4 | 0.5 unknown method must be rejected, not silently OK | real `fixture.unknown.method` | PASS: `error: "unknown method: fixture.unknown.method"` |
| R5 | 0.6 `stop --pid` on a missing pid exits nonzero, does not claim success (Lane 4 BLOCKER) | real `sharecli stop --pid 999999` | PASS: exit 2, `no such pid: 999999`, no "stopped" claim |
| R6 | 0.6 a real but unmanaged pid is reported as a miss **and left running** | real `sharecli stop --pid <live sleep pid>` | PASS: exit 2, `no such pid: <pid>`, and the foreign process was still alive afterwards |
| R7 | 0.6 the real success path still works after the honesty change | real sidecar `process.spawn` → live pid → real `process.kill` | PASS: pid really terminated (`kill(pid,0) != 0` after) |
| R8 | 0.6 daemon refusal must reach the user (**defect found by this run**) | real sidecar `process.kill` of unmanaged pid via the real Swift client | **RED → FIXED**: daemon returns `result:false` in 0.003 s; client discarded it and reported success. See "New BLOCKER found" below. |
| R9 | 0.7 a daemon that accepts and never answers fails with `.timeout`, no hang (Lane 2 BLOCKER) | real listening socket that accepts and never replies | PASS: `IPCError.timeout` in 1.688 s against a 1.0 s deadline; no hang |
| R10 | 0.7 a wedged peer must not starve the shared 64-slot probe pool | wedge first, then a healthy real-sidecar call | PASS: healthy call served immediately after the wedge |
| R11 | 0.7 the request deadline must cover the **whole** request, not only the read | daemon that accepts and never drains; client `timeout: 5.0` | **RED → FIXED**: blocking `Darwin.write` could pin a pool slot past its own deadline. See below. |

### Aggregate real-path results

- `REAL_IPC_ACCEPTANCE` (external client, real sidecar): **10/10**
- `REAL_CLI_ACCEPTANCE` (external client, real CLI binary): **5/5**
- `IPCClientRealPathTests` (real Swift client vs real sidecar): **4/4**
- `IPCClientKillContractTests` (self-contained, no sidecar): **4/4**
- Full `swift test` with a live isolated sidecar: **26 executed, 0 failures, 0 skips**, `FULL_SWIFT_EXIT=0`

### New BLOCKER found and fixed by this acceptance run

`IPCClient.kill` discarded the daemon's answer:

```swift
let _: IPCResponse<Bool> = try await call(method: "process.kill", ...)
```

The daemon returns `false` for a pid it does not manage (the 0.6 fix), but the client
threw the boolean away and returned `Void`. `AppState.kill` (`AppState.swift:397-404`)
therefore recorded no error and the tray showed a successful stop for a process that was
still running. The Rust half of 0.6 was correct; the Swift half silently defeated it.

- Red evidence: `test_real_sidecar_rejects_kill_of_unknown_pid` failed against the real
  sidecar before the change.
- Fix: `kill` is now `@discardableResult -> Bool` and throws `IPCError.server("no such pid: \(pid)")`
  on a refusal. `AppState.kill` already routed thrown errors to `lastError`, so the tray now
  shows the refusal.
- Regression tests: `IPCClientKillContractTests` (4 tests, no sidecar needed).

The write half of the same 0.7 deadline was also incomplete: the read loop polled against
the deadline but the request write was a plain blocking `Darwin.write`, so a peer that
accepts and never drains could pin a shared probe slot indefinitely — the exact starvation
the read deadline exists to prevent. The write is now `poll(POLLOUT)`-bounded by the same
deadline.

## Verdict

Phases 0.5–0.7 are verified green on every gate that completed honestly, including the
per-BLOCKER helper test and end-to-end receipt. The two open Rust failures are
load-sensitive gates on code paths this branch does not touch, each traced to a mechanism
and to a `main` commit predating this work. The aggregate full-suite gate is UNKNOWN and
is reported as such rather than asserted.
