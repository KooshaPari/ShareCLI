# Audit Findings — 2026-09-20

> 10 lanes dispatched in parallel, all read-only, all empirical where possible.
> Repo left untouched at `2ccfa7ab`. Each entry: `file:line | severity | issue | fix`.
> Severity legend: **BLOCKER** (data loss / silent bad effect) > **HIGH** (real-world
> correctness / observability / UX bug) > **MEDIUM** (robustness gap) > **LOW** > **POLISH**.

---

## Lane 1 — Rust async / lock / hot-reload (`squid`)

**17 findings, top severities HIGH.** Lane report: `session_squid_1789891956247_13479ef994ab7e83`.

- **[HIGH]** `src/commands/serve.rs:256-258` — layer order inverted; auth 401s and rate-limit 429s never reach RED-metrics / traceparent middleware.
  - **Fix:** reorder `.layer(from_fn_with_state(...))` so observability is outermost (axum runs layers bottom-up at registration, top-down at request time).
- **[HIGH]** `src/commands/serve.rs:470-520` — `handle_ws` has no cancellation token, so `with_graceful_shutdown` waits on WS clients forever (Ctrl-C hang).
  - **Fix:** take `CancellationToken`; on shutdown, `ws.send(Close)` then `tokio::time::timeout(2s, ws.close())`.
- **[HIGH]** `src/health_check.rs:155-191` — health-store mutex held across `notifier.dispatch().await` (webhook POSTs).
  - **Fix:** clone the needed state, drop the lock, then `await` the webhook; or move webhook to a bounded mpsc worker.
- **[HIGH]** `src/commands/serve.rs:552-553, 674-679` — per-request `ProcessPool::new()`: `System::new_all()` blocking scan on the runtime every 500 ms per WS client; `list()` always empty so `sharecli_process_*` metrics silently export nothing.
  - **Fix:** hoist `ProcessPool` into `AppState` and reuse; expose it as `Arc<ProcessPool>`.
- **[HIGH]** `src/commands/serve.rs:159-165` — watcher path ignores `SHARECLI_CONFIG_PATH`, so hot-reload silently never fires in that config.
  - **Fix:** resolve watch path through the same helper as `Config::load`.
- **[HIGH]** `src/config_watcher.rs:61-73` — leading-edge debounce drops the final save in a burst, leaving stale config with no warning.
  - **Fix:** trailing debouncer (`notify-debouncer-full`); filter on `event.paths`.

---

## Lane 2 — Swift concurrency (`turkey`)

**12 findings: 1 BLOCKER, 3 HIGH, 6 MEDIUM, 3 LOW.** Compiled against `-swift-version 6`. Lane report: `session_turkey_1789891974660_389f33fb4c8ebf61`.

- **[BLOCKER]** `desktop/ShareCLITray/Sources/ShareCLICore/IPCClient.swift:600-605` — `while true { Darwin.read(fd, &byte, 1) }` with no `SO_RCVTIMEO` / `SO_SNDTIMEO` and no overall deadline.
  - **Measured:** `DispatchQueue.global(qos: .utility)` blocking-closure ceiling = 64. A wedged sidecar saturates the pool in ~5 min; after that supervisor recovery never runs again.
  - **Fix:** `setsockopt(SO_RCVTIMEO/SO_SNDTIMEO)`; cap concurrent probes; `poll(2)` + DispatchSourceRead with deadline; reference `connectOnce` at `SidecarSocketProbe.swift:41-42`.
- **[HIGH]** `desktop/ShareCLITray/Sources/ShareCLICore/AppState.swift:273-278` — cadence drift: loops `await work()` then `sleep(interval)`, so effective period = `interval + work`. Wall-clock `Date()` stamps (`:322,378`) skew sparklines.
  - **Fix:** deadline loop on `ContinuousClock` (SE-0329) — `next += .seconds(3); try await clock.sleep(until: next)`.
- **[HIGH]** `desktop/ShareCLITray/Sources/ShareCLICore/AppState.swift:281-294`, `SidecarSupervisor.swift:83-89` — `pollTask?.cancel()` then spawn replacement without `await pollTask?.value`; cancel is not a join. Two refreshes interleave at await points.
  - **Fix:** join before restart (`await pollTask?.value`); wire `withTaskCancellationHandler` into IPC `call`; close fd on cancel; check `Task.isCancelled` inside read loop.
- **[HIGH]** `SidecarSupervisor.swift:130-132` — `guard !isTicking else { return }` + `defer { isTicking = false }` turns any hung `tick()` into permanent supervisor death; concurrent `ensureRunning()` then returns stale state.
  - **Fix:** bound `tick()` with deadline + reset on exit; replace boolean with a generation counter and "in flight longer than N×interval ⇒ wedged" detection.

---

## Lane 3 — IPC protocol design (`whale`)

**Phantom-method BLOCKERs + an auth/authz BLOCKER + many HIGH.** Lane report: `session_whale_1789891976464_86447ec9f827e49b`.

- **[BLOCKER]** `crates/sharecli-ipc/src/main.rs:50-58,133-139` — no `chmod 0600` on the socket (observed `srwxr-xr-x`); no peer-credential check anywhere in the repo.
  - **Fix:** `set_permissions(0o600)`; verify peer uid == euid via `SO_PEERCRED` / `getpeereid`; reject otherwise. Any local user can call `process.kill_all`.
- **[BLOCKER]** `IPCClient.swift:479-487` + `AppState.swift:307` — `pool.effectiveness` is not in the router; `AppState` swallows the error with `try?` so Pool Effectiveness page is empty forever.
  - **Fix:** implement the method, or delete client method and page.
- **[BLOCKER]** `IPCClient.swift:540-553` + `ProcessesPage.swift:1684` — `process.spawn` unimplemented server-side; user-facing Spawn form always errors.
  - **Fix:** implement `process.spawn` (server-assigned pid is fine) or remove the form.
- **[HIGH]** `handler.rs:53-76` — envelope has no `version`/`protocol` field; no `ipc.hello`/capabilities method; tray↔sidecar skew undetectable.
  - **Fix:** add `ipc.hello` → `{schema_version, capabilities:[methods], server_version}`; echo `version` in every response; reference JSON-RPC 2.0 / MCP lifecycle.
- **[HIGH]** `main.rs:96-107,116-127` — `BufReader::lines()` with no max line length; no request-size bound, no response-size bound — local memory DoS.
  - **Fix:** `tokio_util::codec::LinesCodec::new_with_max_length(N)` over `FramedRead`.
- **[HIGH]** `handler.rs:560-566` + `handler.rs:634-653` — `config.set` is a blind write; tray ↔ CLI write race silently clobbers.
  - **Fix:** expose `config.revision`; accept `if_revision`; return `CONFLICT` with current revision; patch only changed key. (RFC 9110 If-Match.)
- **[HIGH]** `IPCClient.swift:597-605` — one `read()` syscall per byte; 10⁵+ syscalls per 1 Hz `monitoring.report`.
  - **Fix:** buffered `readData(ofLength: 64*1024)` or `NWConnection.receive`.
- **[HIGH]** `serve.rs:148,511-513` + `serve.rs:318-321` — broadcast Lagged is `warn! + continue`; thermal events silently dropped for slow client; 200 ms shutdown grace so lagged client misses `thermal_critical`.
  - **Fix:** keep last level in `watch` channel + re-send on Lagged, or disconnect with "resync required" frame.
- **[HIGH]** `serve.rs:471-509` — 500 ms `interval` uses `MissedTickBehavior::Burst`; slow send → catch-up burst.
  - **Fix:** `MissedTickBehavior::Delay` (or `Skip`).
- **[HIGH]** `handler.rs:612-619` + `log_buffer.rs:95-105` — `log.tail` truncates to 200 lines but returns global `last_id` → silent permanent log loss; spec bug.
  - **Fix:** return `next_since`, `first_available_id`, `dropped` so gaps are explicit.
- **[HIGH]** `handler.rs:408-412` — parse failure returns `id: 0`, not `null` + `-32700`; undecodable requests are indistinguishable from successful replies.
- **[HIGH]** `handler.rs:408-418` + `log_buffer.rs:69-89` — request `id` is never logged; log.tail cannot be used for request tracing.
  - **Fix:** `info!(id, method, elapsed_ms, ok)` per dispatch.
- **[HIGH]** `handler.rs:68-71` — `Response::ok` swallows serialization failure (`unwrap_or(Value::Null)`) → `result:null, error:null`; `listProcesses()` maps null → `[]`; serialization bug renders as "no processes".

---

## Lane 4 — CLI ergonomics / error UX (`parrot`)

**20+ findings; 1 BLOCKER, 6+ HIGH.** Lane report: `session_parrot_1789891986367_5f01f416657d3e44`.

- **[BLOCKER]** `src/commands/mod.rs:646-649` — `stop --pid 999999` exits 0 and prints "Process 999999 stopped." for a PID that does not exist.
  - **Fix:** inspect `pool.kill()`'s `Result`; distinct message and exit for `pid_not_found`.
- **[HIGH]** ~40 `anyhow::bail!` sites classified as "internal" → exit 1.
  - **Fix:** exit-code `UserInput → 64`, `NotFound → 2`, `Internal → 1`; `--theme` already gets this right (exit 64) — apply everywhere.
- **[HIGH]** Every error prints **twice** (`error: X` then `caused by: X`).
  - **Fix:** print only the top frame + a `↳ caused by X (debug-only)` block under `RUST_BACKTRACE=1`.
- **[HIGH]** `main.rs:1057-1061` — `serve --on-conflict <typo>` silently degrades to `Abort`.
  - **Fix:** clap `value_enum` with `RenameAll = "kebab"` and reject unknowns loudly.
- **[HIGH]** `upgrade --channel beta` documented but rejects every channel.
  - **Fix:** wire channels; surface "stable|beta|nightly" accept-set in help.
- **[HIGH]** `upgrade --check` documented but doesn't exist.
  - **Fix:** implement or remove the doc line.
- **[MEDIUM]** `ps --json` requires `--all` but `proc --json` does not; precondition only in error text, never in `--help`.
- **[MEDIUM]** `src/ansi.rs` — 11 functions emit `ESC }` instead of `ESC [`; unit tests assert the broken bytes.
- **[DEAD CODE]** `src/commands/history.rs` (236 lines) and `src/api.rs` unreachable.

---

## Lane 5 — Observability (`rabbit`)

**21 findings; 1 BLOCKER, 8 HIGH.** Lane report: `session_rabbit_1789891983199_a39014baa9a107a6`.

- **[BLOCKER]** `src/commands/serve.rs:256-258` — layer order puts auth outermost → 401s/429s never reach `HttpRedMetrics::record`; `sharecli_http_unauthorized_total` always 0; SLO-4 auth-burn alerts are dead.
- **[HIGH]** `docs/ops/alertmanager/sharecli.yml:78-82` — `histogram_quantile(0.99) > 500` with top bucket `le="100"` can never fire.
- **[HIGH]** `serve.rs:584-604` — `process` label is non-unique (pool is PID-keyed) → duplicate series; scrape rejected during incidents.
- **[HIGH]** `serve.rs:551-555` + `runtime.rs:173-227` — two blocking `lsof` subprocesses per process per scrape inside async handler, uncached.
- **[HIGH]** `audit_log.rs:30-44` — audit is opt-in; spawn/stop/denials and thermal self-shutdown unaudited by default.
- **[HIGH]** `main.rs:906,915` — no tracing subscriber when stderr is not a TTY; daemonized `sharecli serve` emits zero logs.
- **[HIGH]** `serve.rs:344-366` — `traceparent` stored as attribute, not parent span; synthesized response header does not match exported span.
- **[HIGH]** `serve.rs:159-165` — health-store mutex held across `notifier.dispatch().await`.

---

## Lane 6 — FUSE correctness (`swan`)

**BLOCKER-class defects + multiple HIGH.** Lane report: `session_swan_1789891961503_b30a3973c9f7d4c2`. Verified via scratch probe + 128 unit tests.

- **[HIGH]** `crates/sharecli-fuse/src/lib.rs:971` (`rename_rel`) — after a rename, both inodes 2 and 3 resolve to `b.txt`. Duplicate `ino→path` mapping. Probe-confirmed.
- **[HIGH]** `intercept::rename` (`lib.rs:962-970`) + `read_cache.rs` — directory rename only invalidates the renamed paths; descendants keep `ino→old-path` mapping and stale cache entries.
- **[HIGH]** `lib.rs:912` — `unlink` removes `map.remove_rel(&rel)`; open fds lose the path and then ENOENT on read. POSIX violation.
- **[HIGH]** `lib.rs` (setattr) — no path lock, concurrent `write` + `truncate` → torn content.
- **[HIGH]** `write_serialize.rs:91-96` — staging filename = `DefaultHasher(abs_path)` over 64-bit space, shared by all backing paths → CoW corruption from collision.
- **[HIGH]** `write_serialize.rs:149-152` — `fs::copy` + `remove_file` fallback is **not atomic**; if backing root is on a different volume this is the **normal path**, exposing a partial/zero-length window.
- **[HIGH]** `crates/sharecli-fuse/src/neg_dentry.rs` — no cap, no LRU, no sweeper. Probe: 500k remember_miss → +107 MB RSS and it never shrinks; deletes insert negative entries that are never re-probed → permanent junk.
- **[HIGH]** `lib.rs:702-705` (and others) — `set_deref` on macOS = symlink-follow; provenance xattr setting failure converts a landed write into EIO.
- **[HIGH]** `neg_dentry.rs:113-126` + `lib.rs:909-911` — `unlink`/`rmdir` insert negative entries for the just-deleted path; memory grows.
- **[HIGH]** `provenance.rs` — on xattr-less filesystems (exFAT/FAT32, SMB, many NFS) every write fails with EIO while `read_provenance` returns Ok(None) — false clean signal.
- **[HIGH]** Missing ops: no `readlink`/`symlink`/`link` (`node_modules/.bin/*` all symlinks) ⇒ stated caching objective defeated; no `flush`/`fsync`/`release`.
- **[HIGH]** `lib.rs` — `read` does the opposite of coalescing (loops over fs reads).
- **[HIGH]** `read_cache.rs:88-101` — cache key `(path, mtime)` only; on 1s-granularity mtimes stale hit is possible; no ctime/inode/size.

---

## Lane 7 — Hypervisor / CoalesceCache / SlotQueue (`tiger`)

**7 findings, 3 are BLOCKER-class.** Lane report: `session_tiger_1789891970229_a5e058b74619ccf9`. Empirically probed the **real crates** end to end.

- **[BLOCKER]** Failed runs are cached. `with_lock_detailed` (`ipc/lib.rs:321-325`) stores any exit code with no gate.
  - **Probe-confirmed:** an exit-1 failure was replayed to siblings (`runs=1`, `stdout="FAILED"`, `exit_code=1`) for the full 300 s TTL.
  - The origin harness's `error_ttl` (`rules.conf:25`) is parsed into `RuleOpts.error_ttl` but **has zero read-sites** — feature was dropped in the port.
  - Signal kills (`unwrap_or(-1)`) are cached too.
  - **Fix:** skip store when `exit_code != 0`; honor `error_ttl` and store under a separate slot; or store only success entries.
- **[BLOCKER]** `CoalesceCache::with_lock` blocks forever. `lock_exclusive()` (`ipc/lib.rs:300-303`) has no deadline.
  - **Probe:** caller had not returned after 2.5 s while the lock was held. One hung child wedges every sibling agent silently.
  - **Fix:** change `lock_exclusive()` to `with_deadline(Duration::from_secs(30))`; align with SlotQueue's 30 s loud-fail.
- **[BLOCKER]** One orphaned waiter ticket bricks a lane permanently. `is_my_turn` (`queue.rs:155-196`) uses a static rank with no aging; ticket cleanup isn't RAII.
  - **Probe:** one orphan Critical ticket → `Err("queue timeout")` for a Normal waiter with `max_concurrent=2` and two free slots.
  - **Fix:** real aging like CFS; RAII `WaiterTicket` that decrements on Drop via a guard closure.
- **[MEDIUM]** Nocache exact-match misses `ruff check --fix-only` and `--fix=ALL` — routing mutating runs into the cache.
  - **Fix:** parse `=` separator; flag-set membership.
- **[MEDIUM]** Semantic `ruff check .` ≡ `ruff check src/` is **not implemented** — key is FS-state dependent.
- **[MEDIUM]** Cache key hashes only `env_subset`; the child inherits the full ambient environment.
- **Verified correct:** AC-008.13's env-order/env-value/cwd dimensions all hold; argv-separator and 0x01 cwd-injection hypotheses did **not** reproduce.

---

## Lane 8 — Config hot-reload (`jaguar`)

**Three HIGHs, multiple MEDIUMs.** Lane report: `session_jaguar_1789891991283_a1530c95cfd75048`. Three live probes; repo left clean.

- **[HIGH]** `#[serde(default)]` shadow on `Config` (container-level) at `src/config.rs:19` fills a present-but-partial table from the **child type's** Rust Default (None for `max_memory_mb`/`max_processes`), not `RuntimeConfig::default()` (4096/100).
  - **Two-tier and arbitrary:** `RuntimeConfig, PoolConfig, MonitoringConfig, PortConfig, PathsConfig, DefaultHarnessConfig, ProjectLimitsConfig, SpawnConfig` lack struct-level `#[serde(default)]`; `Config, ServeConfig, ServeJwtConfig, SpawnPolicyConfig, CastConfig, HealthCheckConfig, NotifierConfig` have it.
- **[HIGH]** AC-002.5 is only true for the direct constructor; nothing asserts the TOML-driven path.
- **[MEDIUM-HIGH]** No atomic write. `Config::save/init` use `std::fs::write` = open/truncate/write-in-place. No temp+rename, no `fsync`, no `.bak`.
- **[MEDIUM]** Lost-update race + validation bypass: `config set` reads, mutates, validates candidate, writes whole file. Concurrent CLI edits silently clobbered.

---

## Lane 9 — Tray UX / a11y (`turtle`)

**19 findings, 3 P0 + 8 P1 + 7 P2 + 1 P3.** Read every Swift file (22 of 22). Lane report: `session_turtle_1789891979540_5f8824175ce373ff`.

| # | Sev | Finding |
|---|---|---|
| 1 | **P0** | Cmd+K palette has no keyboard nav and no focus (`CommandPalette.swift` — `selectedIndex` written only on `query` change, `filtered[selectedIndex]` never submitted) |
| 2 | **P0** | Canvas DAG invisible to AT, invisible focusable buttons (`ProcessesTreeCanvas.swift` 100%-transparent `Button` + bare `Canvas`) |
| 3 | **P0** | Palette overlay is not modal to AT (ZStack overlay, no focus trap) |
| 4 | **P1** | No Retry on IPC failure; raw developer strings surfaced (`AppState.swift:406-504`, 6 internal `lastError` strings) |
| 5 | **P1** | HelpSheet documents wrong shortcuts, omits ⌘8 |
| 6 | **P1** | Light-appearance color contrast fails AA on status text |
| 7 | **P1** | `prefers-reduced-motion` ignored — 8 motion sites |
| 8 | **P1** | Destructive fleet actions have no confirm, no role, adjacent to Quit |
| 9 | **P1** | No ⌘F filter-focus equivalent anywhere |
| 10 | **P2** | Empty-state / disconnect-affordance fragmentation (5 implementations) |
| 11 | **P2** | 18 icon-only buttons unlabeled |
| 12 | **P2** | Slider/Toggle missing accessible names/values |
| 13 | **P2** | Motion durations off-token (4 of 8 sites) |
| 14 | **P2** | CTA token contract NOT implemented (system accent, not green/violet) |
| 15 | **P2** | Palette command inventory gaps (no kill-PID, no sidecar restart) |
| 16 | **P2** | Status symbol overloading (gearshape×2, heart×3, kill glyph×3) |
| 17 | **P2** | 5 files over the 500-line hard limit (ProcessesPage 1904, HealthPage 1024, ConfigPage 812) |
| 18 | **P3** | `accessibilityDescription: nil` on 5 NSMenu images |
| 19 | **P3** | Type-design drift in one tile (rounded + monospaced) |

---

## Lane 10 — Spec coverage drift (`t-rex`)

**Spec frozen 2026-08-12; code moved 38 days.** Lane report: `session_t-rex_1789891965226_f306be79d68ecb32`.

- 27 AC ids exist in code but not in FR.md (`AC-006.41`, `AC-007.83–97`, `AC-008.19–20`, `AC-009.15–22+25`).
- 11 dead `Source:` links to `src/commands/proc.rs`, refactored to `src/commands/proc/mod.rs` on 2026-09-16.
- TRACEABILITY.md claims "0 gaps" on a per-FR-only status column, while FR-001 is labeled `ACCEPTED` and 2 of its 6 tests fail in this sandbox.
- `rules.conf` is **not** bit-identical to `rules.conf.from-tar` (11193B vs 5481B, different SHA-256); the working copy added `stale=`, `priority=`, `semantic=`, the Tier-1 mutating-tool section; the tarball uniquely has `error_ttl=`.
- Lane hypothesis corrected: heavy FRs ARE well covered (FR-006 = 3.9 tests/AC, FR-007 = 3.5), not 1 happy-path test per 5 ACs.
- Honest gaps: DDG blocked research twice; `AC-007.11`'s FUSE read-coalesce panel claim unverified.

---

## Severity roll-up

| Severity | Count |
|---|---|
| **BLOCKER** | 8 (1 Rust, 1 Swift, 4 IPC, 1 FUSE hygiene, 3 hypervisor cache, 1 observability, 1 CLI) |
| **HIGH** | ~28 across lanes |
| **MEDIUM** | ~30 |
| **LOW / POLISH** | rest |

## Cross-cutting themes

1. **Auth/authz gap on the IPC socket** — single BLOCKER affects every install.
2. **Failure path is the silent path** — failed runs are cached, errors logged to dev strings, audit opt-in, observability layer order hides 401/429s.
3. **Missing primitives** — `process.spawn`, `pool.effectiveness`, kbd nav on the palette, Cmd+K, Cmd+F, ⌘8, Retry buttons — code references them but they are empty.
4. **Spec drift** — 27 ACs past the spec's last index; 11 dead `Source:` links.
5. **Hypervisor broken** — failed-runs caching + immortal orphans + lock-with-no-deadline = three ways to wedge an agent's fleet silently.
