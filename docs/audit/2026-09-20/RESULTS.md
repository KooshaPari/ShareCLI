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

## Phase 1 task 1.1 — serve middleware layer order (2026-09-29)

`Router::layer` wraps everything added before it, so the **last** `.layer(...)` call is
outermost at request time. Observability was applied **first**, making it innermost: an
auth `401` and a rate-limit `429` short-circuited before ever reaching it, so those
failures were never counted in RED metrics and the response carried no `traceparent`.

**Red — helper coverage** (router tests driving the real `build_router`, not a copy):

| Test | Failure before the fix |
|---|---|
| `observability_records_auth_401` | `auth 401 must be counted in RED metrics: before=0 after=0` |
| `observability_records_rate_limit_429` | `429 response must carry a traceparent header` |

**Red — real public path** (committed `tests/e2e_serve_observability_order.rs`, real
`sharecli serve` binary, real socket, run against the original code by stashing the fix):

```
401 response must carry a traceparent header; got headers:
test result: FAILED. 0 passed; 1 failed
```

**Fix:** move `http_observability_middleware` to the last `.layer(...)` call in
`apply_middleware`, giving `observability -> auth -> rate limit -> route`. The relative
order of auth and rate limit is unchanged; only observability moved from innermost to
outermost. The ordering contract is now documented on `apply_middleware` and guarded by
both tests.

**Green (observed):**

| Gate | Result |
|---|---|
| `observability_records_auth_401` + `observability_records_rate_limit_429` | 2 passed, 0 failed |
| `tests/e2e_serve_observability_order` against fixed binary | 1 passed, 0 failed |
| Standalone public-path harness | `E2E_SERVE_OBSERVABILITY 6/6` |
| `cargo test --lib` | 568 passed, 0 failed, 0 ignored |
| Neighbouring serve targets (`e2e_serve_healthz`, `c00_serve_error_envelope`, `c02_serve_rate_limit`, `fr012_serve_jwt_auth`, `c07_dev_mode_gate`) | 19 passed, 0 failed |
| `cargo clippy --lib --tests` | 0 errors (pre-existing warnings only) |

The e2e asserts both halves of the fix end to end: the `401` carries a W3C-shaped
`traceparent`, and the real `GET /metrics/prometheus` reports
`sharecli_http_unauthorized_total` incremented (`0 -> 1`). Controls confirm authenticated
`/config` still returns `200` and public `/healthz` still answers without credentials.

`cargo fmt --check` still exits non-zero from **pre-existing** drift in files this branch
does not touch (`crates/sharecli-ipc/*`, `src/config.rs`, many `tests/fr007_*`); both
files changed here were formatted and are clean.

## Phase 1 task 1.2 — hot-reload trailing debounce + watch path resolution (2026-09-29)

Two independent defects, each proved red before the fix.

### 1.2a — the server loaded one config file and watched another

`sharecli serve` rebuilt its watch path inline from `dirs::config_dir()`, while
`Config::load` resolves through `Config::config_path()`, which honours
`SHARECLI_CONFIG_PATH`. With the override set, the two disagreed: the server loaded the
overridden file but watched the default location, so saving to the file it had actually
loaded produced **no reload at all**.

**Red — real public path** (`tests/e2e_serve_hot_reload.rs`, real `sharecli serve`, real
socket, run against the original code):

```
config hot-reload did not fire for SHARECLI_CONFIG_PATH;
GET /config still reports Some("initial")
test result: FAILED. 0 passed; 1 failed
```

**Fix:** `Config::config_path()` is now public and documented as the single source of
truth for path resolution; `serve` resolves through a new `config_watch_path()` helper
that delegates to it.

**Green:** `e2e_serve_hot_reload` 1 passed (reload observed after 1.3 s) and
`config_watch_path_uses_config_path_override` 1 passed.

> Harness note: the helper's first run failed on **my test**, not the product — I read
> `Config::config_path()` after restoring the environment, so it compared the restored
> default against the override. Both resolutions are now observed while the override is
> set. Recorded here rather than silently corrected.

### 1.2b — leading-edge debounce lost saves

The debouncer fired on the **first** event and dropped everything inside the 200 ms
window. For a burst of saves that means several reloads of intermediate states, and the
final save can be dropped entirely.

**Red — real file system:**

```
assertion `left == right` failed: a burst of 6 saves must coalesce into exactly one
reload, got 4
  left: 4
 right: 1
```

**Fix:** rewritten as a genuine trailing debouncer. A `Debouncer` holds a monotonic
`generation` plus the time of the *latest* event; a dedicated `config-debounce` thread
sleeps to `latest + DEBOUNCE`, extends the deadline when a newer event lands, and claims
the batch only if the generation is unchanged. The notify callback now only records
events, so the deadline can never be extended from a thread that is simultaneously
deciding to fire.

The debounce state was split into `src/config_watcher/debounce.rs` so the deadline
arithmetic is testable with injected instants instead of only through wall-clock
filesystem timing, and so both files stay under the 350-line target
(`config_watcher.rs` 252, `debounce.rs` 165).

**Second defect found by my own new test:** `Debouncer::claim` accepted the same
generation twice, so a batch could fire more than once. `debounce_claim_rejects_a_stale_generation`
and `debounce_fires_once_after_the_window` went red on it; `claim` now also requires
`pending`.

**Green (observed) — as committed in `113366e0`:**

| Gate | Result |
|---|---|
| `trailing_debounce_coalesces_a_burst_into_one_reload_of_final_content` (real FS, was `got 4`) | 1 passed, final content `v6` |
| `five_edits_within_the_window_produce_exactly_one_reload` (the plan's literal AC) | 1 passed, `reloads == 1` |
| `debounce_*` deterministic unit tests (deadline, stale claim, single fire, shutdown) | 4 passed |
| `config_watch_path_uses_config_path_override` | 1 passed |
| `reload_config_*` + `watcher_new_*` | 5 passed |
| `tests/e2e_serve_hot_reload` (real serve, real socket) | 1 passed |
| `cargo test --lib` | 571 passed, 0 failed, 0 ignored |
| `rustfmt --check` on all changed files; `cargo clippy --lib --tests` | clean |

The filesystem-timing tests live at the integration tier
(`tests/config_watcher_hot_reload.rs`) beside the deterministic `Debouncer` unit tests,
which keeps wall-clock flakiness out of the fast unit run.

#### 1.2b correction — the reload-*count* assertions were unsound and are removed

Both filesystem tests above pinned `reloads == 1`. They passed at commit time, but they
assert the host's scheduler rather than the contract, and they do not discriminate the
leading-edge implementation from the trailing-edge one.

The debouncer clocks **delivered** events; an external test only sees its own writes. A
temporary probe, since removed, measured write-to-callback latency `L` against the
*original* leading-edge watcher: **210, 219, 224, 281, 380 ms** over 5 samples at load
251-438, against a 200 ms window — `L >= DEBOUNCE` on every sample. When `L` is that
large the original leading-edge debouncer also reads the settled file, because a save made
after a fire has already left that window by the time its own event is delivered.

Four constructions of a discriminating filesystem test were tried — observe-then-save, a
100 ms offset, a single save inside an open window, and the original count assertion.
All four were green against the original code, 3/3 runs each.

**Action taken:**

- Removed both count-based filesystem assertions and kept one filesystem **receipt**,
  `burst_of_saves_reaches_the_live_config_with_final_content`: at least one reload,
  monotonic progress, final content observed. Its module docs state plainly that it is not
  a discriminator, and why.
- Moved the count requirement to the unit tier, where instants are injected and no wall
  clock is involved: `debounce::tests::five_events_within_the_window_yield_exactly_one_claim`
  — five events 40 ms apart yield exactly one claim, and the deadline follows the 5th.
- Narrowed visibility back after the import that needed it was removed: `mod debounce`
  and `pub(super) const DEBOUNCE`, since nothing outside the module reads either.
- Corrected module docs that named tests which no longer exist, and one intra-doc link to
  a private module.

Honest consequence: the *red* evidence for the debounce change is the `got 4` capture
above, taken against the original code. The committed tests cannot re-derive it, because
the original logic was an inline closure inside the notify callback with no seam to inject
a clock — the extraction is what made it testable at all.

**Green (observed) — current working tree:**

| Gate | Result |
|---|---|
| `cargo test --lib`, 2 consecutive runs | 572 passed, 0 failed, 0 ignored |
| `config_watcher` lib tests (5 debounce + 5 watcher) | 10 passed |
| `tests/config_watcher_hot_reload` (filesystem receipt) | 1 passed |
| `tests/e2e_serve_hot_reload` — `serve_hot_reload_follows_config_path_override` | 1 passed |
| `tests/e2e_serve_observability_order` — `serve_observability_records_auth_401_e2e` | 1 passed |
| `rustfmt --check` on all changed files | clean |
| `cargo clippy --all-targets` | 0 errors; 2 pre-existing warnings (`sharecli-ipc/src/queue.rs:164`, `commands/proc/tests.rs:1`) |

Line counts, all inside the 350-line target: `config_watcher.rs` 254, `debounce.rs` 196,
`tests/config_watcher_hot_reload.rs` 111, `tests/e2e_serve_hot_reload.rs` 149.

`runtime::tests::test_process_pool` failed once during this session (`src/runtime.rs:933`,
`process-pool spawn failed`) and then passed 3/3 in isolation and in both full-suite
re-runs. This branch does not touch `runtime.rs`; the test spawns `sleep 1` under load
459-487. Recorded as a pre-existing load-sensitive flake, not a regression.

### 1.3 — `#[serde(default)]` shadowing on partial tables

**Finding (lane 8 HIGH):** `#[serde(default)]` on the container `Config` only fills a
field whose *table is absent entirely*. A table that is present but partial is
deserialized by the child type, where serde's implicit rule for `Option<T>` fills a
missing key with `None` rather than that type's `Default`. A partial `[runtime]` table
therefore silently dropped `max_memory_mb` (4096) and `max_processes` (100).

The audit also called the shape "two-tier and arbitrary": eight of the fifteen table types
carried no struct-level `#[serde(default)]`, so the result depended on which table the
operator happened to edit.

**Red — 4 failed / 1 passed** against the original `src/config.rs`:

```
runtime_config_from_partial_toml_uses_defaults           FAILED  left: None  right: Some(4096)
partial_runtime_table_keeps_defaults_in_config            FAILED  left: None  right: Some(4096)
load_from_disk_of_a_partial_runtime_table_keeps_defaults  FAILED  left: None  right: Some(4096)
every_present_but_empty_table_fills_from_its_own_default  FAILED  missing field `enabled`
absent_tables_still_come_from_config_default              ok
```

The fourth failure is the sharper one: for tables whose fields are not `Option`, a
present-but-empty table did not merely lose its defaults, it **failed to parse**.

**Fix:** struct-level `#[serde(default)]` added to all eight — `RuntimeConfig`,
`PoolConfig`, `MonitoringConfig`, `PortConfig`, `PathsConfig`, `DefaultHarnessConfig`,
`ProjectLimitsConfig`, `SpawnConfig` — bringing all fifteen table types under one rule.
The policy is now documented in the `config` module doc: every table owns its defaults,
and the attribute is called out as load-bearing for `Option<T>` fields. `src/config.rs`
+25 / −0.

**Green:**

| Gate | Result |
|---|---|
| `tests/config_defaults` (was 4 failed / 1 passed) | 5 passed |
| `cargo test --lib` | 572 passed, 0 failed, 0 ignored |
| `fr002_config_load` / `fr002_config_init` | 3 passed / 2 passed |
| `config_live_write_guard` / `config_watcher_hot_reload` | 1 passed / 1 passed |
| `rustfmt --check` on changed files; `cargo clippy --all-targets` | clean, 0 errors |

The suite includes the disk-level receipt `load_from_disk_of_a_partial_runtime_table_keeps_defaults`,
which sets `SHARECLI_CONFIG_PATH` and drives the production `Config::load()` — the
TOML-driven path the audit flagged as never asserted, as distinct from the direct
constructor.

> Harness note: that receipt was first written with `#[serial_test::serial]` but **without
> `#[test]`**, so it compiled as dead code and did not run; the suite reported 4 tests
> rather than 5. Caught by counting the tests, not by a failure. Recorded rather than
> silently corrected.

**Size:** `src/config.rs` is 1195 lines, over the 500-line hard limit. This is
**pre-existing** (1170 before this change; this task added 25) and no task in this phase
covers it — splitting a 1195-line config module warrants its own plan.

## Verdict

Phases 0.5–0.7 are verified green on every gate that completed honestly, including the
per-BLOCKER helper test and end-to-end receipt. The two open Rust failures are
load-sensitive gates on code paths this branch does not touch, each traced to a mechanism
and to a `main` commit predating this work. The aggregate full-suite gate is UNKNOWN and
is reported as such rather than asserted.
