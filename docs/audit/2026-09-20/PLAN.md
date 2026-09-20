# Mature-Engineering Plan — sharecli @ `2ccfa7ab`

> Synthesizes `FINDINGS.md` (10 audit lanes, 2026-09-20). Tasks decompose to ~10 minutes.
> Pinned lanes: BLOCKER first, then HIGH, then MEDIUM, then POLISH. Each task names
> the source finding(s), the file(s) it touches, the validation it must run before
> being marked complete, and the receipt.

## Goals (success criteria)

1. **Zero BLOCKER** findings from the audit are open. Verified by re-running the same
   probes (each lane's reproduction) and seeing them pass.
2. **No silent failures.** Failed runs are never cached; errors never surface developer
   strings to users; observability never misses 401/429; hypervisor never wedges a
   fleet; IPC socket is auth-checked.
3. **No dead UI hooks.** Pages and palette commands reachable from the UI either
   work end-to-end or are deleted. `process.spawn`, `pool.effectiveness`,
   Cmd+K palette, Cmd+F focus, ⌘8, Retry buttons, kbd nav — all wired.
4. **Spec in sync with code.** FR.md index goes past AC-009.25; all `Source:` links
   resolve; `rules.conf` is the single source of truth, with `error_ttl` re-introduced
   (lane 7 BLOCKER).
5. **Audit re-run clean.** A second pass on the same 10 lanes produces only LOW/POLISH
   findings.

---

## Phase 0 — BLOCKERs (Day 0)

### 0.1 Lock down IPC socket (lane 3 BLOCKER)
- **Files:** `crates/sharecli-ipc/src/main.rs:50-58,133-139`
- **Steps:**
  - `set_permissions(0o600)` after `bind`.
  - `getpeereid` / `SO_PEERCRED` check on each accept; reject if peer uid ≠ euid.
  - Add `Integration: AUTH` test that a non-self peer cannot connect.
- **Receipt:** `cargo test -p sharecli-ipc integration::auth` green; `chmod 600`
  observed on a scratch UDS.

### 0.2 Stop caching failed runs (lane 7 BLOCKER)
- **Files:** `crates/sharecli-ipc/src/lib.rs:321-325`, `crates/sharecli-ipc/src/queue.rs`
- **Steps:**
  - In `with_lock_detailed`, skip store when `exit_code != 0` OR when `signal != 0`.
  - Restore `error_ttl` plumbing: read `RuleOpts.error_ttl` (parse already works, it
    just has no read-sites); if set, store failure entries under a separate slot with
    their own TTL.
  - If `error_ttl` unset, store nothing on failure.
  - Add unit test: exit-1 result is `None` after the next call within the old TTL.
- **Receipt:** lane-7 repro probe (`runs=1`, `exit_code=1`) now returns `Ok(None)`;
  test `cache_negative_skips_store` passes.

### 0.3 Hypervisor lock with deadline (lane 7 BLOCKER)
- **Files:** `ipc/lib.rs:300-303`
- **Steps:**
  - Replace `lock_exclusive()` with `with_deadline(Duration::from_secs(30))`.
  - On deadline, return `Err(HypervisorError::Timeout)`; sibling agents log + retry
    under backoff.
- **Receipt:** the 2.5 s probe now fails with the documented error code, not hangs.

### 0.4 RAII WaiterTicket with aging (lane 7 BLOCKER)
- **Files:** `crates/sharecli-ipc/src/queue.rs:155-196`
- **Steps:**
  - Wrap the ticket in a guard type whose `Drop` decrements the count.
  - Add a rank-aging field (`rank += waited.as_millis()/10`) so an orphan Critical
    ticket does not starve a Normal waiter with free slots.
- **Receipt:** probe `Err("queue timeout")` with `max_concurrent=2` + 2 free slots +
  orphan Critical now succeeds for the Normal waiter.

### 0.5 Implement or delete phantom IPC methods (lane 3 BLOCKERs)
- **Files:** `crates/sharecli-ipc/src/handler.rs`, `IPCClient.swift`, `ProcessesPage.swift:1684`
- **Steps:**
  - Implement `process.spawn` (server-assigned pid; spawn through `runtime::spawn`).
  - Implement `pool.effectiveness` (aggregate from `CoalesceCache` hit/miss counters).
  - If a method is not implementable in v0.9, remove the corresponding page + client
    method, not silently break it.
- **Receipt:** both methods return typed results over a scratch UDS; the Pool
  Effectiveness page renders numbers; the Spawn form succeeds on a valid pid.

### 0.6 Stop `stop --pid 999999` lying (lane 4 BLOCKER)
- **Files:** `src/commands/mod.rs:646-649`
- **Steps:**
  - Inspect `pool.kill()`'s `Result`; print "no such pid" and exit 2 for misses.
- **Receipt:** `sharecli stop --pid 999999` → exit 2, "no such pid: 999999".

### 0.7 Swift IPC read loop has a deadline (lane 2 BLOCKER)
- **Files:** `desktop/ShareCLITray/Sources/ShareCLICore/IPCClient.swift:600-605`
- **Steps:**
  - `setsockopt(SO_RCVTIMEO, …)`; cap concurrent probes at the measured 64 ceiling
    with a semaphore.
  - Move read off `Darwin.read` to `poll(2)`/`DispatchSourceRead` with deadline.
- **Receipt:** block-forever probe fails with `IPCError.timeout`; supervisor recovers.

---

## Phase 1 — HIGH (Day 1-2)

### 1.1 Restore observability layer order (lane 1, 5)
- `src/commands/serve.rs:256-258` — reorder so `HttpRedMetrics` and `traceparent` are
  outermost. Validate by `cargo run -- serve --port 9999` then `curl :9999/healthz`
  without auth → metrics + trace span present.

### 1.2 Hot-reload trailing debounce + watch path resolution (lane 1, 8)
- `src/config_watcher.rs:61-73` — trailing debouncer.
- `src/commands/serve.rs:159-165` — resolve via `Config::load` helper.
- Validate by `SHARECLI_CONFIG_PATH=/tmp/c.toml sharecli serve`, edit + save → reload
  fires; rapid 5 edits within 200 ms → exactly one reload.

### 1.3 Fix `#[serde(default)]` shadowing (lane 8)
- `src/config.rs:19` — make each table own its defaults (struct-level for `RuntimeConfig`
  et al.); document the policy.
- Add `RuntimeConfig::from_partial_toml()` test that asserts 4096 / 100 from a partial
  `[runtime]` table.

### 1.4 Atomic config write (lane 8)
- `Config::save/init` — temp file in same dir, `fsync`, `rename`; keep `.bak`.
- Validate by killing mid-write; load succeeds against `.bak`.

### 1.5 `if_revision` for `config.set` (lane 3)
- `handler.rs:560-566,634-653` — `config.revision` method, `if_revision` param on
  `config.set`, return `CONFLICT` with current revision on mismatch.

### 1.6 Framed IPC, capped line length, single connection (lane 3)
- `main.rs:96-107,116-127` — `LinesCodec::new_with_max_length`.
- `IPCClient.swift:14-24` — single long-lived connection with id→continuation map
  and per-request cancellation.
- Validate by feeding a 1 MB line; second connection attempt rejected.

### 1.7 Effective cadence in Swift polling (lane 2)
- `AppState.swift:273-278` — deadline loop on `ContinuousClock`.

### 1.8 Swift supervisor tick is bounded and self-resetting (lane 2)
- `SidecarSupervisor.swift:130-132` — bound tick with deadline; reset on exit.

### 1.9 Restore AC indices + dead `Source:` links (lane 10)
- `docs/specs/FR.md` — extend FR-006, 007, 008, 009 with the missing ACs.
- `docs/specs/TRACEABILITY.md` — replace `proc.rs` with `proc/mod.rs` everywhere.

### 1.10 CLI exit-code taxonomy (lane 4)
- `src/error.rs` — distinct `UserInput → 64`, `NotFound → 2`, `Internal → 1`; stop
  double-printing.

### 1.11 Daemonized tracing subscriber (lane 5)
- `main.rs:906,915` — always install subscriber; route to syslog/journald when stderr
  is not a TTY.

### 1.12 Pool IPC metrics + label uniqueness (lane 5)
- `serve.rs:584-604,551-555` — single shared `ProcessPool`; cache `lsof` for ≤ 2 s;
  label by `pid` not `process`.

### 1.13 FUSE inode-map + rename invalidation (lane 6)
- `inode_map.rs:72-80`, `lib.rs:962-970` — on `rename`, `remove_rel` of destination;
  on `rename` of a directory, descend and invalidate the subtree.
- Validate by probe: `rename a.txt b.txt` → `resolve(2)=b.txt`, `resolve(3)=undefined`.

### 1.14 FUSE unlink keeps open fds readable (lane 6)
- `lib.rs:912` — `open` stores `FileHandle(ino.0)` plus an owned `PathBuf` captured
  at `open` time, so reads continue to work.

### 1.15 FUSE setattr path lock (lane 6)
- Add path lock around `setattr`; tests for concurrent `write`+`truncate` on the
  same path.

### 1.16 FUSE staging collision-safe (lane 6)
- `write_serialize.rs:91-96` — `tempdir()` per staging, unique names; EXDEV fallback
  stages into destination directory.

### 1.17 FUSE neg-dentry cap + sweeper (lane 6)
- `neg_dentry.rs` — LRU with `DEFAULT_NEG_CAP = 65_536`; sweeper removes entries not
  re-probed in `DEFAULT_NEG_TTL * 10`.

### 1.18 FUSE missing ops: symlink + fsync (lane 6)
- Implement `readlink`, `symlink`, `link`, `flush`, `fsync`, `release` so symlinked
  `node_modules` resolve and durability holds.

### 1.19 Hypervisor: `error_ttl` plumbed; nocache `=` parse (lane 7)
- `queue.rs` / `ipc/lib.rs` — read `RuleOpts.error_ttl`; `nocache` parser splits on `=`
  and matches flag set; semantic equivalence for `ruff check` walks implemented.

### 1.20 Tray: Cmd+K palette kbd nav + focus trap (lane 9, P0)
- `CommandPalette.swift` — `FocusState`, `onSubmit`, `onKeyPress(.upArrow/.downArrow)`;
  palette `.isModal` to AT.

### 1.21 Tray: Canvas DAG focusable + labeled (lane 9, P0)
- `ProcessesTreeCanvas.swift` — render DAG as buttons per node with labels;
  `accessibilityElement(children: .contain)` on the canvas.

### 1.22 Tray: Retry buttons (lane 9, P1)
- One `Retry` button per failed action; replace developer strings with actionable
  copy; localized key for "Not connected to sharecli-ipc" → "Sidecar not running —
  Start".

### 1.23 Tray: Reduce-motion honored (lane 9, P1)
- Wrap every motion site in `@Environment(\.accessibilityReduceMotion)`.

### 1.24 Tray: Destructive confirm + role (lane 9, P1)
- `Kill selected` / `Kill all` → `Button(role: .destructive)` + `.confirmationDialog`.

### 1.25 Tray: ⌘F filter focus (lane 9, P1)
- Four filter fields get a `.keyboardShortcut("f", modifiers: .command)`.

### 1.26 HelpSheet shortcuts complete (lane 9, P1)
- Document ⌘1..⌘8, ⌘K, ⌘F, ⌘R, ⌘W, ⌘/, ⌘, with accurate platform markers.

### 1.27 Tray: light-appearance AA (lane 9, P1)
- Audit 8 files for status text contrast; introduce semantic tokens
  (`StatusForeground`, `ValueForeground`) and check both appearances.

### 1.28 Tray: declarative file decomposition (lane 9, P2 #17)
- `ProcessesPage` 1904 → ≤500: split into `ProcessesTable`, `ProcessRow`,
  `ProcessDetailSheet`, `SpawnForm`.
- `HealthPage` 1024 → split: `HealthHeader`, `HealthCharts`, `HealthFooter`.
- `ConfigPage` 812 → split: `ConfigTable`, `ConfigEditor`, `ConfigPreview`.

### 1.29 Tray: CTA tokens (lane 9, P2)
- Add `AccentColor` asset; replace `.buttonStyle(.borderedProminent)` calls with
  `.tint(Color("AccentPrimary"))`; add violet/green tokens from `assets/tokens.css`.

### 1.30 Tray: status-symbol consolidation (lane 9, P2)
- Single `StatusIcon` enum + `IconFor.action(.killAll)` resolver; replace 3 hearts
  and 3 kill glyphs with one each.

---

## Phase 2 — MEDIUM (Day 3-5)

### 2.1 Cold-cache limits in `BufReader::lines` → LinesCodec with N
### 2.2 Rate-limit/circuit-breaker middleware order
### 2.3 `/proc/<pid>/io` once per scrape (cached)
### 2.4 Lint comment double-print removed
### 2.5 Help text for `ps --json` / `proc --json`
### 2.6 Per-FR acceptance status reflects test count
### 2.7 `rules.conf` single source of truth; remove `from-tar`
### 2.8 WebSocket close handshake on shutdown
### 2.9 Neg-dentry process-wide atomic counters → per-mount
### 2.10 `with_lock_detailed` skip-store on signal too
### 2.11 ANSII `ESC [` byte sequence
### 2.12 `prefers-reduced-motion` SwiftUI environment
### 2.13 Dead code: `src/commands/history.rs`, `src/api.rs`
### 2.14 Tray: empty-state consolidation to one component
### 2.15 Tray: accessible names on sliders/toggles
### 2.16 Tray: motion tokens centralized
### 2.17 Spec: bit-identical `rules.conf` + `error_ttl` line restored
### 2.18 Audit: hot-reload signal handlers distinct from server lifecycle

---

## Phase 3 — POLISH (Day 6+)

- Tray: NSMenu image `accessibilityDescription`
- Tray: variable-length title alignment
- Tray: type-design drift in `MiniCompositeHealthCard`
- CLI: truecolor leak to pipe + `version` output to stderr
- CLI: `sharecli upgrade --check`
- FUSE: xattr namespace rename to `com.sharecli.*`
- FUSE: `xattr::set_deref` for symlink-follow
- FUSE: `parent dir fsync` after rename
- FUSE: `libc::EXDEV` constant instead of literal `18`
- FUSE: smoke hermetic (clean up smoke-dir)
- IPC: `JSONRPC`-style `code`/`message`/`data`
- IPC: response echo `version` field
- IPC: one-time `ipc.hello` on connect, store capabilities

---

## Phase 4 — Re-audit (Day 7)

- Re-spawn the 10 lanes with the same prompts and the same probes. Expect only
  LOW / POLISH findings.
- Capture diff in `docs/audit/2026-09-20/RESULTS.md`.

---

## Acceptance receipts (one per BLOCKER + per HIGH lane)

| Lane | Receipt |
|---|---|
| squid | `cargo test -p sharecli` + scratch `serve` + auth probe |
| turkey | `swiftc -typecheck -swift-version 6` clean + blocking probe fails fast |
| whale | `cargo test -p sharecli-ipc` + UDS probe matrix |
| parrot | `sharecli stop --pid 999999` exits 2; `sharecli --theme foo` exits 64 |
| rabbit | 401/429 traced; daemon logs visible in syslog |
| swan | 128 unit tests pass + symlink + unlink + rename + CoW probes |
| tiger | lane-7 reproduction script returns Ok on all 3 BLOCKER probes |
| jaguar | config patch with partial table restores defaults + atomic write |
| turtle | VoiceOver passes Cmd+K, ⌘F, ⌘8, Retry; 0 icon-only unlabeled |
| t-rex | FR.md index >= AC-009.25; `proc.rs` link 0 hits in `grep` |

---

## Open unknowns (NOT blocking this plan)

- Real VoiceOver output for unlabeled buttons (needs Accessibility Inspector).
- AXKit/FSEvents live overshoot unverified by design (no process/display capture).
- `Cargo.lock` unrelated `agileplus-cache` duplicate key warning.
- DDG web search blocked twice during lanes 4, 6, 10.
