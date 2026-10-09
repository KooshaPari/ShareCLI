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

### 1.4 — Atomic config write with `.bak` recovery

**Finding (lane 8 MEDIUM-HIGH):** `Config::save` and `Config::init` used `std::fs::write`
— open, truncate, write-in-place. No temp+rename, no `fsync`, no `.bak`.

**Red — 3 failed / 1 passed** against the original `src/config.rs`:

```
save_keeps_the_previous_generation_as_a_backup           FAILED  save must keep a .bak of the previous config
load_recovers_from_backup_when_the_primary_is_missing    FAILED  left: None  right: Some("/rescued")
load_recovers_from_backup_when_the_primary_is_truncated  FAILED  load must succeed against .bak: TOML parse error at line 1, column 10
save_leaves_no_temporary_files_behind                    ok
```

The second failure is the one that matters most: with the primary file gone, `load()`
returned `Config::default()` — every registered project silently discarded with no error.

**Fix:** new `src/config_write.rs` (123 lines) implementing stage-in-same-directory →
`fsync` → preserve the outgoing generation as `.bak` (itself staged, then renamed, so an
interrupted backup cannot publish a half-written `.bak`) → atomic `rename` → directory
`fsync`. Every failure path removes the staging file, so a failed save leaves the previous
config and no debris. `Config::save` and `Config::init` now call `write_atomic`;
`Config::load` prefers the primary and recovers from `.bak` when the primary is missing or
unparsable, warning through `tracing`, and never invents a default when both are unusable.

The writer lives in its own module rather than inlined so `config.rs` does not grow
further; it is declared in `lib.rs` and `main.rs` alongside `config`.

**Green:**

| Gate | Result |
|---|---|
| `tests/config_atomic_write` (was 3 failed / 1 passed) | 4 passed |
| `cargo test --lib` | 572 passed, 0 failed, 0 ignored |
| config / project / session integration, 10 targets | 52 passed, 0 failed |
| `e2e_serve_hot_reload` / `e2e_serve_observability_order` | 1 passed / 1 passed |
| `rustfmt --check` on changed files | clean |
| `cargo clippy --all-targets` | 0 errors, same 2 pre-existing warnings |

Targets run: `config_atomic_write`, `config_defaults`, `config_watcher_hot_reload`,
`config_live_write_guard`, `fr002_config_init`, `fr002_config_load`,
`fr003_project_registry`, `c01_coverage_lift`, `c01_coverage_lift_wave18`, `session_cli`.

**Crash validation:** an in-process kill cannot be staged portably, so the tests construct
exactly the state a crash under the old scheme leaves behind — primary truncated or
absent, `.bak` holding the last good generation — and assert recovery. The atomicity
itself rests on `rename(2)` being atomic within one file system, which is precisely why
the staging file is created in the config's own directory rather than in a system temp
directory elsewhere.

> Harness note: the first implementation failed to compile — `.with_context()` was called
> on an unwrapped `io::Error` instead of on the `Result`. Caught before any test ran.

**Format:** `rustfmt` reports pre-existing drift in `src/commands/config_edit.rs`,
`src/config_validator.rs`, `src/util/mod.rs` and `src/main.rs` (lines 37, 437). None are
touched by this change; `src/main.rs` differs by exactly the one added `mod config_write;`
line. Left as-is rather than folded into an unrelated commit.

**Size:** `src/config.rs` is now 1231 lines (pre-existing over-limit: 1170 at branch start,
1195 after 1.3).

### 1.5 — `config.set` gained an `if_revision` guard (2026-10-01)

**Lane 3 HIGH.** `config.set` was a blind read-modify-write: the tray's ConfigPage
and the CLI patch the same file, so whichever wrote second silently clobbered the
first. A search of `src/` for `revision` returned nothing at all — the repo had no
concept of a config version, so the fix had to introduce one rather than wire up an
existing one.

**Design.** RFC 9110 `If-Match` semantics without the HTTP machinery: a new
`config.revision` method returns a fingerprint of the current config, and
`config.set` accepts an optional `if_revision` that must match it. The fingerprint
is a **content hash, not a counter** — SHA-256 over the canonical JSON encoding —
so it needs no persisted state, cannot drift across a sidecar restart, and any
client that serializes the config the same way can reproduce it and compare. A
mismatch reports `CONFLICT: ... current revision is <hex>`; the *winner's* revision
is what a client needs in order to refetch and retry rather than guess.

**The canonicalization is the whole trick.** `Config` stores its maps as
`std::collections::HashMap`, whose iteration order is randomized per process, so
serializing the struct directly would emit different bytes for the same config on
every run. Round-tripping through `serde_json::Value` first fixes that: this
workspace builds `serde_json` without `preserve_order` (verified by grepping every
`Cargo.toml`), so `serde_json::Map` is a `BTreeMap` and every object key is sorted
at every nesting depth — `Value` is canonical by construction, and `revision_of`
hashes `to_string(to_value(&config))`.

The guard is evaluated while the write lock is held, so check and mutation cannot
be interleaved by a concurrent writer. A non-string `if_revision` is a loud error
instead of a silently absent guard: a client that believes it is guarded must never
have its guard dropped.

**Red evidence** (every row measured against the original handler, with the fix
stashed via `git stash push -- crates/sharecli-ipc/src/handler.rs`):

| Test | Red against original | Observed message |
|---|---|---|
| `config_revision_is_exposed_and_stable_while_unchanged` | 4 failed / 0 passed (as a set) | `unknown method: config.revision` |
| `config_set_with_a_stale_if_revision_returns_conflict` | same run | `a stale if_revision must be rejected` |
| `config_set_rejects_a_non_string_if_revision` | 1 failed / 0 passed | `a non-string if_revision must be rejected` |

The stale-write assertion is deliberately built on a literal `"stale-revision"`
rather than on a value read through `config.revision`, so its red proves the
*guard* was missing and not merely that the read helper did not exist. An earlier
formulation of that test read the revision first and therefore proved only
`unknown method`; it was rewritten before the fix rather than reported as evidence.

**Green gates (observed 2026-10-01):**

| Gate | Result |
|---|---|
| `cargo test -p sharecli-ipc` (6 targets) | **168 passed / 0 failed / 2 ignored** (`A_exit=0`) |
| `sharecli-ipc` unit tier, `config_revision::tests` | **5 passed / 0 failed** |
| `tests/config_revision_dispatch.rs` | **5 passed / 0 failed** (red 0/4 → green 5/0) |
| `cargo test -p sharecli --test config_defaults --test config_atomic_write` | **9 passed / 0 failed** (`B_exit=0`) |
| `cargo test -p sharecli --lib` | **572 passed / 0 failed** |
| `cargo clippy --all-targets` | **exit 0**, only the 2 pre-existing warnings (`sharecli-ipc/src/queue.rs:164`, `commands/proc/tests.rs:1`) |
| `rustfmt --check` | clean on `config_revision.rs`, the new test, `cache_key.rs`; `handler.rs` shows **the same 3 hunks as `HEAD`** (verified by formatting the `HEAD` blob in-tree), so this change adds none |

**Harness notes:**

* The dispatch tests mutate `SHARECLI_CONFIG_PATH`, which is process-wide. Run in
  parallel they tripped over each other — one fixture's temp directory was removed
  while another still pointed at it, surfacing as `replace .../config.toml` inside
  a `config.set` error rather than as an obvious race. All five are marked
  `#[serial_test::serial]`, the pattern already used elsewhere for env mutation.
* `cargo test` intermittently prints `error: duplicate key` pointing at
  `~/.cargo/git/checkouts/phenoshared-.../agileplus-cache/Cargo.toml:15`. It is
  **outside this repository** and non-fatal (`A_exit=0`, `step2_exit=0`), but it is
  reached through the unpinned `substrate = { git = "https://github.com/KooshaPari/PhenoShared" }`
  dependency in the root `Cargo.toml`, which has no `rev`. Not introduced by this
  branch and not fixed here; recorded because an unpinned git dependency whose
  workspace carries a malformed manifest is a reproducibility risk.
* The host's data volume hit 0 bytes free during a `clippy --all-targets` run
  (`failed to write ... dep-graph.part.bin: No space left on device`), which is why
  the gate had to be re-run. `target/debug/incremental` (6.7 GB, regenerable) was
  dropped to recover ~5.4 GB; free space is 15 GB of 926 GB at time of writing.

**Size:** `crates/sharecli-ipc/src/handler.rs` goes 1205 → 1243 lines — still over
the 500-line target, pre-existing and unchanged in kind. The new
`config_revision.rs` is 115 lines and the new test file 150.

**Side change:** `hex = "0.4"` was added to `sharecli-ipc`; `cache_key.rs` already
carried a private 4-line `mod hex` doing the same lowercase encoding, which was
removed so the crate has one implementation instead of two.

**Not in scope:** the Swift tray still calls `config.set` without `if_revision`, so
it keeps last-writer-wins behaviour until `IPCClient`/`ConfigPage` adopt
`config.revision`. The IPC surface for that now exists and is tested.

## Phase 1 task 1.6a — capped IPC line length (2026-10-01)

**Finding:** `FINDINGS.md:58` `[HIGH] main.rs:96-107,116-127` — "`BufReader::lines()` with
no max line length; no request-size bound, no response-size bound — local memory DoS."
Prescribed fix: `tokio_util::codec::LinesCodec::new_with_max_length(N)`.

**Observed before:** `main.rs` carried two byte-identical connection loops
(`serve_unix_connection`, `serve_tcp_connection`), both built on `BufReader::lines()`.
`lines()` yields a record only on `\n`, so a local peer that connects and streams bytes
with no newline grows the sidecar's heap without bound for as long as it keeps writing.
`tokio-util` was already declared in `Cargo.toml` with `features = ["codec"]` and was
referenced nowhere — the declared fix had never been applied.

### Red — real binary, unmodified server

`crates/sharecli-ipc/tests/ipc_line_limit.rs` spawns the **real** `sharecli-ipc` binary
(`CARGO_BIN_EXE_sharecli-ipc`), not a mocked reader:

| test | result against original `main.rs` |
|---|---|
| `a_request_just_under_the_cap_is_answered` | pass |
| `a_huge_frame_is_refused_but_the_connection_still_serves` | pass |
| `oversized_frame_is_rejected_without_a_response` | **fail** |

The failure is the defect itself: the original server buffered all 1 048 576 bytes of the
oversized frame and then *answered* with an 80-byte JSON body
(`parse error: expected value at line 1 column 1`) instead of refusing the frame. No
response is the required behaviour — a frame over the cap must never reach the handler.
**Red: 2 passed / 1 failed.**

The three codec-level unit tests in `framing.rs` cannot produce red against the original,
because `framing.rs` did not exist; the integration test above carries the red for the
behaviour, and the unit tests pin the mechanism.

### Fix

New `crates/sharecli-ipc/src/framing.rs` (116 lines):

* `pub const MAX_REQUEST_LINE_BYTES: usize = 256 * 1024` — far above the largest frame on
  the wire (`process.spawn` carries only `name`/`command`/`args`/`project`/`harness`/`cwd`)
  and far below the 1 MiB frame the test uses to exercise the cap.
* `serve_framed<R: AsyncRead, W: AsyncWrite>(reader, writer, handler)` wraps the split
  halves in `Framed::new(.., LinesCodec::new_with_max_length(MAX_REQUEST_LINE_BYTES))` and
  dispatches each frame. `line?` propagates `MaxLineLengthExceeded` out of the loop to the
  accept loop, which logs it and closes **that one connection** — the bound is on a frame,
  not on a peer; every other client keeps serving.
* `serve_unix_connection` and `serve_tcp_connection` in `main.rs` are now `into_split()` +
  a delegate call. This removed the duplicated loop outright: `main.rs` 282 → **256**
  lines. Modules declared in both `lib.rs` (`pub mod framing;`) and `main.rs` (`mod framing;`).

**Response size, and why it is not separately bounded:** responses are derived from
dispatched handler state plus request params — a peer does not stream bytes into them. With
the request cap in place, any response that echoes request content is itself bounded by
256 KiB plus encoding overhead, so the unbounded-peer-write vector named in the finding is
closed on the request side. No separate response cap was added, and none is claimed.

### Green gates

| gate | observed |
|---|---|
| `ipc_line_limit` (real-binary integration) | **3 passed / 0 failed** |
| `framing` unit (in `sharecli_ipc` lib target) | **3 passed / 0 failed** |
| `cargo test -p sharecli-ipc` (7 targets) | **177 passed / 0 failed / 2 ignored**, `A2_exit=0` — run twice, identical |
| `cargo clippy --all-targets` | `B_exit=0`; exactly the 2 pre-existing warnings (`commands/proc/tests.rs` module-name, `queue.rs:164` `saturating_add`) |
| `rustfmt --check` | clean on `framing.rs`, `ipc_line_limit.rs`; `main.rs` contributes **0** own hunks (the 3 reported are the pre-existing `handler.rs` ones, reached by rustfmt's module recursion) |

Target breakdown of the 177: lib 104 · main 47 · `c01_climb2_ipc` 4 · `config_revision_dispatch`
5 · `handler_dispatch` 13 · `ipc_line_limit` 3 · doc-tests 1. New in this task: **+6**
(3 framing unit + 3 integration).

**Clippy correction during the gate:** the first full run flagged one warning I had
introduced — `unused import: LinesCodecError` at `framing.rs:16`, because `LinesCodecError`
is named only inside `#[cfg(test)] mod tests`. `B_exit` was still 0 (warnings do not fail
the gate), but the accepted baseline is *exactly the 2 pre-existing warnings*, so the import
was scoped into the test module and the whole gate re-run rather than shipped.

### Harness notes

* Isolation is **child-only env** — `SHARECLI_IPC_SOCK`, `SHARECLI_CONFIG_PATH`,
  `SHARECLI_SESSION_DB` are passed with `Command::env(...)`. No `std::env::set_var`, so the
  test cannot mutate process state for its siblings.
* The `Sidecar` fixture declares `child` **before** the temp directory so Swift's
  declaration-order drop kills the server before the directory it lives in disappears.
* `read_frame` and the oversized-frame write both tolerate the post-fix failure shape: the
  server now hangs up mid-write, so `write_all` can surface `BrokenPipe`. The test accepts
  `write_refused || hung_up` and maps `BrokenPipe`/`ConnectionReset`/`NotConnected` in the
  reader — both are the same refusal, reached at different moments.

**Not in scope:** 1.6b — a single long-lived Swift connection with an id→continuation map.
That item originates at `PLAN.md:165`, **not** in `FINDINGS.md`; `IPCClient.swift` has no
per-call-connection finding, and its per-call design is the documented thread-safety
mechanism for a 754-line client covered by 5 Swift test files. It is staged second and
reported separately.

## Phase 1 task 1.6b — one long-lived Swift IPC connection (2026-10-01)

**Provenance, stated plainly.** The item is `PLAN.md:162-166`: "`IPCClient.swift` —
single long-lived connection with id→continuation map and per-request cancellation.
Validate by feeding a 1 MB line; second connection attempt rejected." It is **not** a
`FINDINGS.md` defect; `FINDINGS.md` has no per-call-connection entry, and the per-call
design was the client's documented thread-safety mechanism ("each call creates its own
socket connection"). What the same rewrite *does* close is a real, still-open finding:
`FINDINGS.md:61` `[HIGH]` — `while true { Darwin.read(fd, &byte, 1) }`, one `read(2)`
per byte and 10⁵+ syscalls per 1 Hz `monitoring.report`.

**Before:** `IPCClient.call` opened a socket, wrote, read byte-at-a-time to `\n`, closed —
once per call. `IPCClient.swift` was 754 lines.

### Red — against the original client

`IPCClientConnectionReuseTests.testThreeCallsRideOneConnection` talks to **`IPCClient`**,
not to the new type, precisely so it still compiles and runs against pre-refactor code.
It was executed against a package copy rebuilt from `HEAD` (`IPCClient.swift` restored
via `git show HEAD:…`, `IPCConnection.swift`/`IPCConnectionTests.swift` excluded):

```
XCTAssertEqual failed: ("3") is not equal to ("1")
- three calls must reuse one connection; the per-call client accepts once per call
RED_EXIT=1
```

**Red: 1 test, 1 failure — three calls, three accepts.** The same test in the working
tree passes with `acceptCount == 1`.

### Design

New `Sources/ShareCLICore/IPCConnection.swift` (313 lines):

* **One serial `DispatchQueue` owns the fd.** Reader and every write run on it, so there
  is one close path and no operation is inside a syscall on an fd another path is
  closing. `pending` is only touched on that queue and needs no lock.
* **id → `CheckedContinuation<Data, Error>`.** The map holds *raw* reply lines, not a
  typed value, which is what lets one map serve every `T` the caller asks for; decoding
  stays in `IPCClient.call`. Concurrent calls therefore stay concurrent instead of
  queueing behind one socket.
* **Per-request cancellation.** Each `send` schedules its own timeout with
  `queue.asyncAfter`. A late reply finds no entry and is dropped rather than delivered
  to an unrelated caller. The timer captures `self`, so a continuation can never be
  dropped along with its owner — every registered continuation is guaranteed to resume.
* **Buffered read (closes `FINDINGS.md:61`).** The fd is non-blocking and drained
  64 KiB at a time.
* **`connect()` refuses a second attempt** with the new `IPCError.alreadyConnected` —
  the plan's "second connection attempt rejected". Normal traffic goes through `send`,
  which reuses the live connection.
* **`SO_NOSIGPIPE`** is set on the fd. The old path had no protection: writing to a
  peer that had already gone would raise SIGPIPE and terminate the tray. A connection
  that now outlives sidecar restarts meets that case far more often.
* **Reader is a 10 ms `DispatchSourceTimer`, not a source bound to the fd.** An
  fd-backed source signalled against an already-closed descriptor is the classic crash;
  a timer cannot be. Cost: one `read(2)` returning `EAGAIN` per tick while idle.
  Benefit, and the reason it runs unconditionally: a sidecar restart is noticed within
  10 ms, so the next call reconnects instead of writing into a dead socket — the
  property the per-call connection had for free.

**Removed with the per-call socket:** `probeSlots` / `maxConcurrentProbes` (cap 64) and
`openUnixSocket`. The semaphore existed to stop *blocking closures on the global utility
queue* from wedging it; the new path issues no blocking closure on that pool at all, so
keeping it would be a shim. `IPCClient` 754 → **640** lines; its header comment, which
documented the per-call design as the thread-safety mechanism, was rewritten to match.

### Green gates

| gate | observed |
|---|---|
| `swift build --package-path desktop/ShareCLITray` | exit 0, whole package incl. the tray app |
| `swift test` (no sidecar) | **33 executed / 0 failures / 7 UDS-gated skipped**, exit 0 |
| `swift test` **with a live isolated `sharecli-ipc`** | **33 executed / 0 failures / 0 skipped**, exit 0 |
| baseline before this task | 26 executed / 0 failures / 7 skipped, exit 0 |

The live run is the end-to-end receipt: `ConfigClientTests` and
`IPCClientRealPathTests` stop skipping and pass against the real Rust sidecar,
including `test_real_sidecar_spawn_then_kill_round_trip`, which kills the sidecar
mid-session — the case a long-lived connection has to survive. Existing coverage is
unbroken: the audit-0.7 suite (`IPCClientDeadlineTests`, 70 concurrent wedged probes
all resolving `.timeout` and the pool serving follow-up work afterwards) passes
unchanged against the new connection.

**Six new tests** in `IPCConnectionTests`: second `connect()` rejected; five calls on one
accept; a 300 KiB reply delivered intact; a timeout cancelling one request while the same
connection keeps serving; eight concurrent requests each cancelling on their own deadline;
and replies routing by id so an unanswered request never inherits another's reply.

**The 1 MB line** from the plan's validation clause is a *server* contract and is
exercised end-to-end by the Rust `ipc_line_limit` suite against the real binary
(1.6a above), not re-tested on the Swift side.

### Harness note — a bug in my own test server, found and fixed

Three of the six tests failed on the first run. The cause was in `LineSidecarServer`,
not in the product: the listener is `O_NONBLOCK` so its accept loop can poll for
`stopped`, and the accepted socket came back non-blocking too, so `serve`'s
`if n <= 0 { return }` treated `EAGAIN` as end-of-stream and exited **after the first
request** — every later request on that connection went unanswered. That reproduces
exactly what was observed (`request 2 never got a reply`, while the single-request
large-response test passed). Fixed by normalising the accepted fd back to blocking
*and* handling `EAGAIN`/`EINTR` explicitly. The existing `HealthyConfigServer` in
`IPCClientDeadlineTests` has the same shape and never noticed because it only ever
answers one request per connection.

Two smaller test fixes were also required: `connect()` returns from the kernel handshake
before the accept loop has dequeued it, so the accept count needs a bounded wait
(`waitUntilAccepted`); and one `Data("\(body)\n").utf8` argument-order slip failed to
compile.

### 1.7 — Effective cadence in Swift polling (lane 2) (2026-10-02)

**Provenance.** `PLAN.md:169-171` — "### 1.7 Effective cadence in Swift polling
(lane 2)", pinning `AppState.swift:273-278` — deadline loop on `ContinuousClock`.

**Problem.** The Swift tray's poll loop slept for `interval` *after each poll finished*,
so the effective period was `interval + work` and the tray drifted past its configured
cadence under load. The fix schedules each poll from the previous poll's *scheduled*
start, so the effective period equals the interval.

**Code change.** NEW `desktop/ShareCLITray/Sources/ShareCLICore/PollLoop.swift`:
`PollCadence.deadline(previous:interval:now:)` (pure) plus a `PollLoop` struct with an
injected `PollClock`; the loop schedules from the previous poll's SCHEDULED start, so
effective period == interval, not interval + work. `AppState.startPolling()`
(AppState.swift:266-275) now builds `PollLoop(interval: .seconds(TrayPoll.intervalSeconds))`
at AppState.swift:269 and runs it in `pollTask`. `TrayPoll.swift`: dead
`intervalNanoseconds` removed; the contract now asserts `intervalSeconds`.

#### Red evidence

Captured twice with the final shipped params, both exit 1:

```
wall-clock cadence test 1: observed 0.924 s where the effective-cadence regime
  requires <= 0.700 s (the broken fixed-loop value is ~0.900 s + jitter) -> RED_EXIT=1
wall-clock cadence test 2: observed 1.616 s where <= 1.300 s is required
  (broken value ~1.600 s + jitter) -> RED_EXIT=1
```

3 pure-deadline tests + 2 deterministic manual-clock tests (`ManualClockState`, bounded
by `maxSleeps`) were also added in `PollLoopTests.swift` (~300 lines). The thresholds
are the MIDPOINT between the two regimes; they were widened after a first red run took
136 s under host load — that measurement is the reason the thresholds are sound.

#### Green gates (observed pre-directive)

| gate | observed |
|---|---|
| `swift test --package-path desktop/ShareCLITray` (no sidecar) | **42 executed / 0 failures / 7 skipped**, exit 0 |
| baseline before this task | 33 executed / 0 failures / 7 skipped |

#### Contract interaction (observed, red then fixed)

`tests/fr007_tray_swift_poll_interval.rs:26` asserted `AppState` contains
`TrayPoll.intervalNanoseconds`; the refactor made that symbol dead, so the contract went
RED (exit 101), captured. The assertion was aligned to `TrayPoll.intervalSeconds`,
mirroring the Windows sibling contract pattern — this strengthens the gate rather than
weakening it. fr007 then green 2/2. `docs/specs/TRACEABILITY.md` AC-007.53 evidence row
updated (commit 8101160e).

#### L1 rust gate — observed directly (2026-10-02)

The background fr007 suite log (`.jcode/scratch/fr007_all.log`) ran the target; verbatim:

```
     Running tests/fr007_tray_swift_poll_interval.rs (target/debug/deps/fr007_tray_swift_poll_interval-5e3277444650a1a1)

running 2 tests
test fr007_tray_swift_poll_interval_seconds ... ok
test fr007_tray_swift_poll_interval_wires_app_state ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

2 executed / 2 passed / 0 failed, exit 0. The block was first observed in the log at
22:33:07 local on 2026-10-02 (the log carries no per-line timestamps; the target's own
reported finish time is `finished in 0.00s` above; the suite's final log write was
22:33:39 local).

**Suite-level totals at that moment — host-load observations, explicitly NOT evidence
about task 1.7's own change:** 78 `test result:` lines (all 78 fr007 targets ran); 9
carried `test result: FAILED`, and cargo's closing summary reads `error: 9 targets
failed:` — `fr007_proc_csv_watch`, `fr007_proc_text_pool_status`,
`fr007_proc_tree_watch_stderr_footer`, `fr007_proc_watch_text_stderr_silent`,
`fr007_ps_all_watch_json_gate_host_watch`, `fr007_ps_all_watch_text_stderr_silent`,
`fr007_report_json_pool_status`, `fr007_status_watch_json_gate_host_watch`,
`fr007_status_watch_text_stderr_silent`. The failing test names were
`fr007_proc_tree_csv_watch_stderr_silent_and_envelope`,
`fr007_proc_tree_watch_text_pool_status_order`,
`fr007_proc_tree_watch_ndjson_stdout_no_companion_leak`,
`fr007_proc_tree_watch_text_stderr_silent`, `fr007_ps_all_watch_ndjson_gate_ordering`,
`fr007_ps_all_watch_text_stderr_silent`,
`fr007_report_watch_ndjson_pool_status_ordering`,
`fr007_status_watch_ndjson_gate_ordering`, `fr007_status_watch_text_stderr_silent`.
Load averages observed at completion: `519.18 489.32 450.40` (22:34 local), with earlier
readings `521.11 404.09 323.75` (22:14) and `521.68 530.48 440.06` (22:25). None of the
9 failing targets is `fr007_tray_swift_poll_interval`, which passed.

#### L2 live-sidecar e2e receipts (observed, agent "hamster", 2026-10-02)

* filter run `AppStatePollTests` against a real isolated sidecar: Executed 1, 0
  failures, 0 skipped, exit 0, PASSED in 4.325 s (not skipped).
* full suite with ONLY `SHARECLI_IPC_SOCK` exported: "Executed 43 tests, with 7 tests
  skipped and 0 failures (0 unexpected)", exit 0. The 7 skips are `ConfigClientTests`
  x4 + `IPCClientRealPathTests` x3, each saying "SHARECLI_TEST_IPC_SOCK not set".
* POST-SEQUENCE diagnostic with BOTH `SHARECLI_IPC_SOCK` and `SHARECLI_TEST_IPC_SOCK`
  exported: Executed 43 / 0 failures / 0 skipped (`skip_line_count=0`). This is the
  end-to-end receipt: 43 executed / 0 skipped, i.e. the 1.6b receipt (33/0/0) grown by
  the 10 new 1.7 tests.
* cleanup verified: both sidecar PIDs dead (`kill -0` fails), no strays, no
  `/tmp/sharecli-l2-*.sock`, default path restored absent.

#### Consistency sweep (grep evidence)

* `intervalNanoseconds` → 0 references repo-wide after the change.
* `Task.sleep` → 0 occurrences inside `AppState.swift` (the loop now lives in `PollLoop`).
* PollLoop wired: `AppState.swift:269`.

## Verdict

Phases 0.5–0.7 are verified green on every gate that completed honestly, including the
per-BLOCKER helper test and end-to-end receipt. The two open Rust failures are
load-sensitive gates on code paths this branch does not touch, each traced to a mechanism
and to a `main` commit predating this work. The aggregate full-suite gate is UNKNOWN and
is reported as such rather than asserted.
