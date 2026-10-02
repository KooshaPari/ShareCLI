# Functional Requirements — sharecli

> Canonical FR index for the sharecli CLI. Each requirement is traceable to source
> code, tests, and acceptance criteria via `docs/specs/TRACEABILITY.md`.

**Scope:** This document defines the *minimum* set of Functional Requirements (FRs)
that the sharecli binary MUST satisfy to be considered Phase 3 complete. Additional
NFRs and design notes live in `SPEC.md` and `PRD.md`; this file is the spec-of-record
for the CLI surface.

**Conventions:**

- FR IDs are stable: `FR-NNN`. They MUST NOT be renumbered once published.
- Each FR has a single **MUST** statement, plus acceptance criteria (AC) that are
  independently testable.
- The `Source` column points at the canonical Rust source file(s) that implement
  the requirement. The `Test` column points at the acceptance test file(s) that
  cover it.
- Phase 3 only covers FR-001..FR-005. Runtime thesis FRs FR-006..FR-011 and
  operator FR-012 MAY be appended but MUST NOT renumber or rewrite existing
  entries.

---

## FR-001 — Managed Process Lifecycle (start / list / stop)

**Statement:** The CLI MUST be able to start a named process associated with a
project and harness type, list the running managed processes with optional
filtering, and stop them by PID, project, harness, or `--all`.

**Source:**

- `src/main.rs:38-91` — `Commands::Ps`, `Commands::Start`, `Commands::Stop` enum variants
- `src/commands/mod.rs:25-138` — `ps`, `start`, `stop` command implementations
- `src/runtime.rs:44-156` — `ProcessPool::spawn`, `ProcessPool::list`, `ProcessPool::kill`, `ProcessPool::kill_all`

**Acceptance Criteria:**

- **AC-001.1:** `sharecli start <project> --harness <harness>` records a process
  in the in-memory `ProcessPool` and returns a non-zero PID.
- **AC-001.2:** `sharecli ps` prints a table with columns `PID`, `NAME`, `MEM(MB)`,
  `PROJECT`, `HARNESS` plus a totals footer.
- **AC-001.3:** `sharecli ps --project <p>` returns only processes whose
  `project` field equals `<p>`.
- **AC-001.4:** `sharecli stop --all` terminates every managed process and
  reports `All processes stopped.`
- **AC-001.5:** `sharecli stop` with no selector exits with an error message
  instructing the user to specify `--pid`, `--project`, `--harness`, or `--all`.

**Test refs:** `tests/fr001_process_lifecycle.rs`, `tests/fr001_stop_filter.rs`

---

## FR-002 — TOML Configuration Management

**Statement:** The CLI MUST load, initialize, validate, and display its TOML
configuration from the platform config directory
(`$XDG_CONFIG_HOME/sharecli/config.toml` or OS equivalent), and it MUST persist
project registrations across invocations.

**Source:**

- `src/config.rs:1-119` — `Config`, `RuntimeConfig`, `Config::load`, `Config::init`, `Config::save`
- `src/commands/mod.rs:194-222` — `config` command (Init, Validate, Show, Get, Set)

**Acceptance Criteria:**

- **AC-002.1:** `sharecli config init` creates the config directory if missing
  and writes a default TOML file that round-trips through `Config::load`.
- **AC-002.2:** `sharecli config validate` reports the number of registered
  projects on success.
- **AC-002.3:** `sharecli config show` prints the serialized TOML containing
  a `[projects]` and `[runtime]` table.
- **AC-002.4:** A `Config` deserialized from TOML preserves the `projects`
  `HashMap<String, String>` and the `RuntimeConfig` fields
  (`node_path`, `bun_path`, `max_memory_mb`, `max_processes`).
- **AC-002.5:** `RuntimeConfig::default()` returns `max_memory_mb = Some(4096)`
  and `max_processes = Some(100)`.

**Test refs:** `tests/fr002_config_load.rs`, `tests/fr002_config_init.rs`

---

## FR-003 — Project Registry (add / list / show / discover / remove)

**Statement:** The CLI MUST maintain a registry of named projects (each mapping
to a filesystem path) under the `[projects]` table, and MUST support add, list,
show, remove, and discover (recursive scan for git repos) operations.

**Source:**

- `src/config.rs:8-68` — `Config.projects`, `default_projects`
- `src/commands/mod.rs:225-313` — `project` command (Add, Remove, List, Show, Discover, Generate)

**Acceptance Criteria:**

- **AC-003.1:** `sharecli project add <name> <path>` inserts a new entry into
  `Config.projects` and persists the change via `Config::save`.
- **AC-003.2:** `sharecli project list` prints one `name -> path` line per
  registered project, or the empty-state hint if none are registered.
- **AC-003.3:** `sharecli project show <name>` prints the resolved path and
  whether the path currently exists on disk.
- **AC-003.4:** `sharecli project discover [path]` scans the given directory
  and reports any subdirectory that contains a `.git` directory.
- **AC-003.5:** `sharecli project remove <name>` removes the entry from the
  `Config.projects` map and persists the change.

**Test refs:** `tests/fr003_project_registry.rs`, `tests/fr003_project_discover.rs`

---

## FR-004 — Process & Pool Health Status

**Statement:** The CLI MUST be able to report the health of managed processes
(per-harness counts, per-harness memory, system memory) and of the shared
runtime pool (node/bun totals, idle count, in-use count, max-per-type), and
MUST report per-process resource compliance.

**Source:**

- `src/runtime.rs:152-356` — `system_memory_usage`, `SharedRuntime::status`, `SharedRuntime::health_check`, `PoolStatus`, `RuntimeHealth`
- `src/monitoring.rs:1-118` — `HealthStatus`, `ProcessStats`, `MonitoringReport`, `ProcessStats::is_idle`
- `src/commands/mod.rs:140-191` — `status` command
- `src/commands/mod.rs:325-396` — `pool_status`, `health` commands

**Acceptance Criteria:**

- **AC-004.1:** `sharecli status` prints a per-harness table of `(count,
  memory_mb)` totals, followed by a shared-runtime pool table, and a
  system-memory line.
- **AC-004.2:** `sharecli pool` reports node and bun pool totals, idle
  counts, and the `max_per_type` ceiling.
- **AC-004.3:** `sharecli health [--harness <h>]` reports
  `HEALTHY`/`DEGRADED` based on whether every pooled process is still alive
  and under the 1 GB high-memory threshold.
- **AC-004.4:** `HealthStatus::mark_unhealthy(reason)` increments
  `checks_failed` and emits a `Health check failed: <reason>` message to
  stderr.
- **AC-004.5:** `ProcessStats::is_idle(threshold)` returns `true` only when
  the process has been up longer than `threshold` seconds AND `cpu_percent < 1.0`.

**Test refs:** `tests/fr004_status_health.rs`, `tests/fr004_pool_status.rs`

---

## FR-005 — Per-Project Resource Limits

**Statement:** The CLI MUST be able to set per-project memory and
max-process-count limits, persist them in-memory across calls within a single
process lifetime, and check whether the currently running processes for a
project are within those limits.

**Source:**

- `src/runtime.rs:358-455` — `ProjectLimits`, `ProjectResources::set_limits`,
  `ProjectResources::get_limits`, `ProjectResources::check_limits`, `ResourceCheck`
- `src/commands/mod.rs:398-447` — `set_limits`, `check_limits` commands

**Acceptance Criteria:**

- **AC-005.1:** `ProjectLimits::default()` returns
  `memory_limit_mb = 1024`, `max_processes = 10`, `cpu_affinity = None`.
- **AC-005.2:** `sharecli limits <project> --memory <mb> --processes <n>` sets
  the project's limits and prints a confirmation.
- **AC-005.3:** `ProjectResources::get_limits` returns the most recently
  set limits, or `ProjectLimits::default()` for unknown projects.
- **AC-005.4:** `ResourceCheck::overall_ok` is `true` only when both
  `memory_ok` and `processes_ok` are `true`.
- **AC-005.5:** `sharecli check <project>` prints memory, process count, and
  per-axis status (`OK` / `EXCEEDED`) plus an overall verdict line.

**Test refs:** `tests/fr005_project_limits.rs`, `tests/fr005_resource_check.rs`

---

## FR-006 — Agent Detection (proc scan, no bin wrap)

**Statement:** The runtime MUST discover agents via process / pattern scan
(including process-tree ancestor walk) and MUST NOT require wrapping or
replacing vendor agent executables as the primary detection path.

**Source:**

- `crates/sharecli-core/src/detect.rs` — known-agent pattern registry
- `crates/sharecli-core/src/proc_scan.rs` — `/proc` + ancestor walk
- `crates/sharecli-core` Hypervisor — observation path (direct argv)

**Acceptance Criteria:**

- **AC-006.1:** Known agent names / path basenames resolve to agent families.
- **AC-006.2:** Ordinary shells / tools are not classified as agents.
- **AC-006.3:** Hypervisor executes argv directly (no wrap).
- **AC-006.4:** Process scan lists agent PIDs only.
- **AC-006.5:** Child tool under an agent walks to the agent family.
- **AC-006.6:** Human tool under a non-agent shell is not an agent path.
- **AC-006.7:** `sharecli ps` table includes an `AGENT` column derived from
  proc-scan ancestor walk.
- **AC-006.8:** `sharecli ps --all` lists host-detected agent processes via
  [`scan_host_agents`](crates/sharecli-fleet/src/proc_scan.rs).
- **AC-006.9:** `sharecli thermal` polls [`scan_host_agents`](crates/sharecli-fleet/src/proc_scan.rs)
  on each redraw and renders a DetectedAgent inventory panel.
- **AC-006.10:** Detected agent PIDs carry live per-process resource samples
  via [`AgentResourceSample::capture_for_pid`](crates/sharecli-fleet/src/resource_watch.rs)
  / [`watch_detected_agents`](crates/sharecli-fleet/src/resource_watch.rs); thermal
  TUI agent rows and `sharecli ps --all` MUST show RSS (FD on Linux when exposed);
  dead PIDs MUST be omitted rather than silent zero.
- **AC-006.11:** `sharecli proc` lists host-detected agents with live RSS/FD samples
  and thermal gate section (parity with `ps --all` agent inventory).
- **AC-006.12:** Aggregate watched-agent RSS from [`watch_host_agents`](crates/sharecli-fleet/src/resource_watch.rs)
  escalates [`AgentAwareThermalGate`](crates/sharecli-core/src/lib.rs) via
  [`combined_agent_contention_tier`](crates/sharecli-fleet/src/agent_contention.rs)
  (default warn ≥16GiB, refuse ≥32GiB total agent RSS).
- **AC-006.13:** `sharecli proc --json` and `sharecli status --json` emit structured
  detected-agent rows (`pid`, `family`, `comm`, `mem_rss_bytes`, `fd_count`) plus
  gate fields including `agent_total_rss_bytes`.
- **AC-006.14:** Ambiguous agent `comm` names (`forge`, `goose`, `gemini`) require
  cmdline fingerprint markers in [`match_known_agent`](crates/sharecli-fleet/src/detect.rs)
  to avoid false positives from unrelated tooling.
- **AC-006.15:** `sharecli proc --watch N` (N ≥ 1) clears the terminal and
  re-renders the host agent inventory every N seconds until Ctrl-C; MUST honor
  `--json` (pretty JSON each refresh) and print a `[watch]` footer with the
  refresh interval.
- **AC-006.16:** `sharecli proc --tree` renders parent-child process forests
  rooted at top-level detected agents (nested agents appear as child subtrees;
  human-only shells are omitted). `--tree --json` emits a `forests` array of
  nested nodes (`pid`, `ppid`, `comm`, optional `family`, `children`) plus
  `roots`; `--tree --watch` re-renders the forest each refresh.
- **AC-006.17:** `sharecli proc --family <id>` and `--min-rss <size>` filter
  host agent inventory rows and `--tree` root forests (RSS suffix `K`/`M`/`G` or
  plain bytes; invalid sizes fail loudly). Gate snapshot remains host-wide;
  filtered `--json` payloads list only matching agents/roots.
- **AC-006.18:** `sharecli proc --watch --json` streams NDJSON to stdout (one
  compact JSON object per refresh with a `ts` unix timestamp plus the usual
  inventory or tree fields). Watch footer and exit messages go to stderr; stdout
  MUST stay pipe-clean (no ANSI clear, no pretty-print). One-shot `--json`
  without `--watch` remains pretty-printed multi-line JSON without `ts`.
- **AC-006.19:** `sharecli proc --sort rss|fd|pid|state` orders flat inventory rows
  and `--tree` root forests after filters apply: `rss` and `fd` descending
  (missing FD counts as zero; PID ascending tie-break), `pid` ascending,
  `state` ascending by process state letter (missing state sorts last; PID
  ascending tie-break). `--json` / NDJSON `agents` arrays and text tables MUST
  reflect the chosen order; invalid sort keys fail loudly.
- **AC-006.20:** [`match_known_agent`](crates/sharecli-fleet/src/detect.rs) adds
  the `amp` family and expands cmdline fingerprints for `codex`, `aider`, and
  `cursor-agent` (node/npx/python wrapper argv). Generic `codex-` path prefixes
  without vendor markers MUST NOT match; bare `gemini` comm remains
  fingerprint-gated per AC-006.14.
- **AC-006.21:** `sharecli proc --limit N` (N ≥ 1) caps flat inventory rows and
  tree root forests after `--family`, `--min-rss`, and `--sort`; JSON/NDJSON
  `agents` / `forests` payloads and `watched` / `roots` counts reflect the cap;
  `--limit 0` MUST fail loudly.
- **AC-006.22:** `sharecli thermal` full-layout Detected Agents panel renders
  parent-child process forests from
  [`build_host_agent_forests`](crates/sharecli-fleet/src/proc_scan.rs) with
  `├──` / `└──` connectors (parity with `sharecli proc --tree`); agent roots
  show family plus live RSS from per-PID watch; compact layout keeps the flat
  summary; empty forests fall back to flat inventory lines.
- **AC-006.23:** `sharecli proc --pid N` (N ≥ 1) prints a one-shot detail view
  for a live host process: `ppid`, parent `comm`, `cmdline`, live RSS/FD samples,
  direct agent `family` when the PID is a known agent, otherwise nearest agent
  ancestor when under an agent subtree. `--pid --json` emits a structured object;
  missing or dead PIDs MUST fail loudly; `--pid` MUST NOT combine with `--watch`.
- **AC-006.24:** `sharecli proc --csv` emits RFC 4180-style CSV of flat agent
  inventory (`pid,family,comm,mem_rss_bytes,mem_rss,fd_count`) after
  `--family`, `--min-rss`, `--sort`, and `--limit`; empty inventory emits the
  header row only; fields with commas or quotes MUST be escaped; `--csv` MUST
  NOT combine with `--json`, `--watch`, or `--pid` (without `--tree`).
- **AC-006.25:** `sharecli proc --ppid N` keeps flat inventory rows and `--tree`
  root forests whose agent parent PID equals `N`, composed with `--family`,
  `--min-rss`, `--sort`, `--limit`, `--json`, and `--csv`; `--ppid` MUST NOT
  combine with `--pid`.
- **AC-006.26:** `sharecli proc --tree --csv` emits RFC 4180-style CSV of agent
  process forests with pre-order rows
  (`root_index,depth,pid,ppid,family,comm,mem_rss_bytes,mem_rss,fd_count`) after
  `--family`, `--min-rss`, `--ppid`, `--sort`, and `--limit`; `root_index`
  separates forests, `depth` is 0 at each agent root; empty forests emit the
  header row only; `--tree --csv` MUST NOT combine with `--json`, `--watch`, or
  `--pid`.
- **AC-006.27:** `sharecli proc --max-rss <size>` keeps flat inventory rows and
  `--tree` root forests at or below the RSS bound (same `K`/`M`/`G`/bytes
  parsing as `--min-rss`), composed with `--family`, `--min-rss`, `--ppid`,
  `--sort`, `--limit`, `--json`, `--csv`, and `--tree --csv`; when both bounds
  are set, `--min-rss` MUST NOT exceed `--max-rss` (fail loudly); invalid
  sizes fail loudly.
- **AC-006.28:** `sharecli proc --min-fd N` and `--max-fd N` keep flat inventory rows and
  `--tree` root forests within the open-FD band (missing FD treated as 0), composed with
  `--family`, `--min-rss`, `--max-rss`, `--ppid`, `--sort`, `--limit`, `--json`, `--csv`,
  and `--tree --csv`; when both bounds are set, `--min-fd` MUST NOT exceed `--max-fd`
  (fail loudly); invalid counts fail loudly.
- **AC-006.29:** `sharecli proc --comm <pattern>` keeps flat inventory rows and
  `--tree` root forests whose process `comm` contains the pattern (case-insensitive
  substring), composed with `--family`, `--min-rss`, `--max-rss`, `--min-fd`,
  `--max-fd`, `--ppid`, `--sort`, `--limit`, `--json`, `--csv`, and `--tree --csv`;
  empty pattern MUST fail loudly.
- **AC-006.30:** `sharecli proc --cmdline <pattern>` keeps flat inventory rows and
  `--tree` root forests whose joined argv/cmdline contains the pattern
  (case-insensitive substring), composed with `--comm`, `--family`, `--min-rss`,
  `--max-rss`, `--min-fd`, `--max-fd`, `--ppid`, `--sort`, `--limit`, `--json`,
  `--csv`, and `--tree --csv`; empty pattern MUST fail loudly.
- **AC-006.31:** `sharecli proc --state <R|S|D|Z|…>` keeps flat inventory rows and
  `--tree` root forests whose process state letter matches (Linux `/proc` /
  sysinfo mapping), composed with `--comm`, `--cmdline`, `--family`, `--min-rss`,
  `--max-rss`, `--min-fd`, `--max-fd`, `--ppid`, `--sort`, `--limit`, `--json`,
  `--csv`, and `--tree --csv`; empty or invalid state MUST fail loudly.
- **AC-006.32:** Flat `--json` agent rows and `--csv` columns (and `--tree --csv`
  rows) include a `state` field/column with the process state letter so operators
  see state without re-scanning; composes with `--state` filter and all other proc
  export flags; missing state leaves an empty CSV field and empty JSON string.
- **AC-006.33:** Flat text inventory (`sharecli proc` table) and `proc --pid` detail
  (text + `--json`) expose the process `state` letter matching JSON/CSV parity from
  AC-006.32; missing state shows `-` in text table and empty string in JSON detail;
  composes with all proc flags.
- **AC-006.34:** `sharecli proc --tree` text nodes and `--tree --json`
  [`AgentTreeNodeJson`](src/commands/proc/mod.rs) rows include a `state` field with the
  process state letter (parity with `--tree --csv` from AC-006.32); missing state
  shows `-` on text nodes and empty string in JSON; composes with `--state` filter
  and all other proc tree flags.
- **AC-006.35:** Live tree state lookup resolves every PID in displayed agent forests
  (roots and nested children) via `collect_forest_pids` /
  `build_forest_state_map`, so `--tree` text/JSON/CSV surfaces child process state
  without relying only on scanned-agent PIDs in `state_by_pid`; composes with all
  proc tree flags including `--watch`.
- **AC-006.36:** `sharecli proc --sort state` orders flat inventory rows and
  `--tree` root forests by process state letter (ascending), composed with all
  existing filters and export flags; missing state sorts last; equal letters
  tie-break by ascending PID; `--json`, NDJSON, CSV, and text surfaces reflect
  the chosen order.
- **AC-006.37:** `sharecli proc --watch --json` NDJSON agent rows include a `state`
  field with the process state letter (parity with flat `--json` from AC-006.32);
  missing state serializes as empty string; composes with `--state` filter and all
  other proc watch/export flags.
- **AC-006.38:** `sharecli proc --exclude-family <id>` keeps flat inventory rows and
  `--tree` root forests whose agent family does NOT match `<id>` (case-insensitive,
  negation of `--family`), composed with `--comm`, `--cmdline`, `--state`,
  `--min-rss`, `--max-rss`, `--min-fd`, `--max-fd`, `--ppid`, `--sort`, `--limit`,
  `--json`, `--csv`, and `--tree --csv`; `--family` and `--exclude-family` MUST NOT
  be combined (fail loudly).
- **AC-006.39:** `sharecli thermal` full-layout Detected Agents agent-tree nodes show
  the process state letter after each PID (parity with `sharecli proc --tree` from
  AC-006.34/35): live lookup via `collect_forest_pids` /
  `build_host_forest_state_map` on each redraw; root and nested child nodes MUST
  surface state; missing state shows `-`; compact layout uses flat summary via
  `agent_lines` (state letters: AC-006.40).
- **AC-006.40:** `sharecli thermal` flat Detected Agents lines (full layout,
  compact summary, and empty-forest fallback via `agent_lines`) show the process
  state letter after each PID (parity with flat `sharecli proc` text inventory from
  AC-006.33): live lookup via `build_host_agent_state_map` when no forests are
  displayed and `build_host_forest_state_map` / pinned test state otherwise;
  missing state shows `-`; composes with tree mode (AC-006.39).
- **AC-006.41:** `ProcSort` ordering applies consistently across flat inventory
  rows, watched-agent rows (text/JSON), and `--tree` root forests ordered by live
  RSS/FD/PID/state samples (extends AC-006.19 and AC-006.36).

**Test refs:** `tests/fr006_agent_detection.rs`, `tests/fr006_proc_tree.rs`, `tests/fr006_ps_agent_column.rs`, `tests/fr006_thermal_tui_agents.rs`, `tests/fr006_thermal_tui_agent_tree.rs`, `tests/fr006_agent_pid_watch.rs`, `tests/fr006_proc_cli.rs`, `tests/fr006_proc_fingerprints.rs`, `tests/fr006_proc_fingerprints_ext.rs`, `tests/fr006_agent_rss_gate.rs`, `tests/fr006_proc_watch.rs`, `tests/fr006_proc_tree_cli.rs`, `tests/fr006_proc_filters.rs`, `tests/fr006_proc_ndjson.rs`, `tests/fr006_proc_sort.rs`, `tests/fr006_proc_limit.rs`, `tests/fr006_proc_pid_detail.rs`, `tests/fr006_proc_csv.rs`, `tests/fr006_proc_ppid.rs`, `tests/fr006_proc_tree_csv.rs`, `tests/fr006_proc_comm.rs`, `tests/fr006_proc_cmdline.rs`, `tests/fr006_proc_state.rs`, `tests/fr006_proc_state_export.rs`, `tests/fr006_proc_state_text.rs`, `tests/fr006_proc_tree_state.rs`

---

## FR-007 — Resource & Syscall-Relevant Watch

**Statement:** The runtime MUST expose resource watch signals (CPU / MEM /
Net / FD at minimum) and, when FUSE intercept is enabled, IO paths used by
coalesce. Thermal watch signals MAY surface via FR-011.

**Source:**

- `crates/sharecli-core`, `crates/sharecli-fleet`, `src/monitoring.rs`

**Acceptance Criteria:**

- **AC-007.1:** Mock thermal levels are visible via poll (watch signal).
- **AC-007.2:** Fake thermal gate decisions are stable for a level.
- **AC-007.3:** Idle heuristic encodes CPU + uptime watch signal.
- **AC-007.4:** FD watch samples the current process open descriptor count
  (`sample_self_fds` / `ResourceWatchSample::capture`); MUST fail loudly on
  unsupported OS rather than returning silent zero.
- **AC-007.5:** Host net RX/TX watch returns byte counters via
  `sample_host_net` / `ResourceWatchSample::capture`; MUST fail loudly on
  unsupported OS rather than returning silent zero.
- **AC-007.6:** [`Hypervisor::run`](crates/sharecli-core/src/lib.rs) captures
  a live [`ResourceWatchSample`](crates/sharecli-fleet/src/resource_watch.rs)
  on every invocation and attaches it to [`SpawnOutcome::resource_watch`](crates/sharecli-core/src/lib.rs).
- **AC-007.7:** RSS watch samples current process resident memory via
  `sample_self_rss_bytes` / `ResourceWatchSample::capture`; MUST fail loudly on
  unsupported OS rather than returning silent zero.
- **AC-007.8:** Host 1-minute load average is sampled via
  `sample_host_load_1m` / `ResourceWatchSample::capture`; MUST fail loudly on
  unsupported OS rather than returning silent zero.
- **AC-007.9:** `sharecli status` and `sharecli thermal` surface FUSE
  read-coalesce hit/miss meters via [`global_read_cache_meters`](crates/sharecli-fuse/src/read_cache.rs)
  / [`format_status_section`](crates/sharecli-fuse/src/read_cache.rs).
- **AC-007.10:** `sharecli status` captures a live
  [`ResourceWatchSample::capture`](crates/sharecli-fleet/src/resource_watch.rs)
  after the system-memory section and prints
  [`format_status_section`](crates/sharecli-fleet/src/resource_watch.rs)
  with open FD count, RSS bytes, 1-minute load average, and host net RX/TX
  counters; MUST fail loudly via `?` when sampling is unsupported or errors.
- **AC-007.11:** `sharecli thermal` polls a live
  [`ResourceWatchSample::capture`](crates/sharecli-fleet/src/resource_watch.rs)
  on each redraw and renders host FD/RSS/load plus FUSE read-coalesce meters
  in dedicated TUI panels (Feb harness dashboard slice); capture failure MUST
  render an explicit unavailable message (no silent zero panel).
- **AC-007.12:** `sharecli thermal` host resource watch panel
  ([`resource_watch_lines`](crates/sharecli-thermal-tui/src/lib.rs)) MUST surface
  host net RX/TX byte counters from [`ResourceWatchSample`](crates/sharecli-fleet/src/resource_watch.rs)
  in full and compact layouts, matching [`format_status_section`](crates/sharecli-fleet/src/resource_watch.rs)
  parity established by AC-007.10.
- **AC-007.13:** `sharecli proc --json` and `sharecli proc --watch --json` MUST
  emit a `host_watch` object on every snapshot with live
  [`ResourceWatchSample`](crates/sharecli-fleet/src/resource_watch.rs) fields
  (`fd_count`, `mem_rss_bytes`, `load_1m`, `net_rx_bytes`, `net_tx_bytes`) via
  [`HostResourceWatchJson`](src/monitoring.rs); MUST fail loudly via `?` when
  sampling is unsupported or errors (parity with `status` text + thermal TUI).
- **AC-007.14:** `sharecli proc` text mode and `sharecli proc --csv` (flat and
  `--tree`) MUST surface live host
  [`ResourceWatchSample`](crates/sharecli-fleet/src/resource_watch.rs) fields
  via [`HostResourceWatchJson::format_text_section`](src/monitoring.rs) (footer
  after gate section, matching `status` text) and
  [`HostResourceWatchJson::format_csv_companion`](src/monitoring.rs) (companion
  `host` CSV record after agent rows); MUST fail loudly via `?` when sampling
  is unsupported or errors (parity with JSON `host_watch` from AC-007.13).
- **AC-007.15:** `sharecli proc --tree --json` and `sharecli proc --tree --watch --json`
  MUST emit a `host_watch` object on every snapshot with live
  [`ResourceWatchSample`](crates/sharecli-fleet/src/resource_watch.rs) fields via
  [`HostResourceWatchJson`](src/monitoring.rs) on [`AgentTreeSnapshot`](src/commands/proc/mod.rs);
  MUST fail loudly via `?` when sampling is unsupported or errors (parity with flat
  JSON `host_watch` from AC-007.13).
- **AC-007.16:** `sharecli proc --pid N --json` MUST emit a `host_watch` object with live
  [`ResourceWatchSample`](crates/sharecli-fleet/src/resource_watch.rs) fields via
  [`HostResourceWatchJson`](src/monitoring.rs) on [`ProcDetailSnapshot`](src/commands/proc/mod.rs);
  `sharecli proc --pid N` text detail MUST append the host watch footer via
  [`HostResourceWatchJson::format_text_section`](src/monitoring.rs) (parity with AC-007.14);
  MUST fail loudly via `?` when sampling is unsupported or errors.
- **AC-007.17:** `sharecli proc --pid N --json` MUST emit a `gate` object with live
  [`GateStatusSnapshot`](crates/sharecli-fleet/src/agent_contention.rs) fields
  (`thermal_pressure`, `detected_agents`, `agent_total_rss_bytes`, `agent_contention`,
  `gate_decision`) from proc-scan agent inventory + thermal poll on
  [`ProcDetailSnapshot`](src/commands/proc/mod.rs); `sharecli proc --pid N` text detail MUST
  print [`format_gate_status_from_snapshot`](crates/sharecli-fleet/src/agent_contention.rs)
  after process fields and before the host watch footer (parity with flat `gate` from
  AC-006.13 and text gate section from AC-006.11).
- **AC-007.18:** `sharecli proc --tree --json` and `sharecli proc --tree --watch --json`
  MUST emit a `gate` object with live
  [`GateStatusSnapshot`](crates/sharecli-fleet/src/agent_contention.rs) fields on
  [`AgentTreeSnapshot`](src/commands/proc/mod.rs) (parity with flat `gate` from AC-006.13 and
  pid `gate` from AC-007.17); MUST fail loudly via `?` when thermal poll errors.
- **AC-007.19:** `sharecli proc --csv` (flat and `--tree`) MUST append a companion
  `gate` CSV record after agent rows and before the `host` companion from AC-007.14 via
  [`GateStatusSnapshot::format_csv_companion`](crates/sharecli-fleet/src/agent_contention.rs)
  (parity with text gate section from AC-006.11 and JSON `gate` from AC-006.13 / AC-007.18);
  MUST fail loudly via `?` when thermal poll errors.
- **AC-007.20:** `sharecli proc --tree` text mode MUST print
  [`format_gate_status_section`](crates/sharecli-fleet/src/agent_contention.rs) after the
  forest inventory and before the host watch footer from AC-007.14 (parity with flat text
  gate section from AC-006.11 and pid detail gate ordering from AC-007.17); MUST fail loudly
  via `?` when thermal poll errors.
- **AC-007.21:** `sharecli proc` flat text mode MUST print
  [`format_gate_status_section`](crates/sharecli-fleet/src/agent_contention.rs) after the
  agent inventory and before the host watch footer from AC-007.14 (parity with tree text
  gate ordering from AC-007.20 and pid detail gate ordering from AC-007.17); MUST fail loudly
  via `?` when thermal poll errors.
- **AC-007.22:** `sharecli proc --watch` text mode and `sharecli proc --watch --json`
  (NDJSON) MUST preserve gate → `host_watch` ordering on every refresh cycle (parity with
  one-shot flat text from AC-007.21 and NDJSON `gate`/`host_watch` from AC-006.13 /
  AC-007.13); MUST fail loudly via `?` when thermal poll or host sampling errors.
- **AC-007.23:** `sharecli proc --tree --watch` text mode and `sharecli proc --tree --watch --json`
  (NDJSON) MUST preserve gate → `host_watch` ordering on every refresh cycle (parity with
  flat watch from AC-007.22 and one-shot tree text from AC-007.20); MUST fail loudly via `?`
  when thermal poll or host sampling errors.
- **AC-007.24:** `sharecli proc --json`, `sharecli proc --tree --json`, and
  `sharecli proc --pid N --json` one-shot JSON MUST serialize `"gate"` before `"host_watch"`
  in raw output (parity with watch NDJSON ordering from AC-007.22 / AC-007.23); serde field
  order on [`AgentProcSnapshot`](src/commands/proc/mod.rs), [`AgentTreeSnapshot`](src/commands/proc/mod.rs),
  and [`ProcDetailSnapshot`](src/commands/proc/mod.rs) is the contract.
- **AC-007.25:** `sharecli status --json` MUST emit top-level `gate` and `host_watch` siblings
  with the same shapes as `sharecli proc --json` (AC-007.13), plus flat `agents` array,
  `scanned`, `watched`, and `total_processes`; MUST NOT nest the full
  [`AgentProcSnapshot`](src/commands/proc/mod.rs) under `agents`; raw JSON MUST serialize
  `"gate"` before `"host_watch"` (parity with AC-007.24).
- **AC-007.26:** `sharecli thermal` gate decision panel MUST derive ADMIT/DENY from
  [`gate_status_snapshot_with_rss`](crates/sharecli-fleet/src/agent_contention.rs) using live
  `detected_agents` inventory (count + summed RSS from watched agents), surfacing
  `agent_total_rss_bytes` and `agent_contention` in full and compact layouts — parity with
  `sharecli proc` / `status --json` gate (AC-006.13 / AC-007.25); MUST NOT use count-only
  [`effective_gate_decision`](crates/sharecli-fleet/src/agent_contention.rs).
- **AC-007.27:** `sharecli status` text MUST print the thermal gate section before the host
  resource watch section (parity with `sharecli proc` flat text from AC-007.21 and
  `sharecli status --json` gate → `host_watch` ordering from AC-007.25).
- **AC-007.28:** `sharecli proc --watch --json` (NDJSON) MUST print gate and host watch
  text companion sections on **stderr** in gate → `host_watch` order on every refresh;
  stdout MUST remain pipe-clean (valid NDJSON lines only — no gate/host_watch text
  companions, no `[watch]` footer, no ANSI clear sequences; parity with AC-006.18).
- **AC-007.29:** `sharecli proc --tree --watch --json` (NDJSON) MUST print gate and host watch
  text companion sections on **stderr** in gate → `host_watch` order on every refresh via
  the same `eprint_gate_host_watch_stderr_companions` helper as AC-007.28; stdout MUST remain
  pipe-clean (valid NDJSON lines only — no gate/host_watch text companions, no `[watch]`
  footer, no ANSI clear sequences; parity with AC-006.18 tree watch NDJSON).
- **AC-007.30:** `sharecli proc --json` and `sharecli proc --tree --json` (one-shot, no
  `--watch`) MUST NOT print gate or host watch text companion sections on **stderr**; stderr
  MUST be empty on success (errors only on failure). Gate and `host_watch` MUST appear only
  in the JSON body (inverse contract of AC-007.28 / AC-007.29).
- **AC-007.31:** `sharecli proc --pid N --json` (one-shot, no `--watch`) MUST NOT print gate
  or host watch text companion sections on **stderr**; stderr MUST be empty on success (errors
  only on failure). Gate and `host_watch` MUST appear only in the JSON body on
  [`ProcDetailSnapshot`](src/commands/proc/mod.rs) (parity with AC-007.30; inverse contract of
  watch NDJSON stderr companions).
- **AC-007.32:** `sharecli status --json` (one-shot) MUST NOT print gate or host watch text
  companion sections on **stderr**; stderr MUST be empty on success (errors only on failure).
  Gate and `host_watch` MUST appear only in the JSON body (parity with AC-007.30 / AC-007.31;
  extends AC-007.25 JSON shape contract with pipe-clean stderr).
- **AC-007.33:** `sharecli proc --csv` and `sharecli proc --tree --csv` (one-shot, no
  `--watch`) MUST NOT print gate or host watch text companion sections on **stderr**; stderr
  MUST be empty on success (errors only on failure). Gate and `host_watch` MUST appear only
  in CSV companion rows on stdout (parity with AC-007.30 / AC-007.31 / AC-007.32; extends
  AC-007.19 CSV companion contract with pipe-clean stderr).
- **AC-007.34:** `sharecli proc`, `sharecli proc --tree`, and `sharecli proc --pid N`
  (one-shot, no `--watch`) MUST NOT print gate or host watch text companion sections on
  **stderr**; stderr MUST be empty on success (errors only on failure). Gate and `host_watch`
  MUST appear only in text sections on stdout (parity with AC-007.30 / AC-007.31 / AC-007.32 /
  AC-007.33; extends AC-007.17 / AC-007.20 / AC-007.21 text gate ordering with pipe-clean
  stderr).
- **AC-007.35:** `sharecli proc --watch` and `sharecli proc --tree --watch` (text mode, no
  `--json`) MUST NOT print gate or host watch text companion sections on **stderr** during
  refresh cycles; stderr MUST be empty on success (errors only on failure). Gate, `host_watch`,
  and the `[watch]` refresh footer MUST appear only on **stdout** (inverse contract of
  AC-007.28 / AC-007.29 NDJSON stderr companions; extends AC-007.34 to watch text refresh).
- **AC-007.36:** `sharecli status` (one-shot, no `--json`) MUST NOT print gate or host watch
  text companion sections on **stderr**; stderr MUST be empty on success (errors only on
  failure). Gate and `host_watch` MUST appear only in text sections on stdout (parity with
  AC-007.30 / AC-007.31 / AC-007.32 / AC-007.34; extends AC-007.27 text gate ordering with
  pipe-clean stderr).
- **AC-007.37:** `sharecli health` and `sharecli pool` (one-shot text) MUST print gate →
  `host_watch` text sections on **stdout** after pool/runtime health output (parity with
  `status` / AC-007.36 and `proc` / AC-007.34). **stderr** MUST be empty on success (errors
  only on failure); gate and `host_watch` MUST NOT appear on stderr.
- **AC-007.38:** `sharecli ps --all` (one-shot text) MUST print gate → `host_watch` text
  sections on **stdout** after host agent inventory (parity with AC-007.37 health/pool and
  AC-011.6 gate). **stderr** MUST be empty on success (errors only on failure); gate and
  `host_watch` MUST NOT appear on stderr.
- **AC-007.39:** `sharecli report` (text, one-shot and `--watch` refresh) MUST print gate →
  `host_watch` text sections on **stdout** after the report body (parity with AC-007.38
  ps --all and AC-011.6 gate). **stderr** MUST be empty on success (errors only on failure);
  gate and `host_watch` MUST NOT appear on stderr.
- **AC-007.40:** `sharecli report --format json` MUST emit top-level `gate` + `host_watch`
  JSON siblings after fleet analytics fields (parity with `status --json` AC-007.25 / proc JSON
  AC-007.24 key order: `gate` before `host_watch`). **stderr** MUST be empty on success
  (errors only on failure); gate and `host_watch` MUST NOT appear on stderr.
- **AC-007.41:** `sharecli serve` WebSocket `/ws` periodic snapshots MUST emit top-level
  `gate` and `host_watch` siblings with the same shapes as `sharecli status --json`
  (AC-007.25), plus a compact `agents` summary (`scanned`, `watched`, `total_rss_bytes`,
  `families`) and the existing `processes` pool array; raw JSON MUST serialize `"gate"` before
  `"host_watch"` (parity with AC-007.24 / AC-007.25); embedded dashboard MUST render
  functional operator panels from the envelope (not decorative cards).
- **AC-007.42:** `sharecli report --watch --format json` streams NDJSON to stdout (one
  compact JSON object per refresh with a `ts` unix timestamp plus fleet analytics fields and
  live `gate` + `host_watch` siblings). Each line MUST serialize `"gate"` before
  `"host_watch"` (parity with AC-007.40 / AC-007.24). stderr MUST print gate → `host_watch`
  text companion sections and the `[watch]` footer on every refresh (parity with
  AC-007.28). stdout MUST remain pipe-clean (valid NDJSON lines only — no gate/host_watch
  text companions, no `[watch]` footer, no ANSI clear sequences). One-shot
  `--format json` without `--watch` remains pretty-printed multi-line JSON without `ts`
  (AC-007.40); MUST NOT print stderr companions.
- **AC-007.43:** `sharecli ps --all --json` MUST emit top-level `gate` + `host_watch` JSON
  siblings after managed pool fields (`processes`, `total_memory_mb`) and host agent inventory
  fields (`agents`, `scanned`, `watched` from [`AgentProcSnapshot::capture()`](src/commands/proc/mod.rs))
  (parity with `status --json` AC-007.25 / `ps --all` text AC-007.38). Raw JSON MUST serialize
  `"gate"` before `"host_watch"`. **stderr** MUST be empty on success (errors only on failure);
  gate and `host_watch` MUST NOT appear on stderr. `sharecli ps --json` without `--all` MUST
  fail loudly (no pool-only JSON shortcut).
- **AC-007.44:** `sharecli health --json` and `sharecli pool --json` MUST emit top-level `gate` +
  `host_watch` JSON siblings after their respective runtime health / pool status fields (parity
  with `status --json` AC-007.25 / `ps --all --json` AC-007.43). Raw JSON MUST serialize
  `"gate"` before `"host_watch"`. **stderr** MUST be empty on success (errors only on failure);
  gate and `host_watch` MUST NOT appear on stderr.
- **AC-007.45:** IPC `health.status` / [`HealthSnapshot`](crates/sharecli-ipc/src/handler.rs)
  MUST emit top-level `gate` + `host_watch` JSON siblings after runtime health fields (parity
  with `health --json` AC-007.44). Raw JSON MUST serialize `"gate"` before `"host_watch"`.
  Tray/desktop consumers (`sharecli-tray-linux`, Swift `IPCClient`) decode the same wire shape
  without shelling out to `sharecli health --json`.
- **AC-007.46:** IPC `monitoring.report` / [`MonitoringReportSnapshot`](crates/sharecli-ipc/src/handler.rs)
  MUST emit top-level `gate` + `host_watch` JSON siblings after fleet monitoring fields (parity
  with `report --format json` AC-007.40 and `health.status` AC-007.45). Raw JSON MUST serialize
  `"gate"` before `"host_watch"`.
- **AC-007.47:** Tray/desktop consumers (`sharecli-tray-linux`, Swift `IPCClient`) decode
  `MonitoringReportSnapshot` from IPC `monitoring.report` via `monitoring_report()` /
  `monitoringReport()` RPC helpers (parity with AC-007.45 `HealthSnapshot` wiring). Wire unit
  tests MUST decode `gate` + `host_watch` without shelling out to `sharecli report --format json`.
- **AC-007.48:** Tray/desktop refresh loops (`AppState.refresh`, Linux tray `refresh`) MUST
  consume a single `monitoring.report` snapshot per poll to drive operator gate/host_watch +
  managed-process inventory (parity with dashboard/report operator panels). Split
  `health.status` + `process.list` polls MUST NOT be used for operator refresh paths.
  Mapping helpers derive tray `HealthSnapshot` + process rows from `MonitoringReportSnapshot`
  fields (`total_processes`, `processes`, `gate`, `host_watch`).
- **AC-007.49:** `sharecli ps --all --watch --json` streams NDJSON to stdout (one compact JSON
  object per refresh with a `ts` unix timestamp plus managed pool fields, host agent inventory
  fields, and live `gate` + `host_watch` siblings). Each line MUST serialize `"gate"` before
  `"host_watch"` (parity with AC-007.43 / AC-007.42). stderr MUST print gate → `host_watch`
  text companion sections and the `[watch]` footer on every refresh (parity with
  AC-007.42). stdout MUST remain pipe-clean (valid NDJSON lines only — no gate/host_watch
  text companions, no `[watch]` footer, no ANSI clear sequences). One-shot
  `ps --all --json` without `--watch` remains pretty-printed multi-line JSON without `ts`
  (AC-007.43); MUST NOT print stderr companions. `sharecli ps --watch --json` without `--all`
  MUST fail loudly.
- **AC-007.50:** `sharecli ps --all --watch` (text mode, no `--json`) MUST NOT print gate or
  host_watch text companion sections on **stderr** during refresh cycles; stderr MUST be empty
  on success (errors only). Gate → `host_watch` text sections and the `[watch]` footer MUST
  appear on **stdout** only (parity with AC-007.35 proc text watch stderr silence; inverse
  contract of AC-007.49 NDJSON stderr companions; extends AC-007.38 one-shot ps --all text).
- **AC-007.51:** Windows WinUI tray refresh (`TrayWindow.RefreshDataAsync`) MUST consume a
  single `monitoring.report` snapshot per poll to drive operator gate/host_watch + managed-process
  inventory (parity with Linux tray + Swift `AppState.refresh` AC-007.48). Split
  `health.status` + `process.list` polls MUST NOT be used for operator refresh paths.
  Mapping helpers on `MonitoringReportSnapshot` derive tray health + process rows from
  `total_processes`, `processes`, `gate`, and `host_watch` (`sharecli-tray-windows`, C#
  `MonitoringReportSnapshot.cs`).
- **AC-007.52:** Windows WinUI tray (`TrayWindow`) MUST poll on a ~3 s cadence (parity with
  Linux tray `POLL_INTERVAL` and Swift `AppState.startPolling`). A `DispatcherQueueTimer` (or
  equivalent) MUST invoke the same AC-007.51 `RefreshDataAsync` / `monitoring.report` refresh
  path on each tick (not launch + manual Refresh only). Poll interval MUST be centralized in
  `sharecli-tray-windows` (`TRAY_POLL_INTERVAL_SECS`) and mirrored in C# `TrayPoll.IntervalSeconds`;
  unit tests MUST prove interval wiring.
- **AC-007.53:** Linux tray (`sharecli-tray-linux`) and Swift `AppState.startPolling` MUST use
  the same `TRAY_POLL_INTERVAL_SECS = 3` cadence as Windows AC-007.52. Poll interval MUST be
  centralized in `sharecli-tray-linux` (`poll.rs`) and Swift `TrayPoll.intervalSeconds`; the Linux
  tray sleep loop and Swift `Task.sleep` MUST reference those shared constants (not inline `3` /
  `3_000_000_000`). Unit tests MUST prove Linux/Swift wiring parity with
  `tests/fr007_tray_windows_poll_interval.rs`.
- **AC-007.54:** Windows WinUI tray process actions MUST wire per-process **Kill** and
  **Kill All Managed** to IPC `process.kill` / `process.kill_all` (parity with Linux tray
  `ipc::kill`/`kill_all` and Swift `AppState.kill`/`killAll`). Kill params MUST use
  `{ "pid": <u32> }` for single kill and `{}` for kill_all. After kill, tray MUST refresh via
  the AC-007.51 `RefreshDataAsync` / `monitoring.report` path (Swift/Linux refresh parity).
  Unit tests MUST prove Windows/Linux/Swift kill wiring (`tests/fr007_tray_windows_kill.rs`,
  `tests/fr007_tray_linux_kill.rs`, `tests/fr007_tray_swift_kill.rs`).
- **AC-007.55:** Windows WinUI tray process grid MUST surface **harness** per managed process
  (parity with Linux tray submenu `Harness:` label and Swift `DashboardView` Harness column).
  `MonitoringReportSnapshot.AsProcessSummaries()` MUST map `MonitoringProcessEntry.harness`
  into tray `ProcessInfo.harness`; `TrayWindow` DataGrid MUST bind a Harness column. Rust
  `sharecli-tray-windows` `process_summaries()` already maps harness — C# + XAML MUST match.
  Unit tests MUST prove harness flows from `monitoring.report` wire into Windows tray rows
  (`tests/fr007_tray_windows_harness.rs`, `tests/fr007_tray_windows_monitoring_report_consume.rs`).
- **AC-007.56:** Linux, Swift, and Windows tray UIs MUST surface thermal gate + host_watch
  operator metadata from each `monitoring.report` refresh (parity with dashboard operator
  panels and proc/status text sections). Tray views MUST show at minimum: gate decision,
  thermal pressure, detected agents, agent contention, agent RSS; host load (1m), FD count,
  self RSS, net RX/TX. Format strings MUST live in reusable helpers testable without GUI:
  `sharecli-tray-linux` / `sharecli-tray-windows` `operator_display` modules,
  Swift `OperatorDisplay.swift`, C# `OperatorDisplay.cs`. Linux tray menu + tooltip,
  Swift `TrayPopoverView` + `HealthView`, and WinUI `TrayWindow` gate/host rows MUST call
  those helpers (not ad-hoc partial fields). Unit tests MUST prove format mapping and UI
  wiring (`tests/fr007_tray_gate_host_watch_ui.rs`).
- **AC-007.57:** Linux, Swift, and Windows tray icon / badge / color MUST derive from
  `gate.thermal_pressure` + `gate_decision` on each `monitoring.report` refresh (parity
  with dashboard `#thermal-status` + gate decision CSS: `gate-admit` / `gate-deny` /
  `gate-unavailable`). Visual severity→token mapping MUST live in reusable helpers testable
  without GUI: `sharecli-tray-linux` / `sharecli-tray-windows` `operator_display`
  (`resolve_tray_gate_visual`, `TrayGateVisual`), Swift `OperatorDisplay.resolveTrayGateVisual`,
  C# `OperatorDisplay.ResolveTrayGateVisual`. Linux tray MUST drive `icon_name` +
  `IconStatus::NeedsAttention` from the visual; Swift menu bar icon + popover thermal badge;
  WinUI `ThermalBadgeText` + colored gate row. Unit tests MUST prove golden severity matrix
  and UI wiring (`tests/fr007_tray_thermal_visual.rs`).
- **AC-007.58:** Swift `HealthView` (`DashboardView`) metric cards MUST derive icon/badge/color
  from `OperatorDisplay.resolveTrayGateVisual` on each `monitoring.report` refresh (parity
  with AC-007.57 tray popover thermal badge + dashboard `#thermal-status` CSS tokens). The
  generic `healthy`/`warning` Status card MUST be replaced with thermal gate severity tokens
  (`gate-admit` / `gate-deny` / `gate-unavailable`, warning/critical `#3fb950` / `#d29922` /
  `#f85149`). Thermal gate detail rows MUST use severity foreground colors + badge chip.
  Unit tests MUST prove HealthView wiring (`tests/fr007_tray_thermal_visual.rs`).
- **AC-007.59:** Swift `TrayPopoverView` stats row Status cell MUST derive icon/value/color
  from `OperatorDisplay.resolveTrayGateVisual` (same `gateVisual` as AC-007.57 header badge)
  on each `monitoring.report` refresh. The generic `healthy`/`warning` Status stat cell MUST
  be replaced with thermal gate severity tokens for full Swift tray surface parity with
  `HealthView` (AC-007.58). Unit tests MUST prove popover stats row wiring
  (`tests/fr007_tray_thermal_visual.rs`).
- **AC-007.60:** WinUI `TrayWindow` `HealthStatusText` summary line MUST derive label/color
  from `OperatorDisplay.ResolveTrayGateVisual` on each `monitoring.report` refresh (parity
  with Swift AC-007.58/59). The generic `healthy ? "✓ OK" : "✗ Unhealthy"` summary MUST be
  replaced with thermal gate severity tokens via reusable `FormatHealthStatusLine` /
  `FormatHealthStatusOfflineLine` helpers. Unit tests MUST prove HealthStatusText wiring
  (`tests/fr007_tray_thermal_visual.rs`).
- **AC-007.61:** Linux `sharecli-tray` SNI `tool_tip` summary line MUST derive severity from
  `gate_visual.badge_label` on each `monitoring.report` refresh (parity with Swift AC-007.59
  and Windows AC-007.60). The generic `healthy ? "" : " · UNHEALTHY"` suffix MUST be replaced
  with thermal gate severity tokens via reusable `format_tray_tooltip_summary_line` /
  `format_tray_tooltip_offline_line` helpers. Unit tests MUST prove Linux tooltip wiring
  (`tests/fr007_tray_thermal_visual.rs`).
- **AC-007.62:** Swift `AppEntry` NSStatusItem `button.title` MUST derive severity from
  `gateVisual.badgeLabel` on each `monitoring.report` refresh (parity with Linux AC-007.61
  tooltip and Windows AC-007.60 `HealthStatusText`). The bare `managed | memoryM` title and
  generic `" offline"` fallback MUST be replaced with thermal gate severity tokens via reusable
  `formatMenuBarTitleLine` / `formatMenuBarTitleOfflineLine` helpers. Unit tests MUST prove
  Swift menu bar title wiring (`tests/fr007_tray_thermal_visual.rs`).
- **AC-007.63:** Linux `sharecli-tray` SNI menu header row MUST derive severity from
  `gate_visual.badge_label` on each `monitoring.report` refresh (parity with Linux AC-007.61
  tooltip and Swift AC-007.62 menu bar title). The bare `process(es) · used / total MB` header
  and generic `Daemon offline` fallback MUST be replaced with thermal gate severity tokens via
  reusable `format_tray_menu_header_line` / `format_tray_menu_header_offline_line` helpers.
  Unit tests MUST prove Linux menu header wiring (`tests/fr007_tray_thermal_visual.rs`).
- **AC-007.64:** `sharecli health --watch` (text mode, no `--json`) MUST NOT print gate or
  host_watch text companion sections on **stderr** during refresh cycles; stderr MUST be empty
  on success (errors only). Gate → `host_watch` text sections and the `[watch]` footer MUST
  appear on **stdout** only (parity with AC-007.50 ps text watch stderr silence; extends
  AC-007.37 one-shot health text). `sharecli health --watch --json` streams NDJSON to stdout
  (one compact JSON object per refresh with a `ts` unix timestamp plus runtime health fields
  and live `gate` + `host_watch` siblings). Each line MUST serialize `"gate"` before
  `"host_watch"` (parity with AC-007.44 / AC-007.42). stderr MUST print gate → `host_watch`
  text companion sections and the `[watch]` footer on every refresh. stdout MUST remain
  pipe-clean (valid NDJSON lines only — no gate/host_watch text companions, no `[watch]`
  footer, no ANSI clear sequences). One-shot `health --json` without `--watch` remains
  pretty-printed multi-line JSON without `ts` (AC-007.44); MUST NOT print stderr companions.
- **AC-007.65:** `sharecli pool --watch` (text mode, no `--json`) MUST NOT print gate or
  host_watch text companion sections on **stderr** during refresh cycles; stderr MUST be empty
  on success (errors only). Gate → `host_watch` text sections and the `[watch]` footer MUST
  appear on **stdout** only (parity with AC-007.64 health text watch; extends AC-007.37
  one-shot pool text). `sharecli pool --watch --json` streams NDJSON to stdout (one compact
  JSON object per refresh with a `ts` unix timestamp plus pool status fields and live `gate` +
  `host_watch` siblings). Each line MUST serialize `"gate"` before `"host_watch"` (parity with
  AC-007.44 / AC-007.64). stderr MUST print gate → `host_watch` text companion sections and
  the `[watch]` footer on every refresh. stdout MUST remain pipe-clean (valid NDJSON lines
  only — no gate/host_watch text companions, no `[watch]` footer, no ANSI clear sequences).
  One-shot `pool --json` without `--watch` remains pretty-printed multi-line JSON without `ts`
  (AC-007.44); MUST NOT print stderr companions.
- **AC-007.66:** `sharecli status --watch` (text mode, no `--json`) MUST NOT print gate or
  host_watch text companion sections on **stderr** during refresh cycles; stderr MUST be empty
  on success (errors only). Gate → `host_watch` text sections and the `[watch]` footer MUST
  appear on **stdout** only (parity with AC-007.65 pool text watch; extends AC-007.36
  one-shot status text). `sharecli status --watch --json` streams NDJSON to stdout (one compact
  JSON object per refresh with a `ts` unix timestamp plus status fields and live `gate` +
  `host_watch` siblings). Each line MUST serialize `"gate"` before `"host_watch"` (parity with
  AC-007.25 / AC-007.65). stderr MUST print gate → `host_watch` text companion sections and
  the `[watch]` footer on every refresh. stdout MUST remain pipe-clean (valid NDJSON lines
  only — no gate/host_watch text companions, no `[watch]` footer, no ANSI clear sequences).
  One-shot `status --json` without `--watch` remains pretty-printed multi-line JSON without `ts`
  (AC-007.25); MUST NOT print stderr companions.
- **AC-007.67:** IPC `pool.status` / [`PoolSnapshot`](crates/sharecli-ipc/src/handler.rs) and
  `status.snapshot` / [`StatusSnapshot`](crates/sharecli-ipc/src/handler.rs) MUST emit top-level
  `gate` + `host_watch` siblings after pool/status fields (parity with `pool --json` AC-007.44 and
  `status --json` AC-007.25). Raw JSON MUST serialize `"gate"` before `"host_watch"` (parity with
  `health.status` AC-007.45 / `monitoring.report` AC-007.46). Unknown method errors MUST fail loudly.
- **AC-007.68:** Tray/desktop consumers (`sharecli-tray-linux`, `sharecli-tray-windows`,
  Swift `IPCClient`, C# `PoolStatusSnapshot.cs`) decode `PoolSnapshot` / `StatusSnapshot` from
  IPC `pool.status` / `status.snapshot` via `pool_status()` / `poolStatus()` and
  `status_snapshot()` / `statusSnapshot()` RPC helpers (parity with AC-007.47
  `MonitoringReportSnapshot` wiring). Wire unit tests MUST decode `gate` + `host_watch` without
  shelling out to `sharecli pool --json` / `sharecli status --json`.
- **AC-007.69:** Tray/desktop refresh loops MUST surface dedicated pool + proc-scan status
  lines via `format_pool_tray_line` / `format_status_snapshot_tray_line` helpers without replacing
  `monitoring.report` as the primary gate/host_watch + process inventory source (supplementary
  `pool.status` / `status.snapshot` IPC superseded by embedded siblings in AC-007.72).
- **AC-007.70:** `sharecli serve` WebSocket `/ws` periodic snapshots MUST emit top-level `pool` +
  `status` siblings (same shapes as `pool --json` AC-007.44 / `status --json` AC-007.25 and IPC
  `pool.status` / `status.snapshot` AC-007.67) alongside the existing gate → host_watch → agents
  → `processes` envelope (AC-007.41). Raw JSON MUST serialize keys in order:
  `"gate"` → `"host_watch"` → `"pool"` → `"status"` → `"agents"` → `"processes"`. Embedded
  dashboard operator panels MUST render functional pool + proc-scan fields from the envelope (parity
  with tray `format_pool_tray_line` / `format_status_snapshot_tray_line` AC-007.69).
- **AC-007.71:** `sharecli thermal` TUI MUST surface dedicated runtime pool + proc-scan status
  operator panels using the same field shapes as `pool --json` AC-007.44 / `status --json` AC-007.25
  (via [`PoolOperatorPanel`](crates/sharecli-fleet/src/operator_pool_status.rs) /
  [`StatusOperatorPanel`](crates/sharecli-fleet/src/operator_pool_status.rs) and
  [`run_with_pool_status`](crates/sharecli-thermal-tui/src/lib.rs)). Panel formatters MUST stay
  byte-identical to tray AC-007.69 operator lines; keyboard focus MUST include pool (digit `2`) and
  status (digit `3`) without displacing gate/host-watch/agents navigation.
- **AC-007.72:** IPC `monitoring.report` / [`MonitoringReportSnapshot`](crates/sharecli-ipc/src/handler.rs)
  MUST embed top-level `pool` + `status` siblings (same shapes as `pool.status` / `status.snapshot`
  AC-007.67) after `gate` → `host_watch` within the envelope (parity with dashboard WS AC-007.70 key
  order). Raw JSON MUST serialize `"gate"` → `"host_watch"` → `"pool"` → `"status"`. Tray/desktop
  refresh (Linux/Swift/Windows AC-007.48 / AC-007.51) MUST consume embedded `pool` + `status` from
  the single `monitoring.report` round-trip and MUST NOT call supplementary `pool.status` /
  `status.snapshot` IPC during refresh.
- **AC-007.73:** `sharecli report --format json` / [`FleetReportJson`](src/commands/report.rs) MUST
  embed top-level `pool` + `status` siblings (same shapes as `pool --json` / `status --json`
  AC-007.44) after `gate` → `host_watch` within the envelope (parity with `monitoring.report`
  AC-007.72 / dashboard WS AC-007.70 key order). Raw JSON MUST serialize `"gate"` → `"host_watch"`
  → `"pool"` → `"status"`. `sharecli report --watch --format json` NDJSON lines MUST carry the same
  embedded siblings on every refresh (AC-007.42 watch path). stderr MUST remain silent on one-shot
  success (AC-007.40); watch NDJSON stderr companions remain gate → host_watch text only (AC-007.42).
- **AC-007.74:** `sharecli report` (text, one-shot and `--watch` refresh) MUST print pool +
  proc-scan operator lines on **stdout** after gate → `host_watch` text sections (parity with
  AC-007.39 gate ordering extended by AC-007.73 JSON siblings). Lines MUST use
  [`format_pool_operator_line`](crates/sharecli-fleet/src/operator_pool_status.rs) /
  [`format_status_operator_line`](crates/sharecli-fleet/src/operator_pool_status.rs) via
  [`print_live_pool_status_operator_sections`](src/commands/mod.rs). **stderr** MUST be empty on
  success (errors only); pool/proc-scan lines MUST NOT appear on stderr.
- **AC-007.75:** `sharecli proc` and `sharecli proc --tree` (text, one-shot and `--watch`
  refresh) MUST print pool + proc-scan operator lines on **stdout** after gate → `host_watch`
  text sections (parity with AC-007.74 report text path; extends AC-007.34 / AC-007.35 gate
  ordering). Lines MUST use the same
  [`print_live_pool_status_operator_sections`](src/commands/mod.rs) helper. **stderr** MUST be
  empty on success (errors only); pool/proc-scan lines MUST NOT appear on stderr.
- **AC-007.76:** `sharecli health`, `sharecli pool`, `sharecli status`, and `sharecli ps --all`
  (text, one-shot and `--watch` refresh) MUST print pool + proc-scan operator lines on **stdout**
  after gate → `host_watch` text sections (parity with AC-007.74/75 report/proc text paths).
  Lines MUST use
  [`print_live_pool_status_operator_sections`](src/commands/mod.rs). **stderr** MUST be empty on
  success (errors only); pool/proc-scan lines MUST NOT appear on stderr. `ps --all` without
  `--watch`/`--json` MUST emit operator lines only when `--all` is set (not bare `ps`).
- **AC-007.77:** `sharecli health --json`, `sharecli ps --all --json`, and `sharecli proc --json`
  (plus `proc --tree --json` watch NDJSON) MUST embed top-level `pool` + `status` siblings after
  `gate` → `host_watch` (parity with `report --format json` AC-007.73 / dashboard WS AC-007.70 key
  order). `sharecli pool --json` MUST embed nested `status` only (top-level fields already are the
  pool panel — no redundant nested `pool`). `sharecli status --json` MUST embed nested `pool` only
  (top-level fields already are the proc-scan panel — no redundant nested `status`). Nested
  siblings use the same shapes as `build_pool_json` / `build_status_json` without cross-sibling
  recursion. `--watch --json` NDJSON lines MUST carry the same embedded siblings on every refresh
  (AC-007.64..66 / AC-007.49 watch paths). stderr MUST remain silent on one-shot success.
- **AC-007.78:** IPC `health.status` / [`HealthSnapshot`](crates/sharecli-ipc/src/handler.rs),
  `pool.status` / [`PoolSnapshot`](crates/sharecli-ipc/src/handler.rs), and `status.snapshot` /
  [`StatusSnapshot`](crates/sharecli-ipc/src/handler.rs) MUST embed operator `pool` / `status`
  siblings after `gate` → `host_watch` (parity with CLI `--json` AC-007.77). `health.status` MUST
  embed top-level `pool` + `status` siblings. `pool.status` MUST embed nested `status` only (no
  redundant nested `pool`). `status.snapshot` MUST embed nested `pool` only (no redundant nested
  `status`). Nested siblings use the same shapes as AC-007.67 without cross-sibling recursion.
  Raw JSON MUST serialize `"gate"` → `"host_watch"` → embedded siblings in the same key order as
  AC-007.72 / AC-007.77. Tray/desktop wire decoders MUST tolerate the expanded envelopes (legacy
  field subsets remain valid for mapping helpers).
- **AC-007.79:** `sharecli proc --csv` and `sharecli proc --tree --csv` (one-shot, no `--watch`)
  MUST append companion `pool` and `status` CSV records after the `gate` and `host` companions
  from AC-007.19 / AC-007.14 via [`PoolOperatorPanel::format_csv_companion`](crates/sharecli-fleet/src/operator_pool_status.rs)
  / [`StatusOperatorPanel::format_csv_companion`](crates/sharecli-fleet/src/operator_pool_status.rs)
  (parity with text AC-007.75 and JSON AC-007.77). Raw stdout MUST serialize companion blocks in
  order: `gate` → `host_watch` → `pool` → `status`. **stderr** MUST remain silent on success
  (extends AC-007.33).

- **AC-007.80:** [`ClientMessage::HealthUpdate`](crates/sharecli-ipc/src/ws_client.rs) /
  [`ClientMessage::from_json`](crates/sharecli-ipc/src/ws_client.rs) MUST decode typed WebSocket
  `health_update` frames whose `health` payload embeds top-level `pool` + `status` siblings after
  `gate` → `host_watch` (parity with IPC `health.status` AC-007.78 and CLI `--json` AC-007.77).
  [`HealthSnapshot`](crates/sharecli-ipc/src/handler.rs) wire shape MUST match IPC; expanded
  envelopes MUST NOT fall through to [`ClientMessage::Unknown`](crates/sharecli-ipc/src/ws_client.rs).
  Legacy frames missing required `pool` / `status` fields MAY yield Unknown (no silent partial
  decode). Dashboard untyped WS envelope parity remains AC-007.70 / AC-007.41.

- **AC-007.81:** `sharecli report --format csv` (one-shot, no `--watch`) MUST emit a fleet
  analytics CSV body (summary + per-project + top-consumer sections via
  [`render_report_csv_body`](src/commands/report.rs)) followed by companion `gate`, `host_watch`,
  `pool`, and `status` CSV records (parity with proc CSV AC-007.79 and report text AC-007.74).
  Companion blocks MUST use the same shapes/order as AC-007.79:
  `gate` → `host_watch` → `pool` → `status`. **stderr** MUST remain silent on success.
  `--format csv` MUST NOT combine with `--watch` (one-shot export only, like proc `--csv`).

- **AC-007.82:** `sharecli health --csv`, `sharecli pool --csv`, and `sharecli status --csv`
  (one-shot, no `--watch`) MUST emit command-specific CSV bodies via
  [`render_health_csv_body`](src/commands/mod.rs) / [`render_pool_csv_body`](src/commands/mod.rs) /
  [`render_status_csv_body`](src/commands/mod.rs) followed by companion `gate`, `host_watch`,
  `pool`, and `status` CSV records via [`append_operator_csv_companions`](src/commands/mod.rs)
  (parity with proc CSV AC-007.79 and report CSV AC-007.81). Companion blocks MUST use the same
  shapes/order as AC-007.79: `gate` → `host_watch` → `pool` → `status`. **stderr** MUST remain
  silent on success. `--csv` MUST NOT combine with `--json` or `--watch` (one-shot export only).

- **AC-007.83:** `sharecli ps --all --csv` (one-shot) emits the managed-process +
  agent-inventory CSV body followed by companion `gate` → `host_watch` → `pool` →
  `status` CSV records (parity with AC-007.82); `ps --all --csv` rejects `--json`.
- **AC-007.84:** Operator envelope matrix: CLI one-shot text emits body → `gate` →
  `host_watch` → `pool` → proc-scan and one-shot JSON emits `gate` → `host_watch` →
  `pool`/`status` siblings across proc/report/health/pool/status/`ps --all`
  (stderr silent on success).
- **AC-007.85:** `sharecli proc --tree` one-shot text / `--json` / `--csv` passes the
  same operator envelope matrix contract as AC-007.84 for the tree renderer.
- **AC-007.86:** `sharecli proc --pid` one-shot text / `--json` / `--csv` passes the
  same operator envelope matrix contract, including pool and proc-scan operator
  panel parity with AC-007.77.
- **AC-007.87:** `sharecli proc --pid N --watch [secs]` keeps text/NDJSON watch parity
  with flat `sharecli proc --watch`: stderr silent across refresh cycles, `[watch]`
  footer on stdout (text), pipe-clean NDJSON stdout, and gate → `host_watch` →
  `pool` → `status` companions in every frame.
- **AC-007.88:** `sharecli proc --csv --watch` (flat and `--tree`) emits per-frame
  inventory plus `gate` → `host_watch` → `pool` → `status` companion records with
  stderr silent across the multi-frame envelope.
- **AC-007.89:** `sharecli health|pool|status --csv --watch` emit the command-specific
  CSV body plus `gate` → `host_watch` → `pool` → `status` companions per frame with
  stderr silent across the multi-frame envelope.
- **AC-007.90:** `sharecli report --format csv --watch` emits the fleet CSV body plus
  `gate` → `host_watch` → `pool` → `status` companions per frame with stderr silent;
  `--format csv --watch 0` stays rejected.
- **AC-007.91:** `sharecli proc --pid --csv --watch` emits the PID detail body plus
  `gate` → `host_watch` → `pool` → `status` companions per frame with stderr silent;
  `--pid --csv --json --watch` stays rejected.
- **AC-007.92:** `sharecli proc --pid` loudly rejects inventory-only flags (`--tree`,
  `--family`, `--sort --limit`) while `--pid --csv` remains allowed (not inventory).
- **AC-007.93:** CSV `--watch` frame-marker smoke (short dwell, ≥1 frame): stderr
  silent, no ANSI clear on stdout, frame marker emitted, `# [watch]` footer comment
  included.
- **AC-007.94:** The CSV `# [watch]` footer flushes in the same tick as the CSV body
  so pipe consumers see it immediately.
- **AC-007.95:** The CSV `# [watch]` footer flushes in the same tick as its frame
  (exactly one frame marker before the first footer).
- **AC-007.96:** The text `[watch]` footer flushes in the same tick as the watch body.
- **AC-007.97:** Text `--watch` `[watch]` footer flushes in the same tick across the
  full matrix (exactly one gate before the first footer).

**Test refs:** `tests/fr007_resource_thermal_watch.rs`, `tests/fr007_thermal_tui_watch.rs`, `tests/fr007_thermal_tui_gate_parity.rs`, `tests/fr007_thermal_tui_pool_status.rs`, `tests/fr004_status_health.rs`, `tests/fr007_proc_json_host_watch.rs`, `tests/fr007_proc_text_csv_host_watch.rs`, `tests/fr007_proc_tree_json_host_watch.rs`, `tests/fr007_proc_pid_json_host_watch.rs`, `tests/fr007_proc_pid_gate.rs`, `tests/fr007_proc_tree_json_gate.rs`, `tests/fr007_proc_text_csv_gate.rs`, `tests/fr007_proc_tree_text_gate.rs`, `tests/fr007_proc_text_gate.rs`, `tests/fr007_proc_watch_gate_order.rs`, `tests/fr007_proc_tree_watch_gate_order.rs`, `tests/fr007_proc_json_gate_order.rs`, `tests/fr007_status_json_host_watch.rs`, `tests/fr007_status_text_gate_order.rs`, `tests/fr007_proc_watch_stderr_footer.rs`, `tests/fr007_proc_tree_watch_stderr_footer.rs`, `tests/fr007_proc_json_stderr_silent.rs`, `tests/fr007_status_json_stderr_silent.rs`, `tests/fr007_proc_csv_stderr_silent.rs`, `tests/fr007_proc_csv_pool_status.rs`, `tests/fr007_report_csv_pool_status.rs`, `tests/fr007_report_csv_stderr_silent.rs`, `tests/fr007_health_pool_status_csv.rs`, `tests/fr007_proc_text_stderr_silent.rs`, `tests/fr007_proc_text_pool_status.rs`, `tests/fr007_proc_watch_text_stderr_silent.rs`, `tests/fr007_status_text_stderr_silent.rs`, `tests/fr007_health_pool_text_stderr_silent.rs`, `tests/fr007_health_watch_text_stderr_silent.rs`, `tests/fr007_health_watch_json_gate_host_watch.rs`, `tests/fr007_pool_watch_text_stderr_silent.rs`, `tests/fr007_pool_watch_json_gate_host_watch.rs`, `tests/fr007_status_watch_text_stderr_silent.rs`, `tests/fr007_status_watch_json_gate_host_watch.rs`, `tests/fr007_ps_all_text_stderr_silent.rs`, `tests/fr007_ps_all_json_gate_host_watch.rs`, `tests/fr007_ps_all_watch_json_gate_host_watch.rs`, `tests/fr007_ps_all_watch_text_stderr_silent.rs`, `tests/fr007_health_pool_status_ps_text_pool_status.rs`, `tests/fr007_operator_json_pool_status.rs`, `tests/fr007_ipc_health_pool_status.rs`, `tests/fr007_ws_client_health_update_pool_status.rs`, `tests/fr007_report_text_stderr_silent.rs`, `tests/fr007_report_text_pool_status.rs`, `tests/fr007_report_json_gate_host_watch.rs`, `tests/fr007_report_json_pool_status.rs`, `tests/fr007_report_watch_json_gate_host_watch.rs`, `tests/fr007_dashboard_ws_operator_envelope.rs`, `tests/fr007_health_pool_json_gate_host_watch.rs`, `tests/fr007_ipc_health_status_gate_host_watch.rs`, `tests/fr007_ipc_monitoring_report_gate_host_watch.rs`, `tests/fr007_ipc_monitoring_report_pool_status.rs`, `tests/fr007_ipc_pool_status_snapshot.rs`, `tests/fr007_ipc_pool_status_tray_wire.rs`, `tests/fr007_tray_pool_status_consume.rs`, `tests/fr007_tray_monitoring_report_consume.rs`, `tests/fr007_tray_windows_monitoring_report_consume.rs`, `tests/fr007_tray_windows_poll_interval.rs`, `tests/fr007_tray_linux_poll_interval.rs`, `tests/fr007_tray_swift_poll_interval.rs`, `tests/fr007_tray_windows_kill.rs`, `tests/fr007_tray_linux_kill.rs`, `tests/fr007_tray_swift_kill.rs`, `tests/fr007_tray_windows_harness.rs`, `tests/fr007_tray_gate_host_watch_ui.rs`, `tests/fr007_tray_thermal_visual.rs`, `tests/fr007_operator_envelope_parity_suite.rs`, `tests/fr007_ps_all_csv.rs`, `tests/fr007_proc_pid_watch.rs`, `tests/fr007_proc_csv_watch.rs`, `tests/fr007_operator_csv_watch.rs`, `tests/fr007_report_csv_watch.rs`, `tests/fr007_proc_pid_csv_watch.rs`, `tests/fr007_proc_pid_combo_rejects.rs`, `src/commands/serve.rs` (`DashboardWsSnapshot`, `build_dashboard_ws_snapshot`), `src/commands/mod.rs` (`PsAllJson`, `PsAllNdjsonLine`, `HealthJson`, `HealthNdjsonLine`, `PoolJson`, `PoolNdjsonLine`, `StatusJson`, `StatusNdjsonLine`, `build_pool_json`, `build_status_json`, `fetch_operator_pool_status_siblings`, `print_live_pool_status_operator_sections`, `render_health_csv_body`, `render_pool_csv_body`, `render_status_csv_body`, `append_operator_csv_companions`, `ps --all --json`, `ps --all --watch --json`, `health --json`, `health --csv`, `health --watch`, `health --watch --json`, `pool --json`, `pool --csv`, `pool --watch`, `pool --watch --json`, `status --json`, `status --csv`, `status --watch`, `status --watch --json`), `src/commands/report.rs` (`FleetReportJson`, `FleetReportNdjsonLine`, `ReportFormat::Csv`, `render_report_csv_body`, `render_once`), `src/commands/proc/mod.rs` (`AgentProcSnapshot`, `AgentTreeSnapshot`, `append_proc_csv_companions`, `render_once` JSON path), `crates/sharecli-fleet/src/operator_pool_status.rs`, `crates/sharecli-ipc/src/handler.rs` (`HealthSnapshot`, `PoolSnapshot`, `StatusSnapshot`, `MonitoringReportSnapshot`, `health.status`, `pool.status`, `status.snapshot`, `monitoring.report`, `process.kill`, `process.kill_all`, `capture_pool_snapshot`, `capture_status_snapshot`), `crates/sharecli-ipc/src/ws_client.rs` (`ClientMessage`, `ClientMessage::from_json`, `SharecliClient`, `SharecliStream`), `crates/sharecli-tray-linux/src/ipc.rs`, `crates/sharecli-tray-linux/src/operator_display.rs`, `crates/sharecli-tray-linux/src/poll.rs`, `crates/sharecli-tray-linux/src/main.rs`, `desktop/ShareCLITray/Sources/ShareCLICore/OperatorDisplay.swift`, `desktop/ShareCLITray/Sources/ShareCLICore/AppState.swift`, `desktop/ShareCLITray/Sources/ShareCLICore/TrayPoll.swift`, `desktop/ShareCLITray/Sources/ShareCLICore/IPCClient.swift`, `desktop/ShareCLITray/Sources/ShareCLITray/TrayPopoverView.swift`, `desktop/ShareCLITray/Sources/ShareCLITray/DashboardView.swift`, `desktop/ShareCLITray/Sources/ShareCLITray/AppEntry.swift`, `crates/sharecli-tray-windows/src/ipc.rs`, `crates/sharecli-tray-windows/src/operator_display.rs`, `crates/sharecli-tray-windows/src/poll.rs`, `windows/ShareCLITray/MonitoringReportSnapshot.cs`, `windows/ShareCLITray/PoolStatusSnapshot.cs`, `windows/ShareCLITray/OperatorDisplay.cs`, `windows/ShareCLITray/IpcKill.cs`, `windows/ShareCLITray/TrayPoll.cs`, `windows/ShareCLITray/TrayWindow.xaml`, `windows/ShareCLITray/TrayWindow.xaml.cs`, `src/dashboard.html`

---

## FR-008 — Speculative Coalesce / Debounce / Queue

**Statement:** Identical concurrent invocations MUST coalesce via
Lock-Wait-Cache (`CoalesceCache`) with configurable TTL (default 300s) and
optional debounce window. Mutating argv matching `nocache_args` (Feb harness)
MUST route through an N-slot [`SlotQueue`] / [`PriorityQueue`] and MUST NOT
be served from the coalesce cache.

**Source:**

- `crates/sharecli-ipc` — `command_key`, `CoalesceCache::{new,with_ttl,with_options}`,
  `SlotQueue` / `PriorityQueue`, `has_nocache_arg` / `DEFAULT_NOCACHE_ARGS`
- `crates/sharecli-core` — `Hypervisor::{run,queue,nocache_args}`

**Acceptance Criteria:**

- **AC-008.1:** Identical argv / cwd / env → same `command_key`; different argv → different.
- **AC-008.2:** `CoalesceCache::with_lock` runs the miss path once per key.
- **AC-008.3:** Thermal `Refuse` fails loudly before speculative coalesce / spawn.
- **AC-008.4:** Second identical `Allow` run is served from coalesce cache.
- **AC-008.5:** Entries older than configured TTL are treated as miss; `store` evicts stale `*.json`.
- **AC-008.6:** When debounce is set, a miss waits then shares an in-window sibling store instead of re-running.
- **AC-008.7:** Argv containing a configured `nocache_args` flag (`--fix`, `--force`, `--write`, …) MUST bypass coalesce detection (`should_bypass_coalesce`).
- **AC-008.8:** `SlotQueue` with `max_concurrent=1` MUST serialize concurrent lane work.
- **AC-008.9:** `Hypervisor::run` with nocache argv MUST execute via `SlotQueue` and MUST NOT set `from_cache` on replay.
- **AC-008.10:** End-to-end Hypervisor nocache MUST (a) re-execute identical mutating argv (side-effect counter increments per run), (b) serialize concurrent nocache runs through the Hypervisor queue (`max_concurrent=1`), and (c) remain isolated from a seeded coalesce cache hit on a read-only twin argv.
- **AC-008.11:** `sharecli status` and `sharecli thermal` surface Hypervisor coalesce hit/miss/nocache counters via [`global_coalesce_meters`](crates/sharecli-ipc/src/lib.rs) / [`CoalesceMeters::format_status_section`](crates/sharecli-ipc/src/lib.rs); counters MUST increment on lookup hits, [`CoalesceCache::with_lock_detailed`](crates/sharecli-ipc/src/lib.rs) outcomes, and nocache queue runs.
- **AC-008.12:** `sharecli status` and `sharecli thermal` surface Hypervisor SlotQueue acquire/wait/timeout counters via [`global_slot_queue_meters`](crates/sharecli-fleet/src/slot_queue_meters.rs) / [`SlotQueueMeters::format_status_section`](crates/sharecli-fleet/src/slot_queue_meters.rs); counters MUST increment on successful [`SlotQueue::with_slot`](crates/sharecli-ipc/src/queue.rs) acquisitions, wait-loop iterations, and queue timeouts.
- **AC-008.13:** [`command_key`](crates/sharecli-ipc/src/lib.rs) MUST incorporate `cwd` and `env_subset`: identical argv with different `cwd` or env values produce different keys; permuted env key order produces the same key; [`Hypervisor::run`](crates/sharecli-core/src/lib.rs) coalesce cache hits require matching argv, `cwd`, and `env` (not argv alone).
- **AC-008.14:** On the Hypervisor nocache [`SlotQueue`](crates/sharecli-ipc/src/queue.rs) lane, [`QueuePriority::Critical`](crates/sharecli-ipc/src/queue.rs) MUST acquire a slot before [`QueuePriority::Normal`](crates/sharecli-ipc/src/queue.rs) when both are waiting under contention; [`Hypervisor::run`](crates/sharecli-core/src/lib.rs) MUST pass [`SpawnRequest::queue_priority`](crates/sharecli-core/src/lib.rs) through to [`SlotQueue::with_slot`](crates/sharecli-ipc/src/queue.rs).
- **AC-008.15:** Operator surface MUST propagate into [`SpawnRequest::queue_priority`](crates/sharecli-core/src/lib.rs): non-empty [`SHARECLI_QUEUE_PRIORITY`](crates/sharecli-ipc/src/queue.rs) env overrides optional rules.conf `priority=`; [`SpawnRequest::from_operator`](crates/sharecli-core/src/lib.rs) and [`SpawnRequest::new`](crates/sharecli-core/src/lib.rs) MUST resolve via [`resolve_operator_queue_priority`](crates/sharecli-ipc/src/queue.rs); harness-native callers MUST use the same resolver for rules.conf priority.
- **AC-008.16:** harness-native [`queue`](crates/harness-native/src/strategies/queue.rs) and [`priority_queue`](crates/harness-native/src/strategies/mod.rs) strategies MUST execute via [`Hypervisor::run_queued`](crates/sharecli-core/src/lib.rs) with [`SpawnRequest::from_operator`](crates/sharecli-core/src/lib.rs) (`rules.conf` `priority=` + env); MUST NOT use raw `Command::spawn`; repeated identical invocations MUST NOT set `from_cache`.
- **AC-008.17:** harness-native [`coalesce`](crates/harness-native/src/strategies/coalesce.rs) and [`cache`](crates/harness-native/src/strategies/mod.rs) strategies MUST execute via [`Hypervisor::run`](crates/sharecli-core/src/lib.rs) with [`SpawnRequest::from_operator`](crates/sharecli-core/src/lib.rs); cache root MUST be `{harness_home}/var/sharecli-hypervisor`; [`RuleOpts`](crates/harness-native/src/strategies/mod.rs) `ttl=` / `debounce_ms=` / `max_concurrent=` MUST map into [`HypervisorConfig`](crates/sharecli-core/src/lib.rs); repeated identical invocations MUST set `from_cache` on replay; MUST NOT use raw `Command::spawn`.
- **AC-008.18:** harness-native [`debounce`](crates/harness-native/src/strategies/debounce.rs) strategy MUST execute via [`Hypervisor::run`](crates/sharecli-core/src/lib.rs) with [`SpawnRequest::from_operator`](crates/sharecli-core/src/lib.rs) and [`RuleOpts`](crates/harness-native/src/strategies/mod.rs) `debounce_ms=` mapped into [`HypervisorConfig::coalesce_debounce`](crates/sharecli-core/src/lib.rs) via [`hypervisor_lane`](crates/harness-native/src/strategies/hypervisor_lane.rs); MUST share in-window sibling stores per AC-008.6; repeated identical invocations MUST set `from_cache` on replay; MUST NOT use raw `Command::spawn`.
- **AC-008.19:** [`cache_key_mode`](crates/sharecli-ipc/src/cache_key.rs) `args` shares
  one coalesce cache across cwd/env (both dimensions ignored), while `time` mode keeps
  the legacy cwd/env-sensitive [`command_key`](crates/sharecli-ipc/src/lib.rs).
- **AC-008.20:** Semantic normalization runs before the Hypervisor cache hash
  ([`semantic`](crates/sharecli-ipc/src/semantic.rs)): `.` maps to the project-root
  token and directory paths canonicalize before keying.

**Test refs:** `tests/fr008_coalesce_mesh.rs`; `tests/fr008_queue_priority_operator.rs`; `tests/fr008_coalesce_status.rs`; `tests/fr004_status_health.rs`; `tests/fr007_thermal_tui_watch.rs`; `tests/e2e_hypervisor_nocache.rs`; `crates/harness-native/tests/native_harness_contract.rs`; `sharecli-core` `hypervisor_run_queued_skips_coalesce_cache`; `crates/harness-native/src/strategies/coalesce.rs` (`coalesce_strategy_executes_via_hypervisor`, `coalesce_strategy_serves_cache_on_replay`); `crates/harness-native/src/strategies/debounce.rs` (`debounce_strategy_executes_via_hypervisor`, `debounce_strategy_serves_cache_on_replay`, `debounce_strategy_shares_in_window_store`); `crates/harness-native/src/strategies/hypervisor_lane.rs` (`rule_opts_plumb_hypervisor_config`); `sharecli-ipc` unit tests for TTL/debounce/queue/nocache/meters.

---

## FR-009 — FUSE IO Intercept

**Statement:** On Linux and macOS, sharecli MUST provide a FUSE attach point
(`InterceptFs` / `mount`) over a backing path as the hypervisor IO intercept
extension point. Other platforms MUST return a clear unsupported error.
Core VFS ops (lookup, getattr, open, read, write, readdir, mkdir, unlink,
rmdir, rename) MUST forward to the backing filesystem via an inode map.
In-process read content cache MUST key by path+mtime with hit/miss meters.
Writes to the same path MUST serialize. Staging CoW (`stage_bytes` →
`commit_pending` / `discard_pending`) MUST promote or drop staging copies
without silent no-ops; commit/discard with no pending entry MUST return
`NoPending`. Successful `write_rel` and `commit_rel` MUST stamp write
provenance extended attributes (`user.sharecli.session`,
`user.sharecli.written_at`) on the backing file. Missing-path lookups MUST be
remembered in a TTL negative dentry cache and skipped on subsequent probes
until create / mkdir / rename-into invalidates the entry.

**Source:**

- `crates/sharecli-fuse/src/lib.rs` — `InterceptFs`, `mount`
- `crates/sharecli-fuse/src/inode_map.rs` — `InodeMap`
- `crates/sharecli-fuse/src/read_cache.rs` — `ReadContentCache`
- `crates/sharecli-fuse/src/neg_dentry.rs` — `NegativeDentryCache`
- `crates/sharecli-fuse/src/write_serialize.rs` — `WriteSerialize`
- `crates/sharecli-fuse/src/provenance.rs` — write provenance xattrs
- `crates/sharecli-fuse/src/mount_smoke.rs` — opt-in privileged mount smoke
- `src/commands/fuse.rs` — `sharecli fuse provenance`

**Acceptance Criteria:**

- **AC-009.1:** `InterceptFs::new` constructs over an existing backing path
  without requiring a privileged mount (unit / pure construct).
- **AC-009.2:** `mount` is publicly exported; on unsupported platforms it
  returns an error mentioning unsupported (no silent success).
- **AC-009.3:** `InodeMap` resolves nested parent/child paths and allocates
  stable inodes without a privileged mount.
- **AC-009.4:** In-process read coalesce cache records a miss then a hit for
  identical path+mtime; meters expose hits/misses (no privileged mount).
- **AC-009.5:** Per-path write serialization is available; passthrough write
  succeeds (no ENOSYS); `stage_bytes` + `commit_pending` promotes staging to
  backing; `stage_bytes` + `discard_pending` leaves backing unchanged;
  commit/discard with no pending returns `NoPending`. InterceptFs exposes
  `stage_rel` / `commit_rel` / `discard_rel` for FR tests without mount.
- **AC-009.6:** `write_rel` and `commit_rel` stamp `user.sharecli.session`
  and `user.sharecli.written_at` on the backing path; session id is readable
  via `read_provenance` / `InterceptFs::session_id` (no privileged mount).
- **AC-009.7:** `exists_rel` / FUSE `lookup` remember ENOENT in
  `NegativeDentryCache` (miss then hit within TTL); `invalidate_neg_rel` /
  mkdir / rename-into clear the entry so a newly created path is visible
  without waiting for TTL expiry (no privileged mount).
- **AC-009.8:** With `SHARECLI_FUSE_MOUNT_SMOKE=1`, `run_mount_smoke` performs
  a read/write round-trip through a live FUSE mount over a temp backing tree and
  verifies write provenance xattrs on the backing file after the FUSE write;
  default `cargo test` skips without failure when the env var is unset.
- **AC-009.9:** `sharecli status` and `sharecli thermal` surface FUSE
  negative-dentry hit/miss meters via [`global_neg_dentry_meters`](crates/sharecli-fuse/src/neg_dentry.rs)
  / [`NegDentryMeters::format_status_section`](crates/sharecli-fuse/src/neg_dentry.rs).
- **AC-009.10:** `sharecli status` and `sharecli thermal` surface FUSE
  write-serialize / CoW counters (passthrough writes, stages, commits, discards) via
  [`global_write_serialize_meters`](crates/sharecli-fuse/src/write_serialize_meters.rs)
  / [`WriteSerializeMeters::format_status_section`](crates/sharecli-fuse/src/write_serialize_meters.rs);
  counters MUST increment on `write_rel`, `stage_rel` / `stage_bytes`, `commit_rel` /
  `commit_pending`, and `discard_rel` / `discard_pending`.
- **AC-009.11:** `sharecli fuse provenance <path>` reads write-provenance xattrs via
  [`read_provenance`](crates/sharecli-fuse/src/provenance.rs) on a backing file (no live
  mount required); `--json` emits `{path,session_id,written_at_unix}` or `null` when absent;
  missing paths and directories fail loudly.
- **AC-009.12:** On Hypervisor cache-miss spawns, FUSE intercept mounts MUST use
  [`fuse_session_id_for_command_key`](crates/sharecli-core/src/lib.rs) derived from the
  coalesce [`CommandKey`](crates/sharecli-ipc/src/lib.rs) (`hv-{first 16 hex}`) via
  [`sharecli_fuse::mount_with_session`](crates/sharecli-fuse/src/lib.rs); writes through
  the intercept layer MUST stamp that session id (parity with AC-009.6).
- **AC-009.13:** [`SpawnOutcome::fuse_session_id`](crates/sharecli-core/src/lib.rs) MUST be
  `Some(session)` when a cache-miss spawn had an active FUSE intercept mount
  ([`SpawnOutcome::fuse_intercept_active`](crates/sharecli-core/src/lib.rs)); MUST be `None`
  for coalesce cache hits, nocache queue routing, or cache misses where FUSE did not mount.
  The session string MUST match [`fuse_session_id_for_command_key`] for the coalesce key.
- **AC-009.14:** When FUSE intercept is active, [`SpawnOutcome`](crates/sharecli-core/src/lib.rs)
  MUST expose `fuse_backing` (original cwd) and `fuse_mountpoint` (ephemeral intercept mount);
  [`SpawnOutcome::remap_fuse_path`](crates/sharecli-core/src/lib.rs) and
  [`sharecli_fuse::remap_mount_to_backing`](crates/sharecli-fuse/src/path_remap.rs) MUST
  translate paths under the mount to backing equivalents (prefix-safe, `None` outside subtree).
  [`FuseGuard`](crates/sharecli-core/src/lib.rs) MUST remain mounted for the full coalesce
  spawn window and force-unmount on drop after the child exits.
- **AC-009.15:** FUSE `create_rel` stamps write provenance on the created path and
  invalidates its negative dentry and read-cache entries.
- **AC-009.16:** With `SHARECLI_FUSE_MOUNT_SMOKE=1`, the privileged mount smoke covers
  create / mkdir / unlink / rename round-trips through a live FUSE mount.
- **AC-009.17:** `sharecli fuse` CLI operator surface (mount / unmount / status / list
  / commit / discard): `fuse status --json` exposes read-cache and write-serialize
  meters; `fuse list` with no mounts succeeds.
- **AC-009.18:** [`agents.conf`](crates/sharecli-fuse/src/agents_conf.rs) parsing
  ignores comments and blank lines, matches agent ids by substring, and sanitizes
  agent ids.
- **AC-009.19:** Agent-scoped CoW
  ([`agent_cow`](crates/sharecli-fuse/src/agent_cow.rs)): two agents stage the same
  path independently, committing one leaves the other staged, and `commit_all` /
  `discard_all` operate per agent.
- **AC-009.20:** `--no-serialize` skips the lock-acquisition path and still runs the
  write callback (agent CoW).
- **AC-009.21:** `sharecli fuse mount` loudly rejects invalid `--agent` values (path
  separators) and a missing `--agents-conf` before mounting; help documents the Feb
  flags.
- **AC-009.22:** [`fuse-smoke`](crates/fuse-smoke-runner) CLI runs the OS × arch
  privileged FUSE mount-smoke matrix with loud fail reasons.
- **AC-009.25:** Windows WinFsp mount adapter
  ([`winfsp_mount`](crates/sharecli-fuse/src/winfsp_mount.rs)) for the FUSE intercept.

**Test refs:** `tests/fr009_fuse_intercept.rs`; `tests/fr009_fuse_cli.rs`; `tests/fr009_fuse_hypervisor_session.rs`; `tests/fr004_status_health.rs`; `tests/fr007_thermal_tui_watch.rs`; `sharecli-fuse` unit tests.

---

## FR-010 — Agent Mesh / Shared Substrate

**Statement:** sharecli MUST expose mesh / substrate coordination primitives
for participating hosts (registry subject namespace + device records), a
Maildir-style filesystem task queue (`enqueue` / `claim` / `ack`) as the
execution-substrate port of `thegent.mesh.task_queue`, plus Jun mesh ports
for smart three-way merge (`SmartMerger`) and git worktree pooling
(`WorktreePool`). Operators MUST be able to inspect queue depth
([`MaildirQueue::status`] / `sharecli mesh status`) and reclaim stranded
in-flight work for a given owner ([`MaildirQueue::reclaim_owner`] /
`sharecli mesh reclaim`).

**Source:**

- `crates/sharecli-fleet/src/registry.rs` — `FleetRegistry`, `DeviceRecord`
- `crates/sharecli-mesh` — `MaildirQueue::{enqueue,claim,ack,nack,status,reclaim_owner}`
- `crates/sharecli-mesh` — `SmartMerger`, `WorktreePool`
- `src/commands/mesh.rs` — `sharecli mesh status|reclaim`

**Acceptance Criteria:**

- **AC-010.1:** Disconnected registry uses default subject prefix
  `sharecli.fleet`.
- **AC-010.2:** `subject_for(device_id)` yields `{prefix}.devices.{device_id}`.
- **AC-010.3:** `DeviceRecord` JSON round-trips with required keys; register
  without NATS fails loudly (no silent publish).
- **AC-010.4:** `MaildirQueue::enqueue` writes via `tmp/`→`new/`; `claim` moves
  `new/`→`cur/`; `ack` removes from `cur/`.
- **AC-010.5:** Lower `priority` values are claimed before higher ones.
- **AC-010.6:** `nack` returns a claimed task from `cur/` to `new/` for retry.
- **AC-010.7:** `SmartMerger::merge` falls back to `git merge-file --diff3`
  when mergiraf is unavailable; clean non-overlapping edits succeed;
  conflicting edits set `success=false` and still write output.
- **AC-010.8:** `WorktreePool::allocate` / `release` create and remove git
  worktrees under a pool root; opening a non-git repo fails with
  `NotGitRepo` (loud, no directory-slot silent fallback).
- **AC-010.9:** `MaildirQueue::status` reports `ready` (`new/`), `in_flight`
  (`cur/`), and `pending` (= sum); `sharecli mesh status --queue` / `--json`
  exposes the same counts.
- **AC-010.10:** `MaildirQueue::reclaim_owner` moves matching `cur/` tasks
  back to `new/` (count returned); non-matching owners reclaim 0;
  `sharecli mesh reclaim --queue --owner` performs the same; empty owner
  fails loudly.
- **AC-010.11:** `sharecli status` and `sharecli thermal` surface Maildir
  queue depth (`ready` / `in_flight` / `pending`) via
  [`capture_maildir_status`](crates/sharecli-mesh/src/operator_status.rs)
  / [`MaildirStatus::format_status_section`](crates/sharecli-mesh/src/operator_status.rs)
  when `SHARECLI_MESH_QUEUE` or the default `{state_dir}/mesh/queue` exists.

**Test refs:** `tests/fr010_mesh_substrate.rs`; `tests/fr010_mesh_cli.rs`;
`tests/fr004_status_health.rs`; `tests/fr007_thermal_tui_watch.rs`;
`sharecli-mesh` unit tests.

---

## FR-011 — Thermal Contention Gate

**Statement:** Host pressure MUST map to Green / Yellow / Red and gate
speculative coalesce via Allow / Warn / Refuse semantics.

**Source:**

- `crates/sharecli-fleet/src/thermal.rs` — `ThermalGovernor`, `ThermalLevel`
- `crates/sharecli-core` — `FakeThermalGate`, `ThermalDecision`

**Acceptance Criteria:**

- **AC-011.1:** `ThermalGovernor::with_mock` returns the configured level on poll.
- **AC-011.2:** `FakeThermalGate` Refuse / Allow decisions are stable.
- **AC-011.3:** Hypervisor Refuse path errors with thermally throttled
  (see also AC-008.3).
- **AC-011.4:** Production [`Hypervisor`](crates/sharecli-core/src/lib.rs) wraps
  [`SystemThermalGate`](crates/sharecli-core/src/lib.rs) with
  [`AgentAwareThermalGate`](crates/sharecli-core/src/lib.rs): live proc-scan agent
  count escalates Allow→Warn at `warn_at` (default 4) and any decision→Refuse at
  `refuse_at` (default 8); `sharecli thermal` gate panel uses
  [`effective_gate_decision`](crates/sharecli-fleet/src/agent_contention.rs) so
  agent Refuse shows DENY even when thermal is Green.
- **AC-011.5:** `sharecli status` polls live thermal level and proc-scan agent
  inventory and prints [`format_gate_status_section`](crates/sharecli-fleet/src/agent_contention.rs)
  with thermal level, detected agent count, contention tier, and ADMIT/DENY gate
  decision (parity with thermal TUI gate panel).
- **AC-011.6:** `sharecli report` (text + JSON) and `sharecli ps --all` expose the
  same gate fields via [`gate_status_snapshot`](crates/sharecli-fleet/src/agent_contention.rs)
  / [`format_gate_status_section`](crates/sharecli-fleet/src/agent_contention.rs)
  (detected agent count, contention tier, ADMIT/DENY).
- **AC-011.7:** `sharecli pool` and `sharecli health` print the same live
  [`format_gate_status_section`](crates/sharecli-fleet/src/agent_contention.rs)
  after pool/runtime health output (parity with `status` / AC-011.5).

**Test refs:** `tests/fr011_thermal_gate.rs`, `tests/fr011_agent_thermal_gate.rs`, `tests/fr004_status_health.rs`, `tests/fr011_report_gate.rs`, `tests/fr011_pool_health_gate.rs`, `tests/fr008_coalesce_mesh.rs`

---

## FR-012 — Serve HTTP Federated AuthN

**Statement:** When `sharecli serve` is configured with `auth_mode = "jwt"` (or an
equivalent `[serve.jwt]` block), non-probe HTTP routes MUST reject requests whose
`Authorization: Bearer` JWT fails signature or `iss` / `aud` / `exp` validation
against the configured JWKS. `/healthz` and `/readyz` MUST remain public.

**Source:**

- `src/serve_auth.rs` — JWT / JWKS validation middleware
- `src/config.rs` — `ServeConfig` / `ServeJwtConfig`
- `src/commands/serve.rs` — AuthN wiring at serve startup
- `docs/ops/AUTH.md` — operator guide

**Acceptance Criteria:**

- **AC-012.1:** Valid RS256 JWT with matching `iss`/`aud` is authorized (returns
  `sub` from claims).
- **AC-012.2:** Expired JWT is rejected with reason `jwt_expired`.
- **AC-012.3:** Wrong `aud` / `iss` are rejected; probe paths stay public
  (see unit tests for `/healthz` `/readyz`).

**Test refs:** `tests/fr012_serve_jwt_auth.rs`, `src/serve_auth.rs` unit tests

---

## NFR Notes

- **NFR-001 Platform Support:** The CLI MUST build and run on Linux, macOS,
  and Windows. Process-pool tests are gated with `#[cfg(unix)]` /
  `#[cfg(windows)]` blocks to honour this.
- **NFR-002 Observability:** The CLI MUST emit structured logs via
  `tracing`/`tracing-subscriber`, gated on `--verbose` / `--quiet`.
- **NFR-003 Error Handling:** All commands MUST return `anyhow::Result<()>`
  and MUST NOT panic on missing config (FR-002 covers the missing-file case
  by returning `Config::default()`).
- **NFR-004 Eval boundary:** Harbor soak is not a sharecli product acceptance
  criterion (ADR 0002).
