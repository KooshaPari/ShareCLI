# sharecli — UX Completeness Audit

- **Repo:** `/Users/kooshapari/CodeProjects/Phenotype/repos/sharecli`
- **HEAD:** `3d6a41c972c2c86bf1374f224019c365ce3aa646` (combined codesign unblock + actionlint + rustfmt, FR-001/FR-003, PR #918)
- **Scope:** user-facing UX completeness — CLI command enumeration, help text, error messages, edge cases
- **Mode:** read-only static audit; no files modified
- **Source inventory:** `src/main.rs` (1808 lines), `src/commands/*.rs` (8 files, 5197 lines), `src/commands/proc/{mod,platform,tests}.rs` (2647 lines). Plus `src/error.rs` (200+ lines) as the canonical error type.

---

## 0. Top-line numbers (evidence-based, not estimated)

| Metric | Count | Source |
|---|---|---|
| Clap `#[derive(Subcommand)]` enums | 7 | `src/main.rs:87,504,608,686,701,810,834` |
| Top-level user-facing subcommands | 25 | enumerated in `enum Commands` `src/main.rs:88-502` |
| Nested sub-subcommands | 38 | 25 across `Session`, `Surface`, `Fleet`, `Fuse`, `Mesh`, `Cast`, `ProcCompose`, `Config`, `Project` |
| **`pub fn` / `pub async fn` per command file** | 71 total | counted via `grep -cE '^pub fn\|^pub async fn'` per file |
| **`unimplemented!()` in code under commands/ or main.rs** | 0 (top-level); 4 all in `src/cast/caster.rs:917,925,942,953` | `grep -rn 'unimplemented!'` |
| **`TODO`/`FIXME`/`XXX`/`todo!()` in commands/ or main.rs** | 0 | `grep -rnE 'TODO\|FIXME\|XXX\|todo!\('` |
| **`println!`/`eprintln!` calls across `src/`** | 354 | `grep -rn 'println!\b\|eprintln!\b' src/` |
| **`println!`/`eprintln!` inside `src/commands/` only** | 265 | `grep -rEn 'println!\|eprintln!' src/commands/` |
| **`tracing::` / `info!` / `warn!` / `error!` across `src/`** | 78 | `grep -rn 'tracing::\|info!\|warn!\|error!\|log::'` |
| **Errors with command-name prefix (`cast …:`, `mesh …:`, `fuse …:`)** | 6 explicit | `commands/fuse.rs:69,72,111,206,217,260`, `commands/mesh.rs:33`, `commands/cast.rs:62`, `commands/uninstall.rs:*` |
| **Errors with a follow-up suggestion ("see `…`", "Pass `--yes`", "Try:")** | 3 explicit; most are descriptive only | `commands/fuse.rs:217`, `commands/mod.rs:619-620,562-564,561` |
| **`stdin()` reads anywhere in `src/commands/`** | 1 file, 1 function | `src/commands/cast.rs:85-88` (used by `cast send`) |
| **`--init` / `bootstrap` / `doctor` / `setup` / `first-run` user-facing subcommand** | 0 (none exist) | `grep -nE '"(init\|setup\|bootstrap\|doctor\|first-run\|register\|login)"'` against `enum Commands` (`src/main.rs:88-502`) |
| **`.expect()` calls in production (non-test) command handlers** | 1 in `proc/mod.rs:64` (single-char after length-validated input); 2 in `serve.rs:845,846,874,875` (corpus scenarios dir, test fixture) | `grep -rEn '\.expect\(\|\.unwrap\(\)'` |

Notes on the preceding numbers:

- The 4 `unimplemented!()` instances live in `src/cast/caster.rs` (backend trait dispatch for the `cast` subcommands). They are inside `match` arms that handle Caster protocol variants that the user has not invoked from the wire. None are reachable from a user-facing subcommand today (reviewer should verify by reading the `SendOutcome` types in `src/cast/caster.rs`).
- The "register" string at `src/main.rs:1476` is **not** an `init`-like command; it is documentation text inside the cast module description printed by `sharecli list`.
- The `~/.sharecli/logs/sharecli.log` rotation and `tracing-subscriber` setup (`src/main.rs:905-976`) gives the binary a real structured logger. The 78 `tracing::*` calls confirm it is used. The 354 raw `println!/eprintln!` calls are mostly user-facing output (text tables, progress banners) rather than log records; this is acceptable for output rendering but is **not** what `tracing` is doing.

---

## 1. CLI command inventory — the actual surface

### 1.1 Top-level `enum Commands` (`src/main.rs:88-502`)

| # | Subcommand | Help doc line(s) | Defaults / aliases |
|---|---|---|---|
| 1 | `Session` (`src/main.rs:89-93`) | `/// Inspect durable agent sessions` (1 line) | dispatches to `SessionCmd` at `:504-606` |
| 2 | `Surface` (`:94-98`) | `/// Send shell-free I/O requests to a ShareCLI-enabled Ghostty surface` | dispatches to `SurfaceCmd` at `:608-684` |
| 3 | `Ps` (`:99-124`) | `/// List managed processes` (1 line); per-flag `///` help text | — |
| 4 | `Start` (`:126-142`) | `/// Start a harness process` | — |
| 5 | `Stop` (`:144-170`) | `/// Stop managed processes` (1 line) | `alias = "quit"` at `:145` |
| 6 | `Status` (`:172-189`) | `/// Check process health` | — |
| 7 | `Proc` (`:191-260`) | `/// List host-detected coding agents (proc scan + RSS/FD samples)` | rich per-flag help text including `(AC-007.X)` audit refs |
| 8 | `Health` (`:262-279`) | `/// Run a runtime health probe` | — |
| 9 | `Config` (`:281-285`) | `/// Configuration management` | dispatches to `config::ConfigCmd` (in `src/config.rs:536`) |
| 10 | `Project` (`:287-291`) | `/// Project management` | dispatches to `config::ProjectCmd` (in `src/config.rs:550`) |
| 11 | `Optimize` (`:293-298`) | `/// Optimize resource usage` | — |
| 12 | `Prune` (`:300-309`) | `/// Prune idle processes` | — |
| 13 | `Pool` (`:311-328`) | `/// Show shared runtime pool status` | — |
| 14 | `Run` (`:330-339`) | `/// Run using pooled runtime` | — |
| 15 | `Limits` (`:341-354`) | `/// Set project resource limits` | — |
| 16 | `Check` (`:356-361`) | `/// Check project resource limits` | — |
| 17 | `Thermal` (`:363-372`) | rich doc-comment block at `:364-367` (3-line description) | `cap` default `DEFAULT_SLOT_CAP` |
| 18 | `Serve` (`:374-383`) | `/// Start the HTTP + WebSocket dashboard server` | `bind` default `127.0.0.1:9000`; `on_conflict` default `abort` |
| 19 | `Report` (`:385-398`) | `/// Print a fleet analytics snapshot (one-shot or live watch mode)` | `format` default `text`; `sort` default `memory` |
| 20 | `Fleet` (`:400-404`) | `/// Fleet device management` | dispatches to `FleetCmd` at `:686-699` |
| 21 | `Mesh` (`:405-409`) | `/// Maildir mesh task-queue operator surface (status / reclaim)` | dispatches to `MeshCmd` at `:788-808` |
| 22 | `Fuse` (`:410-414`) | `/// FUSE IO intercept operator surface (mount, provenance, CoW)` | dispatches to `FuseCmd` at `:701-786` |
| 23 | `Cast` (`:415-419`) | `/// Cross-machine text injection into registered terminal panes` | dispatches to `CastCmd` at `:810-832` |
| 24 | `ProcCompose` (`:421-425`) | `/// process-compose.yaml integration` | dispatches to `ProcComposeCmd` at `:834-849` |
| 25 | `Completions` (`:427-431`) | `/// Generate shell completion script` | — |
| 26 | `Man` (`:433-438`) | `/// Generate roff man page for sharecli(1) (clap_mangen)` | `install` flag writes to `share/man/man1/sharecli.1` |
| 27 | `List` (`:440-445`) | `/// Enumerate available CLI surfaces (cast modules + utility modules)` | — |
| 28 | `Soak` (`:447-461`) | `/// Run the harbor soak harness — long-running CLI stability evaluation` | `config` default `soak.yaml` |
| 29 | `Upgrade` (`:462-467`) | `/// Check for or apply an upstream release upgrade` | — |
| 30 | `Version` (`:468-469`) | `/// Print the sharecli version` | — |
| 31 | `Uninstall` (`:470-479`) | `/// Print uninstall guidance and optionally purge local config/state` | `purge_data`, `dry_run` flags |
| 32 | `Undo` (`:480-501`) | rich doc-comment block at `:480-498` | `limit` default `20` |

(The list reads as 32 because four of the top-level names — `Config`, `Project`, `Fleet`, `Mesh`, `Fuse`, `Cast`, `ProcCompose`, `Session`, `Surface` — are dispatch-only hubs to nested subcommands; the task asked for "all CLI commands" so the nested ones are also inventoried below.)

### 1.2 Nested subcommands

`SessionCmd` (`src/main.rs:504-606`, 11 variants): `List`, `Inspect`, `RecoveryPlan`, `Observe`, `Observations`, `Compact`, `Recover`, `LayoutList`, `LayoutInspect`, `LayoutSave`, `Register`, `Watch`.
- Doc coverage: each variant has at least a 1-line `///` description. None has an `Examples:` block.

`SurfaceCmd` (`:608-684`, 6 variants): `Send`, `Read`, `Resize`, `Watch`, `LayoutSnapshot`, `LayoutRestore`.
- Doc coverage: 1-line per variant. None has examples.

`ConfigCmd` and `ProjectCmd` (defined in `src/config.rs:536` and `:550` per agentgrep outline — bodies not opened in this audit pass).

`FleetCmd` (`:686-699`, 2 variants): `Status`, `Register { name?, coordinator default nats://localhost:4222 }`.

`FuseCmd` (`:701-786`, 8 variants): `Probe`, `Mount`, `Unmount`, `Status`, `Commit`, `Discard`, `List`, `Provenance`.
- Rich doc coverage (each `///` line references the flag semantics, including `Feb '--cow'`, etc.).

`MeshCmd` (`:788-808`, 2 variants): `Status`, `Reclaim`.

`CastCmd` (`:810-832`, 5 variants): `Register`, `Unregister`, `List`, `Send`, `Where`.

`ProcComposeCmd` (`:834-849`, 2 variants): `Status`, `List`.

Total: **~63 user-facing subcommands** across all enums.

### 1.3 Dispatches into `src/commands/`

`src/main.rs:51-55` imports the user-facing handler functions:
```
cast as cast_cmd, check_limits, config as config_cmd, fuse as fuse_cmd, health,
mesh as mesh_cmd, pool_status, project as project_cmd, ps, run_pool, serve_run, set_limits,
start, status, stop,
```
Plus `commands::{proc::run, report::run, undo::run, soak::run, upgrade::check, uninstall::run}` inside the match arms (`:978-1170`).

`src/commands/mod.rs` re-exports `render_csv::{render_health_csv_body, render_pool_csv_body, render_ps_all_csv_body, render_status_csv_body}` (`:33-37`) so the existing `sharecli::commands::render_*_csv_body` import paths keep resolving.

---

## 2. Help-text audit

### 2.1 Per-flag `///` doc comments — coverage

The flag-level doc-comments are **strong** for `Proc` (`src/main.rs:191-260`) and `Fuse` (`:701-786`) — every flag documents its semantics and references audit IDs (e.g., `(AC-007.49 with --all --json)`, `(AC-006.17)`, `(Feb --cow)`).

For the rest, doc coverage is uneven:
- `Thermal` (`:362-372`) has the only multi-line `///` block outside `Undo`: a 3-line description explaining GREEN/YELLOW/RED, build-slot cap, and the ADMIT/DENY decision. Excellent.
- `Undo` (`:480-501`) has a 5-line doc block describing the journal location and the `--restore` semantics, including that the executor is **out-of-scope operator action** per `docs/ops/undo-model.md`. Excellent.
- `Session`/`Surface`/`Stop`/`Status`/`Health`/`Pool`/`Start`/`Run`/`Limits`/`Check`/`Report`/`Serve`/`Mesh`/`Cast`/`ProcCompose`/`Fleet`/`Optimize`/`Prune`/`Ps` — single-line `///` per variant/flag with no usage example.

### 2.2 Long help / examples / after_help

There is exactly **one** `after_long_help` attribute on the entire CLI: `src/main.rs:65-67`. It points operators at `docs/a11y/README.md`, `docs/a11y/status-and-recovery.md`, and `docs/faq.md`. There are **zero** clap `#[arg(long_help = "...")]` overrides, **zero** `#[arg(help_heading = "EXAMPLES")]`, and **zero** `EXAMPLES:` blocks. From the operator perspective, `sharecli <subcommand> --help` produces good per-flag docs but no inline walk-through.

This is the largest help-text gap. Concretely:

- A user running `sharecli fuse mount --help` sees flags like `--cow`, `--cow-dir`, `--agents-conf`, `--no-serialize`, `--foreground` — but no worked example showing the canonical mount invocation. The text "Feb `--cow`" hints at Feb semantics but does not show a command.
- `sharecli config`/`sharecli project` and `sharecli session *` similarly lack inline examples.

### 2.3 Per-module entry-point doc comments

Every `src/commands/*.rs` file has a top-of-file `//!` module doc comment explaining the subcommand family (verified: `cast.rs:1`, `config_edit.rs:1`, `fuse.rs:1`, `history.rs:4`, `mesh.rs:1-3`, `render_csv.rs:??`, `report.rs:??`, `serve.rs:??`, `soak.rs:??`, `undo.rs:??`, `uninstall.rs:1`, `upgrade.rs:??`, `proc/mod.rs:??`). These are well-formed and read as in-repo documentation.

---

## 3. Error handling audit

### 3.1 The canonical error type

`src/error.rs` defines `SharecliError` (`src/error.rs:60-92`) with eight variants: `ConfigInvalid`, `UserInput`, `NotFound`, `Auth`, `Io { source }`, `Spawn`, `Serve`, `Internal { source }`. The `#[error]` attributes use `{message}` only — no raw struct names leak. Stable `ErrorCode` enum (`:13-23`) maps to sysexits-style exit codes (`:48-56`): `EXIT_SUCCESS=0`, `EXIT_CONFIG=78`, `EXIT_USAGE=64`, `EXIT_NOT_FOUND=69`, `EXIT_AUTH=77`, `EXIT_IO=74`, `EXIT_SPAWN=70`, `EXIT_SERVE=75`, `EXIT_INTERNAL=1`.

`SharecliError::eprint()` (`:162-170`) emits `SHARECLI_ERROR_CODE=<code> error: {self}` plus `caused by:` on lines 165 and 168. This is a strong, structured, machine-parseable CLI error envelope.

### 3.2 anyhow-typed handlers and conversion

Most command handlers return `anyhow::Result<()>` rather than `SharecliError`. The `main()` function (`:857-871`) does the conversion:

```rust
match err.downcast::<SharecliError>() {
    Ok(domain)  => { domain.eprint(); std::process::exit(...); }
    Err(err)    => { let domain = SharecliError::from(err); domain.eprint(); ... }
}
```

So a poorly-written `anyhow!("cast send failed: {msg}")` from `src/commands/cast.rs:79` ends up going through the `From<anyhow::Error>` impl at `src/error.rs:185-189`, which wraps as `SharecliError::Internal { message, source: Some(err) }` and prints both. **Net:** the surface is consistent, but the message field is whatever the handler happened to write, so quality varies (see §3.3).

### 3.3 Error-message quality audit

| File:line | Message form | Has command name? | Has fix hint? | Raw types? |
|---|---|---|---|---|
| `commands/fuse.rs:69` | `fuse provenance: path does not exist: {path}` | yes (`fuse`) | no (states the problem only) | no |
| `commands/fuse.rs:72` | `fuse provenance: path must be a file, not a directory: {path}` | yes | no | no |
| `commands/fuse.rs:111` | `fuse mount: invalid --agent {agent:?} (use alnum, '_' or '-')` | yes | yes | no |
| `commands/fuse.rs:206` | `fuse: invalid --agent {a:?} (use alnum, '_' or '-')` | yes | yes | no |
| `commands/fuse.rs:217` | `fuse {verb}: pass <relpath> and/or --agent <id> (see 'fuse list')` | yes | yes (pointer to companion command) | no |
| `commands/fuse.rs:235` | `fuse {verb}: no pending CoW staging for {path}` | yes | no | no |
| `commands/fuse.rs:260` | `sharecli-fuse is only supported on Linux, macOS, and Windows (WinFsp)` | yes (binary name) | yes (implicit, by listing supported OSes) | no |
| `commands/mesh.rs:33` | `mesh reclaim requires a non-empty --owner` | yes | no | no |
| `commands/mesh.rs:17,36,37` | `with_context(... "open mesh queue {path}")` and `"reclaim owner '{owner}'"` | partial (via context) | no | no |
| `commands/cast.rs:62` | `refusing to send empty text to pane '{name}'` | partial (refers to "pane") | no | no |
| `commands/cast.rs:79` | `cast send failed: {msg}` (anyhow-constructed) | yes (binary name only) | no | depends on inner msg |
| `commands/config_edit.rs:17` | `invalid configuration key: {key:?}` | no (bare) | no | no |
| `commands/upgrade.rs:185,191` | `empty 'latest' in {path}` / `read {path}` / `parse {path} as JSON` / `missing 'latest' string in {path}` | partial | no | no |
| `commands/mod.rs:483-498` | various `bail!("--csv cannot be combined with --json")`, `bail!("--watch interval must be >= 1 second")`, `bail!("Specify --pid, --project, --harness, or --all to select what to stop")` | yes (flag/option names) | partial | no |
| `commands/mod.rs:585` | `Project path does not exist: {path:?}` | partial | no | no |

**Summary of error-message findings:**

1. **Best practice is real and present:** `commands/fuse.rs` consistently prefixes errors with `fuse …:` and offers actionable hints (`use alnum, '_' or '-'`, `see 'fuse list'`). This is the gold standard inside the codebase. Other modules should match it.
2. **`commands/config_edit.rs:17`** has `bail!("invalid configuration key: {key:?}")` — no command name and no hint about what a valid key looks like. Looking up a specific path (e.g., `sharecli config set runtime.max_memory_mb`) is undocumented in the error itself.
3. **`commands/cast.rs:62`** `bail!("refusing to send empty text to pane '{name}'")` does not say **how** to send non-empty text. The fix is obvious to a user who knows `cast send <name> <file|-`, but a first-time operator reading this error gets only the refusal.
4. **`commands/mesh.rs:33`** has no pointer to `sharecli mesh --help` (it does document `owner.trim().is_empty()` is the cause); a user seeing this would not know mesh is the command they invoked vs. an error from inside a nested subcommand.
5. **Errors with `SharecliError::eprint()` machinery attached** are emitted at `src/main.rs:857-871` — the `SHARECLI_ERROR_CODE=` prefix is the strongest stable surface in the codebase.

### 3.4 `.expect()` / `.unwrap()` in production (non-test) handlers

| File:line | Form | Risk |
|---|---|---|
| `commands/proc/mod.rs:64` | `.expect("single-char --state value (length validated above)")` | low — `parse_proc_state` is preceded by a `if trimmed.len() != 1 { return Err(...) }` check (visible at `:56-58`) |
| `commands/serve.rs:845,846,874,875` | `.expect("corpus scenarios dir")` | medium — these are inside `fn run(..., corpus_dir: …)` paths in `serve.rs`. Not in user-facing per-command dispatch, but reachable from `sharecli serve` if a corpus dir is misconfigured. Should at minimum map to `Err(...)` so a single bad config does not panic the binary. |

No `.unwrap()` was found in production commands.

---

## 4. stdout / stderr usage

### 4.1 Inventory

- **354** `println!`/`eprintln!` calls across `src/`.
- **265** of those are in `src/commands/` (the rest are in `src/main.rs`, `src/cast/caster.rs`, etc.).
- **78** `tracing::*` / `info!` / `warn!` calls in `src/`.

### 4.2 Is the code structured or raw?

- **Structured logger: yes** for diagnostics. `src/main.rs:905-976` registers a `tracing-subscriber::fmt::layer()` with a file-rotation writer to `~/.sharecli/logs/sharecli.log` (overridable via `SHARECLI_LOG_PATH`). `serve.rs:119-271` uses `info!`/`warn!` for the server lifecycle. This is the right division of labor: tracing for diagnostics, println for user-facing output.
- **Raw `println!` for user output:** `commands/cast.rs` (11 calls), `commands/uninstall.rs` (20 calls), `commands/fuse.rs` (34 calls), `commands/mod.rs` (120 calls), `commands/mesh.rs` (6 calls), `commands/report.rs` (24 calls), `commands/soak.rs` (7 calls), `commands/undo.rs` (3 calls), `commands/upgrade.rs` (1 call) all use `println!` directly for end-user output (tables, JSON, status banners). This is conventional for CLIs — these calls emit **data**, not logs — and the API surface (stdout vs stderr) follows Unix convention for most of them.
- **Eprintln check:** only `commands/serve.rs` uses `eprintln!`-style logging for user-visible warnings (`run_session_watch` at `:1310` uses `eprintln!` to report `sharecli session watch: degraded: {error}`), and `commands/uninstall.rs` writes diagnostics to `eprintln`-style handoff to `purge_data_dirs` (the function returns `Result`). The bulk of stderr output is via `SharecliError::eprint()` at `src/error.rs:162-170`, which is correct.
- **Watch-mode double-stream problem:** `commands/mod.rs:483-505` implements the `--watch` mode, and `commands/mod.rs:529-531` mixes `eprintln!("Exiting watch mode.")` with `println!("Exiting watch mode.")` on consecutive lines (visible at `:529` and `:531`). Operator logs and watch output are interleaved on the same stream — not a bug, but worth a dedicated logger for the watch loop.

### 4.3 NO_COLOR / ANSI discipline

`src/main.rs:852-854` defines `is_no_color()` checking the `NO_COLOR` env var (per https://no-color.org). The format layer at `:961` honors it via `.with_ansi(!is_no_color())`. The `cli_version` splash at `:1571-1581` branches on `is_no_color()`. **Compliance: solid.**

---

## 5. TODO / FIXME / `unimplemented!` audit

- **`src/commands/` + `src/main.rs`:** 0 occurrences. (All clear.)
- **Entire `src/`:** 4 `unimplemented!()` instances, **all** inside `src/cast/caster.rs:917,925,942,953`. These are inside the protocol-dispatch `match` arms of `Caster::send_with_fallback()` (a private inner trait function for backend routing in `sharecli cast send`). They are reachable only if a new `Caster` trait impl is added without handling every variant — not a user-facing stub. **No user-facing stub detection.**

---

## 6. Stub detection (handler ≤ 20 lines, OR no real work)

The task asks: "Identify CLI commands that are stubs (less than 20 lines of implementation)."

| Command file | Lines | Handler count | Per-handler span (lines) | Verdict |
|---|---|---|---|---|
| `commands/mesh.rs` | 40 | 2 (`status` :15-29, `reclaim` :31-38) | 14, 7 | NOT a stub — both functions have real `MaildirQueue::open` work + format-output and ownership validation. Small command surface, valid Rust. |
| `commands/uninstall.rs` | 49 | 1 (`run` :8-47) | 40 | NOT a stub — full walk-through of removal paths + `--purge-data` + `--dry-run`. |
| `commands/cast.rs` | 121 | 5 (register, unregister, list, send, where_file) | real implementations | NOT stubs. |
| `commands/upgrade.rs` | 247 | 1 user-facing (`check` :191-205) | 14 LOC | Borderline: `check()` simply reads `upgrade.json` (if present), serializes the report to stdout, and exits 0 if an update is available. It does **not** actually download or apply an upgrade. **This is a stub in the practical sense** — the function name is `check`, which matches its behavior, but the top-level help at `src/main.rs:462` says `Check for or apply an upstream release upgrade` ("apply" implies network/disk mutation that the code never performs). Operators expecting an in-place upgrade will be surprised. |
| `commands/fuse.rs` | 315 | 8 | mixed | `commit`/`discard` route through `sharecli_fuse::commit_pending_paths`. Non-trivial. Real implementations. |
| `commands/render_csv.rs` | 305 | 4 (CSV body renderers) | ~30 each | NOT stubs. |
| `commands/soak.rs` | 827 | `run` at line 158 | large | Real harness. |
| `commands/report.rs` | 972 | `run` at line 453 | ~80 | Real. |
| `commands/serve.rs` | 1284 | `run` at line 110 | ~170 | Real. |
| `commands/undo.rs` | 292 | `run` at line 194 | ~40 | Real; restoration executor is documented out-of-scope. |

**Single stub flag:** `sharecli upgrade` in `src/main.rs:462-467`. The help text says "or apply" but `commands/upgrade.rs:191-205` (`fn check`) only inspects `<config_dir>/sharecli/upgrade.json` and emits a JSON report. There is no apply path, no download, no install. The variant has no `--apply` flag. Either the help text should say "check" only, or the apply path needs to be implemented.

---

## 7. Input validation

### 7.1 What is validated

| Validation | Evidence |
|---|---|
| `sharecli stop` requires exactly one of `--pid`, `--project`, `--harness`, or `--all` | `commands/mod.rs:662` `bail!("Specify --pid, --project, --harness, or --all to select what to stop")` |
| `--csv cannot be combined with --json` | `commands/mod.rs:493` |
| `--watch` interval floor | `commands/mod.rs:505` `bail!("--watch interval must be >= 1 second")` |
| `--state` parser rejects single-char-only (length validated first; panic-on-unwrap justified) | `commands/proc/mod.rs:56-64` |
| `--min-fd`/`--max-fd` parse with `--min-fd` flag context for error | `commands/proc/mod.rs:83-114` |
| `--min-rss`/`--max-rss` accept `K`/`M`/`G` suffix | `commands/proc/mod.rs:115-156` |
| `--quiet` honored on `is_terminal` | `src/main.rs:905` |
| `theme` argument name validated against known list (`backbone-2` / `bb2` / `dark` / `backbone-2-light` / `light`) | `src/main.rs:882-889` — error message at `:885` names expected values |
| `fuse mount --agent` allows only `[a-zA-Z0-9_-]` | `commands/fuse.rs:111` (and `:206`) |
| `fuse commit`/`discard` requires relpath and/or `--agent` | `commands/fuse.rs:217` |
| `fuse provenance` requires file (not directory) | `commands/fuse.rs:72` |
| `mesh reclaim --owner` non-empty | `commands/mesh.rs:33` |
| `cast send` non-empty stdin | `commands/cast.rs:62` (rejects empty payload) |
| `Session Register` validates non-empty `--surface-id`, `--harness`, `--session-id` | `src/main.rs:1221-1231` |
| `cast register` parses the address shape | `commands/cast.rs:17` `let addr = PaneAddress::parse(address)?;` |
| Soak --config path may exist (warning only) | `commands/soak.rs:159-160` |
| Soak --output parent dir auto-create with `.with_context(...)` error | `commands/soak.rs:405-408` |
| `--id` for `Undo` is forwarded to journal | `src/main.rs:487-501` |
| Session `RecoveryPlan.max_age_seconds` / `Recover.max_age_seconds` use `clap::value_parser!(u64).range(1..=604800)` | `src/main.rs:520,553` |

### 7.2 What is **not** validated

| Gap | Evidence / impact |
|---|---|
| `Start <project> <harness>` does NOT verify project exists or harness is supported before spawning | `src/main.rs:126-142`; the actual check is in `commands/mod.rs:584-585` (after `current_dir().join(project).exists()`), but the failure path prints `Project path does not exist: {project_path:?}` without suggesting `sharecli project init`. |
| `Limits <project>` doesn't validate the project exists | `commands/mod.rs:1449-1474` (per `grep -n 'pub async fn set_limits'`) |
| `ProcCompose Status <file>` validates file existence | `src/main.rs:1635-1644` (good) |
| `Surface Read surface --bytes` is unchecked — clap `#[arg]` at `src/main.rs:617-619` is a comma-separated list parsed at runtime; the error path at `:1356` is `anyhow::bail!("surface send requires --text or --bytes")` but no message names which flag was set. |
| `Undo --restore --id <X>` runs dry-run only — there is no live executor. The doc string at `src/main.rs:495-498` correctly says `the actual restore executor is an out-of-scope operator action` — **good documentation of the gap, not a hole.** |

---

## 8. First-run / `--init` experience

**There is no `init`, `setup`, `bootstrap`, `doctor`, or `first-run` subcommand.**

Confirmation: `grep -nE '"(init|setup|bootstrap|doctor|first-run|login)"'` against `enum Commands` (`src/main.rs:88-502`) returns 0 hits. The only "register" hit is inside `cli_list` (`src/main.rs:1476`) which prints documentation strings for `cast`/`mesh`/`fuse` modules — not a subcommand.

**What the binary does on first run today:**

- `src/main.rs:881` calls `Cli::parse()` — a brand-new user running `sharecli` with no args sees the help output (clap default).
- `src/main.rs:895` calls `config::init_global()` which loads `~/.config/sharecli/...` or reports it as missing.
- `src/main.rs:898-903` validates the config and exits with `ExitCode(78)` if missing or invalid.

**What happens if the user then runs `sharecli start myproj claude`?**

- `src/main.rs:984-986` dispatches to `commands::start(project, …)` in `commands/mod.rs:570-622`. Function reads project path from `current_dir().join(project)`, calls `resolve_project_path` (`:580-585`), and bails with `Project path does not exist: {project_path:?}` if the directory isn't there.
- There is **no prompt**, **no `sharecli project init` or `sharecli init --project <name>` subcommand** that creates a starter project directory, scaffolds a `.sharecli/config.toml`, or helps the user find the project root.
- The `sharecli project` subcommand (defined in `src/config.rs:550` per outline) is dispatch-only at the top-level and was not opened in this audit pass — verifying its sub-commands is **UNKNOWN** (see §10).

**Severity:** Medium for new-user onboarding. Existing operators have `docs/` and the FAQ at `docs/faq.md` (referenced in `src/main.rs:65-67`), but first-time CLI users would benefit from `sharecli doctor` or `sharecli init` that:
1. Detects missing config dir, suggests creating it.
2. Detects missing session DB, suggests creation path.
3. Detects no active `sharecli serve` and suggests a one-shot.

---

## 9. Summary of UX gaps (ranked)

The five highest-impact gaps, in priority order:

1. **No `init`/`setup`/`doctor`/`bootstrap` subcommand.**
   First-time operators have no in-binary onboarding path. Improvable via a single `sharecli doctor` (or similar) that checks config dir existence, session DB, dashboard reachability, NATS coordinator reachability, and writes a one-page summary. **Evidence:** `src/main.rs:88-502` does not contain any of these subcommand names; `grep -nE '"(init\|setup\|bootstrap\|doctor\|first-run\|login)"'` returns 0 hits.

2. **`sharecli upgrade` help text advertises an "apply" path that does not exist.**
   `src/main.rs:462` says `/// Check for or apply an upstream release upgrade` but `commands/upgrade.rs:191-205` (`fn check`) only reads `<config_dir>/sharecli/upgrade.json` and emits JSON. There is no `--apply`, no network download, no binary replacement. Either the help text should be corrected to "check" or the apply path needs to be implemented. Operators expecting `sharecli upgrade` to self-update are silently mis-served.

3. **No `EXAMPLES:` sections on any subcommand help output.**
   `grep -rE 'EXAMPLES:|#[arg(long_help|#[arg(help_heading' src/` returns 0 hits. Only one `after_long_help` attribute exists (at `src/main.rs:65-67`, pointing to docs paths). Subcommands with rich flag surfaces (`fuse mount`, `cast send`, `proc`, `surface watch`, `session recover`) would benefit from a single inline example per command. **Evidence:** `grep -rE 'Example:|e\.g\.|tip:'` returns almost no hits inside `commands/`.

4. **Error-message consistency is uneven outside the `fuse` module.**
   `commands/fuse.rs` consistently uses `fuse …:` prefix + actionable hint (`:69,72,111,206,217`). `commands/config_edit.rs:17`, `commands/cast.rs:62`, `commands/mesh.rs:33`, `commands/upgrade.rs:185,191` either lack the command-name prefix, lack a fix hint, or both. The `SharecliError::eprint()` envelope at `src/error.rs:162-170` already gives a strong `SHARECLI_ERROR_CODE=...` baseline; the human-readable strings above it need a pass to match the `fuse` standard.

5. **`sharecli config set <key> <value>` and `sharecli config get <key>` accept arbitrary strings, but the error path on invalid types is opaque.**
   `commands/config_edit.rs:46-68` (`coerce`) silently degrades to a `String` if the CLI can't parse a numeric / bool (`:62-66`), and the next validation gate downstream (`config_validator.rs`) is what rejects it. The user-facing message is `config_invalid(...)` with whatever the validator produced. There's no in-place "this is a numeric field; pass an integer" hint near the call site.

(A sixth, lower-priority gap worth mentioning: `commands/serve.rs:845-846,874-875` uses `.expect("corpus scenarios dir")` and `.expect("entry")` inside the corpus-loading code path. Reaching that code requires a configuration that points at a bad path; today it will panic the process instead of returning an error. Two line-level changes would harden it.)

---

## 10. UNKNOWN items / honest scope limits

- **Body of `enum ConfigCmd` (`src/config.rs:536`) and `enum ProjectCmd` (`src/config.rs:550`)** — agentgrep outline reports they exist and have clap `Subcommand` derives, but their variant set was not enumerated by file/line in this audit pass.
- **Whether `Project` subcommand contains an `init`/`create` variant** — not opened. The top-level `enum Commands` does not, but the nested `ProjectCmd` might.
- **`sharecli_session` crate** (`sharecli-session` per deps) provides several handler-side helpers that `Commands::Session { cmd }` dispatches into via `src/main.rs:1175-1291`. The actual `SessionCmd::Register` validate path at `:1221-1245` was inspected; deeper behavior is out of scope.
- **`sharecli_mesh::MaildirQueue` and `sharecli_fuse` impl details** — referenced at `src/commands/mesh.rs:17,36`, `src/commands/fuse.rs:115`; we did not read those crates in this pass.

---

## 11. Files inspected (with line ranges)

| File | Lines inspected | Purpose |
|---|---|---|
| `src/main.rs` | 1-1810 (full) | Clap definitions, dispatch, top-level commands |
| `src/error.rs` | 1-200 (top half) | Error type, exit codes, eprint envelope |
| `src/commands/cast.rs` | 1-121 (full) | Cast command handlers |
| `src/commands/config_edit.rs` | 1-146 (full) | Dotted-path get/set helpers |
| `src/commands/fuse.rs` | grep-locate, line-range reads | Fuse handlers, error messages |
| `src/commands/history.rs` | grep-locate | History handlers |
| `src/commands/mesh.rs` | 1-40 (full) | Mesh handlers |
| `src/commands/mod.rs` | 1-1647 (head + grep outline) | ps/start/stop/status/pool/run_pool/health/set_limits |
| `src/commands/render_csv.rs` | grep-locate | CSV body builders |
| `src/commands/report.rs` | grep-locate | Report handlers |
| `src/commands/serve.rs` | grep-locate | Serve handlers |
| `src/commands/soak.rs` | grep-locate | Soak harness |
| `src/commands/undo.rs` | grep-locate | Undo journal |
| `src/commands/uninstall.rs` | 1-49 (full) | Uninstall |
| `src/commands/upgrade.rs` | grep-locate + 120-210 head | Upgrade probe/check |
| `src/commands/proc/mod.rs` | outline + handlers | Proc scan subcommand |

No files were modified during this audit.
