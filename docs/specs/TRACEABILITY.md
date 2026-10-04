# Traceability Index — sharecli

> Authoritative mapping between Functional Requirements, source code, and
> acceptance tests. Updated by the spec+test+traceability workflow.

**How to use this file:**

1. Every FR in `docs/specs/FR.md` has a row here.
2. The `Source` column lists the canonical Rust module(s) that implement
   the requirement.
3. The `Tests` column lists every acceptance test file that covers the
   requirement. Each test carries an `// FR-XXX` comment in its source.
4. The `Status` column tracks whether the FR has at least one passing
   acceptance test.

**Phase:** 3+ runtime thesis (FR-006..011) · operator AuthN (FR-012)
**Last updated:** 2026-07-19 (thesis re-enum from origin export)

---

## FR ↔ Source ↔ Tests Matrix

| FR ID   | Title                                | Source                                                                                       | Tests                                                                                              | Status |
|---------|--------------------------------------|----------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------|--------|
| FR-001  | Managed Process Lifecycle            | `src/main.rs`, `src/commands/mod.rs`, `src/runtime.rs`                                      | `tests/fr001_process_lifecycle.rs`, `tests/fr001_stop_filter.rs`                                  | ACCEPTED |
| FR-002  | TOML Configuration Management        | `src/config.rs`, `src/commands/mod.rs`                                                       | `tests/fr002_config_load.rs`, `tests/fr002_config_init.rs`                                         | ACCEPTED |
| FR-003  | Project Registry                     | `src/config.rs`, `src/commands/mod.rs`                                                       | `tests/fr003_project_registry.rs`, `tests/fr003_project_discover.rs`                              | ACCEPTED |
| FR-004  | Process & Pool Health Status         | `src/runtime.rs`, `src/monitoring.rs`, `src/commands/mod.rs`                                 | `tests/fr004_status_health.rs`, `tests/fr004_pool_status.rs`                                      | ACCEPTED |
| FR-005  | Per-Project Resource Limits          | `src/runtime.rs`, `src/commands/mod.rs`                                                      | `tests/fr005_project_limits.rs`, `tests/fr005_resource_check.rs`                                  | ACCEPTED |
| FR-006  | Agent Detection (proc scan)          | `crates/sharecli-fleet/src/proc_scan.rs`, `resource_watch.rs`                                | `tests/fr006_agent_detection.rs`, `tests/fr006_proc_tree.rs`, `tests/fr006_agent_pid_watch.rs`   | ACCEPTED |
| FR-007  | Resource & Syscall-Relevant Watch    | `sharecli-core`, `sharecli-fleet`, `src/monitoring.rs`                                       | `tests/fr007_resource_thermal_watch.rs`                                                           | ACCEPTED |
| FR-008  | Speculative Coalesce / Debounce / Queue | `sharecli-ipc` (cache+queue+nocache), `sharecli-core` Hypervisor                          | `tests/fr008_coalesce_mesh.rs`                                                                    | ACCEPTED |
| FR-009  | FUSE IO Intercept                    | `crates/sharecli-fuse`                                                                       | `tests/fr009_fuse_intercept.rs`                                                                   | ACCEPTED |
| FR-010  | Agent Mesh / Shared Substrate        | `sharecli-fleet` registry, `crates/sharecli-mesh` MaildirQueue / SmartMerger / WorktreePool, `sharecli mesh` CLI | `tests/fr010_mesh_substrate.rs` · `tests/fr010_mesh_cli.rs` | ACCEPTED |
| FR-011  | Thermal Contention Gate              | `sharecli-fleet` thermal, `sharecli-core` FakeThermalGate                                    | `tests/fr011_thermal_gate.rs`, `tests/fr008_coalesce_mesh.rs`                                     | ACCEPTED |
| FR-012  | Serve HTTP Federated AuthN           | `src/serve_auth.rs`, `src/config.rs` (`ServeJwtConfig`), `src/commands/serve.rs`            | `tests/fr012_serve_jwt_auth.rs`                                                                   | ACCEPTED |

**Coverage summary:** 12 FRs mapped (FR-001..012), acceptance tests present, 0 gaps.

---

## Per-FR Detail

### FR-001 — Managed Process Lifecycle

| AC        | Test file                              | Test function                          |
|-----------|----------------------------------------|----------------------------------------|
| AC-001.1  | `tests/fr001_process_lifecycle.rs`     | `fr001_start_records_pid_in_pool`      |
| AC-001.2  | `tests/fr001_process_lifecycle.rs`     | `fr001_ps_table_columns_present`       |
| AC-001.3  | `tests/fr001_process_lifecycle.rs`     | `fr001_ps_filter_by_project`           |
| AC-001.4  | `tests/fr001_stop_filter.rs`           | `fr001_stop_all_terminates_everything` |
| AC-001.5  | `tests/fr001_stop_filter.rs`           | `fr001_stop_without_selector_errors`   |

### FR-002 — TOML Configuration Management

| AC        | Test file                          | Test function                            |
|-----------|------------------------------------|------------------------------------------|
| AC-002.1  | `tests/fr002_config_init.rs`       | `fr002_init_creates_default_toml`        |
| AC-002.2  | `tests/fr002_config_init.rs`       | `fr002_validate_reports_project_count`   |
| AC-002.3  | `tests/fr002_config_load.rs`       | `fr002_show_prints_projects_and_runtime` |
| AC-002.4  | `tests/fr002_config_load.rs`       | `fr002_load_roundtrips_projects_map`     |
| AC-002.5  | `tests/fr002_config_load.rs`       | `fr002_runtime_config_default_values`    |

### FR-003 — Project Registry

| AC        | Test file                            | Test function                              |
|-----------|--------------------------------------|--------------------------------------------|
| AC-003.1  | `tests/fr003_project_registry.rs`    | `fr003_project_add_inserts_and_persists`   |
| AC-003.2  | `tests/fr003_project_registry.rs`    | `fr003_project_list_prints_registered`     |
| AC-003.3  | `tests/fr003_project_registry.rs`    | `fr003_project_show_resolves_path`         |
| AC-003.4  | `tests/fr003_project_discover.rs`    | `fr003_project_discover_finds_git_repos`   |
| AC-003.5  | `tests/fr003_project_registry.rs`    | `fr003_project_remove_drops_entry`         |

### FR-004 — Process & Pool Health Status

| AC        | Test file                          | Test function                            |
|-----------|------------------------------------|------------------------------------------|
| AC-004.1  | `tests/fr004_status_health.rs`     | `fr004_status_prints_harness_table`      |
| AC-004.2  | `tests/fr004_pool_status.rs`       | `fr004_pool_reports_node_and_bun`        |
| AC-004.3  | `tests/fr004_pool_status.rs`       | `fr004_health_reports_healthy_or_degraded` |
| AC-004.4  | `tests/fr004_status_health.rs`     | `fr004_health_status_marks_unhealthy`    |
| AC-004.5  | `tests/fr004_status_health.rs`     | `fr004_process_stats_idle_threshold`     |

### FR-005 — Per-Project Resource Limits

| AC        | Test file                          | Test function                            |
|-----------|------------------------------------|------------------------------------------|
| AC-005.1  | `tests/fr005_project_limits.rs`    | `fr005_project_limits_default_values`    |
| AC-005.2  | `tests/fr005_project_limits.rs`    | `fr005_limits_set_persists_values`       |
| AC-005.3  | `tests/fr005_project_limits.rs`    | `fr005_get_limits_returns_default_for_unknown` |
| AC-005.4  | `tests/fr005_resource_check.rs`    | `fr005_resource_check_overall_ok_logic`  |
| AC-005.5  | `tests/fr005_resource_check.rs`    | `fr005_check_prints_status_lines`        |

### FR-006 — Agent Detection

| AC        | Test file                          | Test function                            |
|-----------|------------------------------------|------------------------------------------|
| AC-006.1..3 | `tests/fr006_agent_detection.rs` | (see file)                               |
| AC-006.4..6 | `tests/fr006_proc_tree.rs`       | (see file)                               |
| AC-006.7..8 | `tests/fr006_ps_agent_column.rs` | ps AGENT column + `--all` inventory      |
| AC-006.9    | `tests/fr006_thermal_tui_agents.rs`; `crates/sharecli-thermal-tui` | thermal TUI agent panel |
| AC-006.10   | `tests/fr006_agent_pid_watch.rs` | per-agent RSS/FD watch rows              |
| AC-006.11..13 | `tests/fr006_proc_cli.rs`; `src/commands/proc/mod.rs` | `sharecli proc` + JSON status/proc |
| AC-006.14   | `tests/fr006_proc_fingerprints.rs`; `crates/sharecli-fleet/src/detect.rs` | cmdline fingerprints |
| AC-006.15   | `tests/fr006_proc_watch.rs`; `src/commands/proc/mod.rs` | `sharecli proc --watch` live refresh |
| AC-006.16   | `tests/fr006_proc_tree_cli.rs`; `crates/sharecli-fleet/src/proc_scan.rs` (`build_agent_forests`); `src/commands/proc/mod.rs` | `sharecli proc --tree` parent-child forests |
| AC-006.17   | `tests/fr006_proc_filters.rs`; `src/commands/proc/mod.rs`; `crates/sharecli-fleet/src/resource_watch.rs` (`parse_rss_bytes`) | `sharecli proc --family` / `--min-rss` filters |
| AC-006.18   | `tests/fr006_proc_ndjson.rs`; `tests/fr006_proc_watch.rs`; `src/commands/proc/mod.rs` | `sharecli proc --watch --json` NDJSON stream |
| AC-006.19   | `tests/fr006_proc_sort.rs`; `src/commands/proc/mod.rs` | `sharecli proc --sort rss|fd|pid|state` |
| AC-006.20   | `tests/fr006_proc_fingerprints_ext.rs`; `tests/fr006_agent_detection.rs`; `crates/sharecli-fleet/src/detect.rs` | amp + expanded cmdline fingerprints |
| AC-006.21   | `tests/fr006_proc_limit.rs`; `src/commands/proc/mod.rs` | `sharecli proc --limit N` caps inventory/tree roots |
| AC-006.22   | `tests/fr006_thermal_tui_agent_tree.rs`; `crates/sharecli-thermal-tui/src/lib.rs` | thermal TUI agent forests via `build_host_agent_forests` |
| AC-006.23   | `tests/fr006_proc_pid_detail.rs`; `src/commands/proc/mod.rs`; `crates/sharecli-fleet/src/proc_scan.rs` (`lookup_proc`) | `sharecli proc --pid N` detail view |
| AC-006.24   | `tests/fr006_proc_csv.rs`; `src/commands/proc/mod.rs` | `sharecli proc --csv` flat inventory export |
| AC-006.25   | `tests/fr006_proc_ppid.rs`; `src/commands/proc/mod.rs`; `crates/sharecli-fleet/src/proc_scan.rs` (`lookup_proc`) | `sharecli proc --ppid N` parent-PID inventory filter |
| AC-006.26   | `tests/fr006_proc_tree_csv.rs`; `src/commands/proc/mod.rs` | `sharecli proc --tree --csv` forest inventory CSV export |
| AC-006.27   | `tests/fr006_proc_filters.rs`; `src/commands/proc/mod.rs`; `crates/sharecli-fleet/src/resource_watch.rs` (`parse_rss_bytes`) | `sharecli proc --max-rss` upper RSS bound filter |
| AC-006.28   | `tests/fr006_proc_filters.rs`; `src/commands/proc/mod.rs` | `sharecli proc --min-fd` / `--max-fd` FD band filters |
| AC-006.29   | `tests/fr006_proc_comm.rs`; `src/commands/proc/mod.rs` | `sharecli proc --comm` COMM substring filter |
| AC-006.30   | `tests/fr006_proc_cmdline.rs`; `src/commands/proc/mod.rs` | `sharecli proc --cmdline` joined argv/cmdline substring filter |
| AC-006.31   | `tests/fr006_proc_state.rs`; `src/commands/proc/mod.rs`; `crates/sharecli-fleet/src/proc_scan.rs` | `sharecli proc --state` process-state letter filter |
| AC-006.32   | `tests/fr006_proc_state_export.rs`; `src/commands/proc/mod.rs` | `state` on flat `--json` / `--csv` and `--tree --csv` exports |
| AC-006.33   | `tests/fr006_proc_state_text.rs`; `tests/fr006_proc_pid_detail.rs`; `src/commands/proc/mod.rs`; `src/commands/mod.rs` | `state` on flat text inventory and `proc --pid` detail |
| AC-006.34   | `tests/fr006_proc_tree_state.rs`; `src/commands/proc/mod.rs` | `state` on `--tree` text nodes and `--tree --json` rows |
| AC-006.35   | `tests/fr006_proc_tree_state.rs`; `src/commands/proc/mod.rs` (`collect_forest_pids`, `build_forest_state_map`) | live tree state for all forest PIDs (roots + children) |
| AC-006.36   | `tests/fr006_proc_sort.rs`; `src/commands/proc/mod.rs` (`ProcSort::State`, `sort_watched_agents`, `sort_agent_forests`) | `sharecli proc --sort state` process-state letter ordering |
| AC-006.37   | `tests/fr006_proc_ndjson.rs`; `src/commands/proc/mod.rs` (`emit_ndjson_line`, `AgentProcNdjsonLine`, `agent_row_from_watch`) | NDJSON watch agent rows include `state` (AC-006.32 parity); flushed stdout + stderr footer |
| AC-006.38   | `tests/fr006_proc_filters.rs`; `src/commands/proc/mod.rs` | `sharecli proc --exclude-family` negates `--family` |
| AC-006.39   | `tests/fr006_thermal_tui_agent_tree.rs`; `crates/sharecli-thermal-tui/src/lib.rs`; `crates/sharecli-fleet/src/proc_scan.rs` (`build_host_forest_state_map`, `state_text_for_pid`) | thermal TUI agent-tree process state letters |
| AC-006.40   | `tests/fr006_thermal_tui_agents.rs`; `crates/sharecli-thermal-tui/src/lib.rs`; `crates/sharecli-fleet/src/proc_scan.rs` (`build_host_agent_state_map`, `state_text_for_pid`) | thermal TUI flat agent-lines process state letters |
| AC-006.41   | `src/commands/proc/mod.rs` (`ProcSort`, `sort_watched_agents`, `sort_agent_forests`); `src/commands/proc/tests.rs` | `ProcSort` ordering applies consistently across flat inventory rows, watched-agent rows (text/JSON), and `--tree` root forests ordered by live samples (extends AC-006.19 and AC-006.36) |
| AC-006.12   | `tests/fr006_agent_rss_gate.rs`; `crates/sharecli-fleet/src/agent_contention.rs` | RSS-aware gate |

### FR-007 — Resource Watch

| AC        | Test file                                | Notes |
|-----------|------------------------------------------|-------|
| AC-007.1..3 | `tests/fr007_resource_thermal_watch.rs` | thermal as watch signal |
| AC-007.4    | `tests/fr007_resource_thermal_watch.rs` (`fr007_fd_watch_samples_self_fds`) | FD watch |
| AC-007.5    | `tests/fr007_resource_thermal_watch.rs` (`fr007_net_watch_samples_host_counters`) | net RX/TX watch |
| AC-007.6    | `tests/fr007_resource_thermal_watch.rs` (`fr007_hypervisor_run_carries_resource_watch`); `crates/sharecli-fleet/src/resource_watch.rs` | Hypervisor live watch path |
| AC-007.7    | `tests/fr007_resource_thermal_watch.rs` (`fr007_rss_watch_samples_self_rss`) | RSS watch |
| AC-007.8    | `tests/fr007_resource_thermal_watch.rs` (`fr007_load_watch_samples_host_load_1m`) | load average watch |
| AC-007.9    | `tests/fr004_status_health.rs`; `tests/fr007_thermal_tui_watch.rs` (`fr007_thermal_tui_fuse_coalesce_lines`); `crates/sharecli-fuse/src/read_cache.rs` | FUSE read-coalesce in status + thermal TUI |
| AC-007.10   | `tests/fr007_resource_thermal_watch.rs` (`fr007_format_status_section`); `tests/fr004_status_health.rs` (`fr004_status_prints_harness_table`); `src/commands/mod.rs` (`status`) | Host resource watch in status |
| AC-007.11   | `tests/fr007_thermal_tui_watch.rs`; `crates/sharecli-thermal-tui` | Host watch + FUSE meters in thermal TUI |
| AC-007.12   | `tests/fr007_thermal_tui_watch.rs` (`fr007_thermal_tui_resource_watch_net_lines`); `crates/sharecli-thermal-tui/src/lib.rs` (`resource_watch_lines`) | thermal TUI host net RX/TX parity with status |
| AC-007.13   | `tests/fr007_proc_json_host_watch.rs`; `src/commands/proc/mod.rs` (`AgentProcSnapshot::host_watch`); `src/monitoring.rs` (`HostResourceWatchJson`) | proc JSON host ResourceWatchSample parity |
| AC-007.14   | `tests/fr007_proc_text_csv_host_watch.rs`; `src/commands/proc/mod.rs` (`print_host_watch_text_footer`, `append_host_watch_csv_companion`); `src/monitoring.rs` (`HostResourceWatchJson::format_text_section`, `format_csv_companion`) | proc text/CSV host ResourceWatchSample parity |
| AC-007.15   | `tests/fr007_proc_tree_json_host_watch.rs`; `src/commands/proc/mod.rs` (`AgentTreeSnapshot::host_watch`) | proc tree JSON/NDJSON host ResourceWatchSample parity |
| AC-007.16   | `tests/fr007_proc_pid_json_host_watch.rs`; `src/commands/proc/mod.rs` (`ProcDetailSnapshot::host_watch`, `render_proc_detail`) | proc pid JSON/text host ResourceWatchSample parity |
| AC-007.17   | `tests/fr007_proc_pid_gate.rs`; `src/commands/proc/mod.rs` (`ProcDetailSnapshot::gate`, `render_proc_detail`); `crates/sharecli-fleet/src/agent_contention.rs` (`format_gate_status_from_snapshot`) | proc pid JSON/text thermal gate parity |
| AC-007.18   | `tests/fr007_proc_tree_json_gate.rs`; `src/commands/proc/mod.rs` (`AgentTreeSnapshot::gate`) | proc tree JSON/NDJSON thermal gate parity |
| AC-007.19   | `tests/fr007_proc_text_csv_gate.rs`; `src/commands/proc/mod.rs` (`append_proc_csv_companions`); `crates/sharecli-fleet/src/agent_contention.rs` (`GateStatusSnapshot::format_csv_companion`) | proc flat/tree CSV thermal gate companion parity |
| AC-007.20   | `tests/fr007_proc_tree_text_gate.rs`; `src/commands/proc/mod.rs` (`render_once` tree text path) | proc tree text thermal gate section before host watch footer |
| AC-007.21   | `tests/fr007_proc_text_gate.rs`; `src/commands/proc/mod.rs` (`render_once` flat text path) | proc flat text thermal gate section before host watch footer |
| AC-007.22   | `tests/fr007_proc_watch_gate_order.rs`; `src/commands/proc/mod.rs` (`run` watch loop + `render_once`) | proc watch text/NDJSON gate → host_watch ordering per refresh |
| AC-007.23   | `tests/fr007_proc_tree_watch_gate_order.rs`; `src/commands/proc/mod.rs` (`run` watch loop + `render_once` tree path) | proc tree watch text/NDJSON gate → host_watch ordering per refresh |
| AC-007.24   | `tests/fr007_proc_json_gate_order.rs`; `src/commands/proc/mod.rs` (`AgentProcSnapshot`, `AgentTreeSnapshot`, `ProcDetailSnapshot`) | proc one-shot JSON gate → host_watch raw key ordering |
| AC-007.25   | `tests/fr007_status_json_host_watch.rs`; `tests/fr006_proc_cli.rs`; `src/commands/mod.rs` (`status`) | status --json top-level gate + host_watch siblings (proc parity) |
| AC-007.26   | `tests/fr007_thermal_tui_gate_parity.rs`; `crates/sharecli-thermal-tui/src/lib.rs` (`gate_panel_lines`, `render_decision`); `crates/sharecli-fleet/src/resource_watch.rs` (`sum_detected_agent_rss_bytes`) | thermal TUI gate panel RSS-aware snapshot parity |
| AC-007.27   | `tests/fr007_status_text_gate_order.rs`; `src/commands/mod.rs` (`status` text path) | status text thermal gate section before host watch (proc parity) |
| AC-007.28   | `tests/fr007_proc_watch_stderr_footer.rs`; `src/commands/proc/mod.rs` (`eprint_gate_host_watch_stderr_companions`, `render_once` NDJSON path) | proc watch NDJSON stderr gate → host_watch companions; stdout pipe-clean |
| AC-007.29   | `tests/fr007_proc_tree_watch_stderr_footer.rs`; `src/commands/proc/mod.rs` (`eprint_gate_host_watch_stderr_companions`, `render_once` tree NDJSON path) | proc tree watch NDJSON stderr gate → host_watch companions; stdout pipe-clean |
| AC-007.30   | `tests/fr007_proc_json_stderr_silent.rs`; `src/commands/proc/mod.rs` (`render_once` one-shot JSON paths) | proc one-shot JSON stderr silent; gate/host_watch JSON body only |
| AC-007.31   | `tests/fr007_proc_json_stderr_silent.rs`; `src/commands/proc/mod.rs` (`render_proc_detail` JSON path) | proc --pid one-shot JSON stderr silent; gate/host_watch JSON body only |
| AC-007.32   | `tests/fr007_status_json_stderr_silent.rs`; `src/commands/mod.rs` (`status` JSON path) | status one-shot JSON stderr silent; gate/host_watch JSON body only |
| AC-007.33   | `tests/fr007_proc_csv_stderr_silent.rs`; `src/commands/proc/mod.rs` (`render_once` one-shot CSV paths) | proc one-shot CSV stderr silent; gate/host_watch CSV companion rows only |
| AC-007.34   | `tests/fr007_proc_text_stderr_silent.rs`; `src/commands/proc/mod.rs` (`render_once` one-shot text paths, `render_proc_detail` text path) | proc one-shot text stderr silent; gate/host_watch stdout text sections only |
| AC-007.35   | `tests/fr007_proc_watch_text_stderr_silent.rs`; `src/commands/proc/mod.rs` (`run` watch loop text path, `render_once` text paths) | proc watch text stderr silent; gate/host_watch + `[watch]` footer stdout only |
| AC-007.36   | `tests/fr007_status_text_stderr_silent.rs`; `src/commands/mod.rs` (`status` text path) | status one-shot text stderr silent; gate/host_watch stdout text sections only |
| AC-007.37   | `tests/fr007_health_pool_text_stderr_silent.rs`; `src/commands/mod.rs` (`health`, `pool`) | health/pool one-shot text gate → host_watch on stdout; stderr silent on success |
| AC-007.38   | `tests/fr007_ps_all_text_stderr_silent.rs`; `src/commands/mod.rs` (`print_host_agent_scan`, `ps --all`) | ps --all one-shot text gate → host_watch on stdout after inventory; stderr silent on success |
| AC-007.39   | `tests/fr007_report_text_stderr_silent.rs`; `src/commands/report.rs` (`render_once` text path) | report text one-shot + --watch gate → host_watch on stdout after report body; stderr silent on success |
| AC-007.40   | `tests/fr007_report_json_gate_host_watch.rs`; `src/commands/report.rs` (`FleetReportJson`, `render_once` JSON path) | report --format json gate → host_watch top-level siblings; stderr silent on success |
| AC-007.41   | `tests/fr007_dashboard_ws_operator_envelope.rs`; `src/commands/serve.rs` (`DashboardWsSnapshot`, `build_dashboard_ws_snapshot`); `src/dashboard.html` | serve `/ws` gate → host_watch JSON envelope + dashboard operator panels |
| AC-007.42   | `tests/fr007_report_watch_json_gate_host_watch.rs`; `src/commands/report.rs` (`FleetReportNdjsonLine`, `run` watch NDJSON path); `src/commands/mod.rs` (`eprint_live_gate_host_watch_sections`) | report --watch --format json NDJSON gate → host_watch per refresh; stderr text companions |
| AC-007.43   | `tests/fr007_ps_all_json_gate_host_watch.rs`; `src/commands/mod.rs` (`PsAllJson`, `ps --all --json`) | ps --all --json gate → host_watch after pool + agent inventory; stderr silent on success; --json requires --all |
| AC-007.44   | `tests/fr007_health_pool_json_gate_host_watch.rs`; `src/commands/mod.rs` (`HealthJson`, `PoolJson`, `health --json`, `pool --json`) | health/pool --json gate → host_watch after runtime fields; stderr silent on success |
| AC-007.45   | `tests/fr007_ipc_health_status_gate_host_watch.rs`; `crates/sharecli-ipc/src/handler.rs` (`HealthSnapshot`, `health.status`); `crates/sharecli-tray-linux/src/ipc.rs`; `desktop/ShareCLITray/Sources/ShareCLICore/IPCClient.swift` | IPC health.status gate → host_watch after runtime fields; tray/desktop wire parity with health --json AC-007.44 |
| AC-007.46   | `tests/fr007_ipc_monitoring_report_gate_host_watch.rs`; `crates/sharecli-ipc/src/handler.rs` (`MonitoringReportSnapshot`, `monitoring.report`) | IPC monitoring.report gate → host_watch after fleet monitoring fields; parity with report --json AC-007.40 and health.status AC-007.45 |
| AC-007.47   | `crates/sharecli-tray-linux/src/ipc.rs` (`MonitoringReportSnapshot`, `monitoring_report`); `desktop/ShareCLITray/Sources/ShareCLICore/IPCClient.swift` (`monitoringReport`); `tests/fr007_ipc_monitoring_report_gate_host_watch.rs` | Tray/desktop wire parity for monitoring.report; decode gate + host_watch via IPC RPC without report --json shell-out |
| AC-007.48   | `crates/sharecli-tray-linux/src/ipc.rs` (`health_snapshot`, `process_summaries`); `crates/sharecli-tray-linux/src/main.rs` (`refresh`); `desktop/ShareCLITray/Sources/ShareCLICore/AppState.swift` (`refresh`); `desktop/ShareCLITray/Sources/ShareCLICore/IPCClient.swift` (`asHealthSnapshot`, `asProcessSummaries`); `tests/fr007_tray_monitoring_report_consume.rs` | Tray refresh single monitoring.report snapshot drives gate/host_watch + process inventory; no split health + process.list polls |
| AC-007.49   | `tests/fr007_ps_all_watch_json_gate_host_watch.rs`; `src/commands/mod.rs` (`PsAllNdjsonLine`, `ps --all --watch --json`, `render_ps_once` watch NDJSON path); `src/commands/mod.rs` (`eprint_live_gate_host_watch_sections`) | ps --all --watch --json NDJSON gate → host_watch per refresh; stderr text companions; --watch --json requires --all |
| AC-007.50   | `tests/fr007_ps_all_watch_text_stderr_silent.rs`; `src/commands/mod.rs` (`ps` watch loop text path, `print_host_agent_scan`, `render_ps_once` text path) | ps --all --watch text stderr silent; gate/host_watch + `[watch]` footer stdout only |
| AC-007.51   | `crates/sharecli-tray-windows/src/ipc.rs` (`health_snapshot`, `process_summaries`); `windows/ShareCLITray/MonitoringReportSnapshot.cs` (`AsHealthSnapshot`, `AsProcessSummaries`); `windows/ShareCLITray/TrayWindow.xaml.cs` (`RefreshDataAsync`); `tests/fr007_tray_windows_monitoring_report_consume.rs` | Windows tray refresh single monitoring.report snapshot drives gate/host_watch + process inventory; no split health + process.list polls |
| AC-007.52   | `crates/sharecli-tray-windows/src/poll.rs` (`TRAY_POLL_INTERVAL_SECS`, `tray_poll_interval`); `windows/ShareCLITray/TrayPoll.cs` (`IntervalSeconds`); `windows/ShareCLITray/TrayWindow.xaml.cs` (`DispatcherQueueTimer`, `StartPeriodicPolling`); `tests/fr007_tray_windows_poll_interval.rs` | Windows tray ~3 s periodic poll via DispatcherQueueTimer; same monitoring.report refresh path as AC-007.51; parity with Linux/Swift tray cadence |
| AC-007.53   | `crates/sharecli-tray-linux/src/poll.rs` (`TRAY_POLL_INTERVAL_SECS`, `tray_poll_interval`); `crates/sharecli-tray-linux/src/main.rs` (`tray_poll_interval`, poll loop); `desktop/ShareCLITray/Sources/ShareCLICore/TrayPoll.swift` (`intervalSeconds`); `desktop/ShareCLITray/Sources/ShareCLICore/AppState.swift` (`startPolling`, `TrayPoll.intervalSeconds`); `desktop/ShareCLITray/Sources/ShareCLICore/PollLoop.swift` (`PollLoop`, `PollCadence`); `tests/fr007_tray_linux_poll_interval.rs`; `tests/fr007_tray_swift_poll_interval.rs` | Linux/Swift tray ~3 s periodic poll via shared TRAY_POLL_INTERVAL_SECS; same monitoring.report refresh path as AC-007.48; parity with Windows AC-007.52 |
| AC-007.54   | `crates/sharecli-tray-windows/src/ipc.rs` (`IPC_METHOD_KILL`, `kill_request_json`, `kill_all_request_json`); `windows/ShareCLITray/IpcKill.cs` (`TryKill`, `TryKillAll`); `windows/ShareCLITray/TrayWindow.xaml.cs` (`OnKillProcessClick`, `OnKillAllClick`, `RefreshDataAsync`); `crates/sharecli-tray-linux/src/ipc.rs` (`kill`, `kill_all`); `crates/sharecli-tray-linux/src/main.rs` (Kill submenu + Kill All Managed); `desktop/ShareCLITray/Sources/ShareCLICore/AppState.swift` (`kill`, `killAll`); `desktop/ShareCLITray/Sources/ShareCLICore/IPCClient.swift`; `tests/fr007_tray_windows_kill.rs`; `tests/fr007_tray_linux_kill.rs`; `tests/fr007_tray_swift_kill.rs` | Windows tray Kill + Kill All Managed → IPC process.kill/kill_all + post-kill monitoring.report refresh; parity with Linux/Swift tray operator kill paths |
| AC-007.55   | `crates/sharecli-tray-windows/src/ipc.rs` (`process_summaries`, `ProcessSummary.harness`); `windows/ShareCLITray/MonitoringReportSnapshot.cs` (`AsProcessSummaries`, `MonitoringProcessEntry.Harness`); `windows/ShareCLITray/TrayWindow.xaml.cs` (`ProcessInfo.harness`); `windows/ShareCLITray/TrayWindow.xaml` (Harness DataGrid column); `tests/fr007_tray_windows_harness.rs`; `tests/fr007_tray_windows_monitoring_report_consume.rs` | Windows tray process grid harness column parity; monitoring.report harness → ProcessInfo + DataGrid; Linux submenu + Swift DashboardView reference |
| AC-007.56   | `crates/sharecli-tray-linux/src/operator_display.rs`; `crates/sharecli-tray-linux/src/main.rs` (menu + tooltip); `crates/sharecli-tray-windows/src/operator_display.rs`; `desktop/ShareCLITray/Sources/ShareCLICore/OperatorDisplay.swift`; `desktop/ShareCLITray/Sources/ShareCLITray/TrayPopoverView.swift`; `desktop/ShareCLITray/Sources/ShareCLITray/DashboardView.swift` (`HealthView`); `windows/ShareCLITray/OperatorDisplay.cs`; `windows/ShareCLITray/TrayWindow.xaml` (`GateStatusText`, `HostWatchStatusText`); `windows/ShareCLITray/TrayWindow.xaml.cs`; `tests/fr007_tray_gate_host_watch_ui.rs` | Tray gate → host_watch UI parity across Linux/Swift/Windows; reusable format helpers map thermal gate + host watch fields into tray display strings; extends AC-007.48/AC-007.51 monitoring.report consume |
| AC-007.57   | `crates/sharecli-tray-linux/src/operator_display.rs` (`resolve_tray_gate_visual`, `TrayGateVisual`); `crates/sharecli-tray-windows/src/operator_display.rs`; `crates/sharecli-tray-linux/src/main.rs` (`icon_name`, `status`, `gate_visual`); `desktop/ShareCLITray/Sources/ShareCLICore/OperatorDisplay.swift`; `desktop/ShareCLITray/Sources/ShareCLITray/AppEntry.swift`; `desktop/ShareCLITray/Sources/ShareCLITray/TrayPopoverView.swift`; `windows/ShareCLITray/OperatorDisplay.cs`; `windows/ShareCLITray/TrayWindow.xaml`; `tests/fr007_tray_thermal_visual.rs` | Tray icon/badge/color from thermal_pressure + gate_decision; dashboard CSS token parity; golden severity matrix |
| AC-007.58   | `desktop/ShareCLITray/Sources/ShareCLITray/DashboardView.swift` (`HealthView`, `gateVisual`, `thermalGateBadge`); `desktop/ShareCLITray/Sources/ShareCLICore/OperatorDisplay.swift` (`resolveTrayGateVisual`); `tests/fr007_tray_thermal_visual.rs` | Swift dashboard HealthView metric cards + thermal gate detail rows use resolveTrayGateVisual severity tokens; replaces generic healthy/warning Status card |
| AC-007.59   | `desktop/ShareCLITray/Sources/ShareCLITray/TrayPopoverView.swift` (`statsRow`, `gateVisual`); `desktop/ShareCLITray/Sources/ShareCLICore/OperatorDisplay.swift` (`resolveTrayGateVisual`); `tests/fr007_tray_thermal_visual.rs` | Swift tray popover stats row Status cell uses resolveTrayGateVisual severity tokens; replaces generic healthy/warning stat cell for full Swift surface parity |
| AC-007.60   | `windows/ShareCLITray/OperatorDisplay.cs` (`FormatHealthStatusLine`, `FormatHealthStatusOfflineLine`); `windows/ShareCLITray/TrayWindow.xaml.cs` (`HealthStatusText`, `ResolveTrayGateVisual`, `SeverityBrush`); `tests/fr007_tray_thermal_visual.rs` | WinUI HealthStatusText summary uses ResolveTrayGateVisual severity tokens; replaces generic OK/Unhealthy health line for Swift AC-007.58/59 parity |
| AC-007.61   | `crates/sharecli-tray-linux/src/operator_display.rs` (`format_tray_tooltip_summary_line`, `format_tray_tooltip_offline_line`); `crates/sharecli-tray-linux/src/main.rs` (`tool_tip`, `gate_visual`); `tests/fr007_tray_thermal_visual.rs` | Linux SNI tooltip summary uses gate_visual.badge_label severity tokens; replaces generic UNHEALTHY suffix for Swift/Windows tray thermal trilogy parity |
| AC-007.62   | `desktop/ShareCLITray/Sources/ShareCLICore/OperatorDisplay.swift` (`formatMenuBarTitleLine`, `formatMenuBarTitleOfflineLine`); `desktop/ShareCLITray/Sources/ShareCLITray/AppEntry.swift` (`formatMenuBarTitleLine`, `resolveTrayGateVisual`, `badgeLabel`); `tests/fr007_tray_thermal_visual.rs` | Swift NSStatusItem menu bar title leads with gateVisual.badgeLabel severity tokens; replaces bare managed/memory title for Linux/Windows tray thermal trilogy parity |
| AC-007.63   | `crates/sharecli-tray-linux/src/operator_display.rs` (`format_tray_menu_header_line`, `format_tray_menu_header_offline_line`); `crates/sharecli-tray-linux/src/main.rs` (`menu`, `gate_visual`); `tests/fr007_tray_thermal_visual.rs` | Linux SNI menu header leads with gate_visual.badge_label severity tokens; replaces bare process/memory header and generic Daemon offline fallback for tray thermal trilogy parity |
| AC-007.64   | `tests/fr007_health_watch_text_stderr_silent.rs`; `tests/fr007_health_watch_json_gate_host_watch.rs`; `src/commands/mod.rs` (`HealthNdjsonLine`, `render_health_once`, `health --watch`, `health --watch --json`); `src/main.rs` (`Health.watch`) | health --watch text stderr silent; gate/host_watch + `[watch]` footer stdout only; health --watch --json NDJSON gate → host_watch per refresh with stderr text companions |
| AC-007.65   | `tests/fr007_pool_watch_text_stderr_silent.rs`; `tests/fr007_pool_watch_json_gate_host_watch.rs`; `src/commands/mod.rs` (`PoolNdjsonLine`, `render_pool_once`, `pool --watch`, `pool --watch --json`); `src/main.rs` (`Pool.watch`) | pool --watch text stderr silent; gate/host_watch + `[watch]` footer stdout only; pool --watch --json NDJSON gate → host_watch per refresh with stderr text companions |
| AC-007.66   | `tests/fr007_status_watch_text_stderr_silent.rs`; `tests/fr007_status_watch_json_gate_host_watch.rs`; `src/commands/mod.rs` (`StatusJson`, `StatusNdjsonLine`, `render_status_once`, `status --watch`, `status --watch --json`); `src/main.rs` (`Status.watch`) | status --watch text stderr silent; gate/host_watch + `[watch]` footer stdout only; status --watch --json NDJSON gate → host_watch per refresh with stderr text companions |
| AC-007.67   | `tests/fr007_ipc_pool_status_snapshot.rs`; `crates/sharecli-ipc/src/handler.rs` (`PoolSnapshot`, `StatusSnapshot`, `pool.status`, `status.snapshot`); `src/commands/proc/mod.rs` (`AgentProcRow` Deserialize for wire roundtrip) | IPC pool.status + status.snapshot gate → host_watch after pool/status fields; parity with pool --json AC-007.44 and status --json AC-007.25 |
| AC-007.68   | `crates/sharecli-tray-linux/src/ipc.rs` (`PoolSnapshot`, `StatusSnapshot`, `pool_status`, `status_snapshot`); `crates/sharecli-tray-windows/src/ipc.rs`; `desktop/ShareCLITray/Sources/ShareCLICore/IPCClient.swift` (`poolStatus`, `statusSnapshot`); `windows/ShareCLITray/PoolStatusSnapshot.cs`; `tests/fr007_ipc_pool_status_tray_wire.rs` | Tray/desktop wire parity for pool.status + status.snapshot; decode gate + host_watch via IPC RPC without pool/status --json shell-out |
| AC-007.69   | `tests/fr007_tray_pool_status_consume.rs`; `crates/sharecli-tray-linux/src/operator_display.rs` (`format_pool_tray_line`, `format_status_snapshot_tray_line`); `crates/sharecli-tray-linux/src/main.rs` (`refresh` pool/status operator lines); `desktop/ShareCLITray/Sources/ShareCLICore/AppState.swift`; `desktop/ShareCLITray/Sources/ShareCLITray/TrayPopoverView.swift`; `desktop/ShareCLITray/Sources/ShareCLITray/DashboardView.swift`; `windows/ShareCLITray/TrayWindow.xaml.cs`; `windows/ShareCLITray/OperatorDisplay.cs` | Tray operator panels surface pool + proc-scan lines; supplementary IPC superseded by AC-007.72 embedded monitoring.report siblings |
| AC-007.70   | `tests/fr007_dashboard_ws_operator_envelope.rs`; `src/commands/serve.rs` (`DashboardWsSnapshot`, `build_dashboard_ws_snapshot`); `src/commands/mod.rs` (`build_pool_json`, `build_status_json`); `src/dashboard.html` | serve `/ws` gate → host_watch → pool → status → agents → processes JSON envelope + dashboard operator panels |
| AC-007.71   | `tests/fr007_thermal_tui_pool_status.rs`; `crates/sharecli-fleet/src/operator_pool_status.rs`; `crates/sharecli-thermal-tui/src/lib.rs` (`pool_panel_lines`, `status_panel_lines`, `run_with_pool_status`); `src/main.rs` (`build_pool_json` / `build_status_json` poll hook); `src/commands/mod.rs` (`From<PoolJson>`, `From<StatusJson>`) | Thermal TUI pool + proc-scan operator panels parity with tray AC-007.69 / dashboard AC-007.70 |
| AC-007.72   | `tests/fr007_ipc_monitoring_report_pool_status.rs`; `tests/fr007_tray_pool_status_consume.rs`; `crates/sharecli-ipc/src/handler.rs` (`MonitoringReportSnapshot`, `monitoring.report`); `crates/sharecli-tray-linux/src/ipc.rs`; `crates/sharecli-tray-windows/src/ipc.rs`; `crates/sharecli-tray-linux/src/main.rs`; `desktop/ShareCLITray/Sources/ShareCLICore/IPCClient.swift`; `desktop/ShareCLITray/Sources/ShareCLICore/AppState.swift`; `windows/ShareCLITray/MonitoringReportSnapshot.cs`; `windows/ShareCLITray/TrayWindow.xaml.cs` | IPC monitoring.report embeds gate → host_watch → pool → status; tray refresh single round-trip (no supplementary pool.status / status.snapshot) |
| AC-007.73   | `tests/fr007_report_json_pool_status.rs`; `tests/fr007_report_json_gate_host_watch.rs`; `tests/fr007_report_watch_json_gate_host_watch.rs`; `src/commands/report.rs` (`FleetReportJson`, `FleetReportNdjsonLine`, `render_once` JSON path); `src/commands/mod.rs` (`build_pool_json`, `build_status_json`, `PoolJson`, `StatusJson`) | report --format json gate → host_watch → pool → status top-level siblings; watch NDJSON parity; stderr silent on one-shot success |
| AC-007.74   | `tests/fr007_report_text_pool_status.rs`; `tests/fr007_report_text_stderr_silent.rs`; `src/commands/report.rs` (`render_once` text path); `src/commands/mod.rs` (`print_live_pool_status_operator_sections`); `crates/sharecli-fleet/src/operator_pool_status.rs` | report text one-shot + --watch gate → host_watch → pool → proc-scan operator lines on stdout; stderr silent on success |
| AC-007.75   | `tests/fr007_proc_text_pool_status.rs`; `tests/fr007_proc_text_stderr_silent.rs`; `tests/fr007_proc_watch_text_stderr_silent.rs`; `src/commands/proc/mod.rs` (`render_once` text path); `src/commands/mod.rs` (`print_live_pool_status_operator_sections`); `crates/sharecli-fleet/src/operator_pool_status.rs` | proc + proc --tree text one-shot + --watch gate → host_watch → pool → proc-scan operator lines on stdout; stderr silent on success |
| AC-007.76   | `tests/fr007_health_pool_status_ps_text_pool_status.rs`; `tests/fr007_health_pool_text_stderr_silent.rs`; `tests/fr007_health_watch_text_stderr_silent.rs`; `tests/fr007_pool_watch_text_stderr_silent.rs`; `tests/fr007_status_watch_text_stderr_silent.rs`; `tests/fr007_ps_all_text_stderr_silent.rs`; `tests/fr007_ps_all_watch_text_stderr_silent.rs`; `src/commands/mod.rs` (`render_health_once`, `render_pool_once`, `render_status_once`, `render_ps_once`, `print_live_pool_status_operator_sections`); `crates/sharecli-fleet/src/operator_pool_status.rs` | health/pool/status/ps --all text one-shot + --watch gate → host_watch → pool → proc-scan operator lines on stdout; stderr silent on success |
| AC-007.77   | `tests/fr007_operator_json_pool_status.rs`; `tests/fr007_health_pool_json_gate_host_watch.rs`; `tests/fr007_pool_watch_json_gate_host_watch.rs`; `tests/fr007_status_watch_json_gate_host_watch.rs`; `tests/fr007_ps_all_json_gate_host_watch.rs`; `tests/fr007_ps_all_watch_json_gate_host_watch.rs`; `tests/fr007_proc_json_gate_order.rs`; `src/commands/mod.rs` (`HealthJson`, `PoolJson`, `StatusJson`, `PsAllJson`, `fetch_operator_pool_status_siblings`, `build_health_json`, `render_pool_once`, `render_status_once`, `build_ps_all_json`); `src/commands/proc/mod.rs` (`AgentProcSnapshot`, `AgentTreeSnapshot`, `render_once` JSON path) | health/ps/proc --json gate → host_watch → pool → status; pool --json adds nested status only; status --json adds nested pool only; watch NDJSON parity; stderr silent on one-shot success |
| AC-007.78   | `tests/fr007_ipc_health_pool_status.rs`; `tests/fr007_ipc_health_status_gate_host_watch.rs`; `tests/fr007_ipc_pool_status_snapshot.rs`; `crates/sharecli-ipc/src/handler.rs` (`HealthSnapshot`, `PoolSnapshot`, `StatusSnapshot`, `health.status`, `pool.status`, `status.snapshot`, `capture_pool_snapshot`, `capture_status_snapshot`) | IPC health.status embeds pool + status; pool.status nested status only; status.snapshot nested pool only; gate → host_watch → siblings key order; no cross-sibling recursion |
| AC-007.79   | `tests/fr007_proc_csv_pool_status.rs`; `tests/fr007_proc_csv_stderr_silent.rs`; `tests/fr007_proc_text_csv_gate.rs`; `tests/fr007_proc_text_csv_host_watch.rs`; `src/commands/proc/mod.rs` (`append_proc_csv_companions`); `crates/sharecli-fleet/src/operator_pool_status.rs` (`PoolOperatorPanel::format_csv_companion`, `StatusOperatorPanel::format_csv_companion`) | proc --csv / --tree --csv gate → host_watch → pool → status companion rows on stdout; stderr silent on success (extends AC-007.33) |
| AC-007.80   | `tests/fr007_ws_client_health_update_pool_status.rs`; `crates/sharecli-ipc/src/ws_client.rs` (`ClientMessage`, `ClientMessage::from_json`, `HealthUpdate`); `crates/sharecli-ipc/src/handler.rs` (`HealthSnapshot`) | WS `health_update` decodes expanded HealthSnapshot pool + status; IPC health.status wrap roundtrip; legacy missing siblings → Unknown |
| AC-007.81   | `tests/fr007_report_csv_pool_status.rs`; `tests/fr007_report_csv_stderr_silent.rs`; `src/commands/report.rs` (`ReportFormat::Csv`, `render_report_csv_body`, `append_report_csv_companions`, `render_once` CSV path); `crates/sharecli-fleet/src/operator_pool_status.rs` | report --format csv fleet body → gate → host_watch → pool → status companion rows on stdout; rejects --watch; stderr silent on success (parity with proc CSV AC-007.79) |
| AC-007.82   | `tests/fr007_health_pool_status_csv.rs`; `src/commands/mod.rs` (`render_health_csv_body`, `render_pool_csv_body`, `render_status_csv_body`, `append_operator_csv_companions`, `health --csv`, `pool --csv`, `status --csv`); `src/main.rs` (`Health.csv`, `Pool.csv`, `Status.csv`); `crates/sharecli-fleet/src/operator_pool_status.rs` | health/pool/status --csv command body → gate → host_watch → pool → status companion rows on stdout; rejects --json/--watch; stderr silent on success (parity with proc/report CSV AC-007.79/81) |
| AC-007.83   | `tests/fr007_ps_all_csv.rs`; `src/commands/render_csv.rs` (`render_ps_all_csv_body`); `src/main.rs` (`ps --all --csv` help text) | `sharecli ps --all --csv` emits the managed-process + agent-inventory CSV body followed by companion `gate` → `host_watch` → `pool` → `status` CSV records; rejects `--json` (parity with AC-007.82) |
| AC-007.84   | `tests/fr007_operator_envelope_parity_suite.rs` (`fr007_operator_matrix_cli_text_one_shot`, `fr007_operator_matrix_cli_json_one_shot`, `fr007_operator_matrix_ipc_envelopes`) | Operator envelope matrix: CLI one-shot text body → `gate` → `host_watch` → `pool` → proc-scan and one-shot JSON `gate` → `host_watch` → `pool`/`status` siblings across proc/report/health/pool/status/`ps --all` (stderr silent) |
| AC-007.85   | `tests/fr007_operator_envelope_parity_suite.rs` (`fr007_operator_matrix_cli_text_one_shot`, `fr007_operator_matrix_cli_json_one_shot`, `fr007_operator_matrix_cli_csv_one_shot`) | `sharecli proc --tree` one-shot text / `--json` / `--csv` passes the same operator envelope matrix contract for the tree renderer |
| AC-007.86   | `src/commands/proc/mod.rs` (`ProcDetailSnapshot` pool/status operator panels, `render_proc_detail_csv`); `tests/fr007_operator_envelope_parity_suite.rs` | `sharecli proc --pid` one-shot text / `--json` / `--csv` passes the operator envelope matrix, including pool and proc-scan panel parity with AC-007.77 |
| AC-007.87   | `tests/fr007_proc_pid_watch.rs`; `tests/fr007_operator_json_pool_status.rs` (`fr007_proc_pid_watch_ndjson_pool_status_ordering`); `src/commands/proc/mod.rs` (`ProcDetailNdjsonLine`) | `sharecli proc --pid N --watch [secs]` keeps text/NDJSON watch parity with flat `proc --watch`: stderr silent, `[watch]` footer on stdout (text), pipe-clean NDJSON, gate → `host_watch` → `pool` → `status` companions |
| AC-007.88   | `tests/fr007_proc_csv_watch.rs`; `src/commands/proc/mod.rs` (`PROC_CSV_WATCH_FRAME_MARKER`) | `sharecli proc --csv --watch` (flat and `--tree`) emits per-frame inventory plus `gate` → `host_watch` → `pool` → `status` companions with stderr silent |
| AC-007.89   | `tests/fr007_operator_csv_watch.rs`; `src/commands/mod.rs` (`HEALTH_CSV_WATCH_FRAME_MARKER`, `POOL_CSV_WATCH_FRAME_MARKER`, `STATUS_CSV_WATCH_FRAME_MARKER`, `PS_CSV_WATCH_FRAME_MARKER`, `emit_operator_csv_watch_frame`, `emit_operator_csv_watch_footer`) | `health`/`pool`/`status`/`ps --all` `--csv --watch` emit command-specific CSV body plus `gate` → `host_watch` → `pool` → `status` companions per frame with stderr silent |
| AC-007.90   | `tests/fr007_report_csv_watch.rs`; `src/commands/report.rs` (`REPORT_CSV_WATCH_FRAME_MARKER`) | `sharecli report --format csv --watch` emits the fleet CSV body plus companions per frame with stderr silent; `--format csv --watch 0` stays rejected |
| AC-007.91   | `tests/fr007_proc_pid_csv_watch.rs`; `src/commands/proc/mod.rs` (`PROC_PID_CSV_WATCH_FRAME_MARKER`) | `sharecli proc --pid --csv --watch` emits the PID detail body plus companions per frame with stderr silent; `--pid --csv --json --watch` stays rejected |
| AC-007.92   | `tests/fr007_proc_pid_combo_rejects.rs`; `src/commands/proc/mod.rs` (`reject_pid_inventory_combos`) | `sharecli proc --pid` loudly rejects inventory-only flags (`--tree`, `--family`, `--sort --limit`) while `--pid --csv` remains allowed |
| AC-007.93   | `tests/fr007_operator_envelope_parity_suite.rs` (`fr007_operator_matrix_cli_csv_watch_frame_smoke`, `assert_csv_watch_frame_smoke`) | CSV `--watch` frame-marker smoke (short dwell, ≥1 frame): stderr silent, no ANSI clear, frame marker emitted, `# [watch]` footer comment included |
| AC-007.94   | `tests/fr007_proc_csv_watch.rs` (`fr007_proc_csv_watch_footer_flushed_same_tick`); `src/commands/mod.rs` (`emit_operator_csv_watch_footer`); `src/commands/proc/mod.rs`; `src/commands/report.rs` | The CSV `# [watch]` footer flushes in the same tick as the CSV body so pipe consumers see it immediately |
| AC-007.95   | `tests/fr007_operator_envelope_parity_suite.rs` (`fr007_operator_matrix_cli_csv_watch_footer_same_tick`, `assert_csv_watch_footer_same_tick`) | The CSV `# [watch]` footer flushes in the same tick as its frame (exactly one frame marker before the first footer) |
| AC-007.96   | `tests/fr007_proc_watch_text_stderr_silent.rs` (`fr007_proc_watch_text_footer_flushed_same_tick`); `src/commands/proc/mod.rs`; `src/commands/report.rs` | The text `[watch]` footer flushes in the same tick as the watch body |
| AC-007.97   | `tests/fr007_operator_envelope_parity_suite.rs` (`fr007_operator_matrix_cli_text_watch_footer_same_tick`, `assert_text_watch_footer_same_tick`) | Text `--watch` `[watch]` footer flushes in the same tick across the full matrix (exactly one gate before the first footer) |

### FR-008 — Coalesce

| AC        | Test file                        | Notes |
|-----------|----------------------------------|-------|
| AC-008.1..4 | `tests/fr008_coalesce_mesh.rs` | command_key / with_lock / thermal / cache hit |
| AC-008.5 | `tests/fr008_coalesce_mesh.rs` (`fr008_ttl_stale_entry_is_miss`); `sharecli-ipc` `ttl_lookup_miss_and_evict_on_store` | TTL miss + eviction |
| AC-008.6 | `tests/fr008_coalesce_mesh.rs` (`fr008_debounce_waits_and_shares`, `fr008_hypervisor_debounce_waits_and_shares`); `sharecli-ipc` `debounce_shares_recent_store` | debounce share window |
| AC-008.7..9 | `tests/fr008_coalesce_mesh.rs` (`fr008_nocache_*`, `fr008_slot_queue_*`, `fr008_hypervisor_nocache_*`) | nocache bypass + SlotQueue + Hypervisor queue route |
| AC-008.10 | `tests/e2e_hypervisor_nocache.rs` | Hypervisor nocache e2e: re-exec, serialize, coalesce isolation |
| AC-008.11 | `tests/fr008_coalesce_status.rs`; `tests/fr004_status_health.rs`; `tests/fr007_thermal_tui_watch.rs`; `sharecli-ipc` `global_coalesce_meters_record_hit_miss_and_nocache` | coalesce operator meters in status + thermal TUI |
| AC-008.12 | `tests/fr008_coalesce_status.rs`; `tests/fr004_status_health.rs`; `tests/fr007_thermal_tui_watch.rs`; `crates/sharecli-fleet/src/slot_queue_meters.rs` | SlotQueue acquire/wait/timeout in status + thermal TUI |
| AC-008.13 | `tests/fr008_coalesce_mesh.rs` (`fr008_command_key_cwd_env_dimensions`, `fr008_hypervisor_cache_respects_cwd_and_env`); `crates/sharecli-ipc/src/lib.rs` (`command_key`); `crates/sharecli-core/src/lib.rs` (`Hypervisor::run`) | command_key cwd/env dimensions + Hypervisor cache isolation |
| AC-008.14 | `tests/fr008_coalesce_mesh.rs` (`fr008_slot_queue_critical_before_normal`, `fr008_hypervisor_nocache_critical_before_normal`); `crates/sharecli-ipc/src/queue.rs` (`critical_dequeues_before_normal_under_contention`); `crates/sharecli-core/src/lib.rs` (`SpawnRequest::queue_priority`, `Hypervisor::run`) | SlotQueue Critical-before-Normal priority + Hypervisor nocache wiring |
| AC-008.15 | `tests/fr008_queue_priority_operator.rs`; `crates/sharecli-ipc/src/queue.rs` (`resolve_operator_queue_priority`, `QUEUE_PRIORITY_ENV`); `crates/sharecli-core/src/lib.rs` (`SpawnRequest::from_operator`, `SpawnRequest::new`); `crates/harness-native/tests/native_harness_contract.rs` (`operator_queue_priority_honors_rules_conf`) | operator env + rules.conf → SpawnRequest queue priority |
| AC-008.16 | `crates/sharecli-core/src/lib.rs` (`Hypervisor::run_queued`, `hypervisor_run_queued_skips_coalesce_cache`); `crates/harness-native/src/strategies/queue.rs` (`queue_strategy_executes_via_hypervisor`, `priority_queue_strategy_executes_via_hypervisor`) | harness queue/priority_queue → Hypervisor nocache lane via `from_operator` |
| AC-008.17 | `crates/sharecli-core/src/lib.rs` (`Hypervisor::from_config`, `HypervisorConfig::coalesce_ttl`, `coalesce_ttl`); `crates/harness-native/src/strategies/coalesce.rs` (`coalesce_strategy_executes_via_hypervisor`, `coalesce_strategy_serves_cache_on_replay`, `cache_strategy_executes_via_hypervisor`); `crates/harness-native/src/strategies/hypervisor_lane.rs` (`rule_opts_plumb_hypervisor_config`, `config_from_rule_opts`) | harness coalesce/cache → Hypervisor::run via from_operator + RuleOpts ttl/debounce/max_concurrent |
| AC-008.18 | `crates/harness-native/src/strategies/debounce.rs` (`debounce_strategy_executes_via_hypervisor`, `debounce_strategy_serves_cache_on_replay`, `debounce_strategy_shares_in_window_store`); `crates/harness-native/src/strategies/hypervisor_lane.rs` (`config_from_rule_opts`, `build_hypervisor`) | harness debounce → Hypervisor::run via hypervisor_lane + debounce_ms → coalesce_debounce |
| AC-008.19 | `crates/sharecli-ipc/src/cache_key.rs` (`CacheKeyMode`, `command_key_with_mode`); `crates/sharecli-core/src/lib.rs` (`HypervisorConfig::cache_key_mode`); `tests/fr008_coalesce_mesh.rs` (`fr008_hypervisor_args_cache_key_mode_ignores_cwd`) | `cache_key_mode` `args` shares one coalesce cache across cwd/env (both dimensions ignored), while `time` mode keeps the legacy cwd/env-sensitive `command_key` |
| AC-008.20 | `crates/sharecli-ipc/src/semantic.rs` (`semantic_normalize_argv`); `crates/sharecli-core/src/lib.rs` (`HypervisorConfig::semantic`); `tests/fr008_coalesce_mesh.rs` (`fr008_hypervisor_semantic_coalesces_repeated_lint_dot`) | Semantic normalization runs before the Hypervisor cache hash: `.` maps to the project-root token and directory paths canonicalize before keying |

### FR-009 — FUSE

| AC        | Test file                        | Notes |
|-----------|----------------------------------|-------|
| AC-009.1..2 | `tests/fr009_fuse_intercept.rs` | construct + mount API; no privileged mount |
| AC-009.3 | `tests/fr009_fuse_intercept.rs`; `inode_map` unit tests | inode map / path resolution |
| AC-009.4 | `tests/fr009_fuse_intercept.rs`; `read_cache` unit tests | read coalesce hit/miss meters |
| AC-009.5 | `tests/fr009_fuse_intercept.rs`; `write_serialize` unit tests | path lock + CoW stage/commit/discard |
| AC-009.6 | `tests/fr009_fuse_intercept.rs`; `provenance` unit tests | write provenance xattrs on write_rel/commit_rel |
| AC-009.7 | `tests/fr009_fuse_intercept.rs`; `neg_dentry` unit tests | negative dentry TTL hit/miss + invalidate |
| AC-009.8 | `tests/fr009_fuse_intercept.rs` (`fr009_privileged_mount_smoke`); `mount_smoke` unit tests | opt-in live FUSE mount + provenance xattrs (`SHARECLI_FUSE_MOUNT_SMOKE=1`) |
| AC-009.9 | `tests/fr004_status_health.rs`; `tests/fr007_thermal_tui_watch.rs` (`fr009_thermal_tui_neg_dentry_lines`); `crates/sharecli-fuse/src/neg_dentry.rs` | FUSE neg dentry in status + thermal TUI |
| AC-009.10 | `tests/fr009_fuse_intercept.rs` (`fr009_global_write_serialize_meters`); `tests/fr004_status_health.rs`; `tests/fr007_thermal_tui_watch.rs`; `crates/sharecli-fuse/src/write_serialize_meters.rs` | FUSE write-serialize / CoW in status + thermal TUI |
| AC-009.11 | `tests/fr009_fuse_cli.rs`; `src/commands/fuse.rs` | `sharecli fuse provenance` reads backing write xattrs |
| AC-009.12 | `tests/fr009_fuse_hypervisor_session.rs`; `sharecli-core` `fuse_session_id_for_command_key`; `sharecli-fuse` `mount_with_session` | Hypervisor FUSE session from coalesce CommandKey |
| AC-009.13 | `tests/fr009_fuse_hypervisor_session.rs` (`fr009_hypervisor_spawn_outcome_fuse_session_id`); `sharecli-core` `SpawnOutcome::fuse_session_id` | SpawnOutcome exposes FUSE session when intercept active |
| AC-009.14 | `tests/fr009_fuse_hypervisor_session.rs` (`fr009_remap_mount_to_backing_subtree`, `fr009_hypervisor_spawn_outcome_fuse_path_remap`); `sharecli-fuse` `path_remap.rs`; `sharecli-core` `SpawnOutcome::remap_fuse_path`, `FuseGuard` teardown | FUSE mount/backing remap + spawn/teardown lifecycle |
| AC-009.15 | `crates/sharecli-fuse/src/lib.rs` (`create_rel`); `tests/fr009_fuse_intercept.rs` (`fr009_create_rel_provenance_and_neg_invalidate`) | FUSE `create_rel` stamps write provenance on the created path and invalidates its negative dentry and read-cache entries |
| AC-009.16 | `crates/sharecli-fuse/src/mount_smoke.rs` (`run_mount_smoke`); `tests/fr009_fuse_intercept.rs` (`fr009_privileged_mount_smoke`) | With `SHARECLI_FUSE_MOUNT_SMOKE=1`, the privileged mount smoke covers create / mkdir / unlink / rename round-trips through a live FUSE mount |
| AC-009.17 | `tests/fr009_fuse_cli.rs` (`fr009_cli_fuse_status_json`, `fr009_cli_fuse_list_empty`); `src/commands/fuse.rs` | `sharecli fuse` CLI operator surface (mount / unmount / status / list / commit / discard): `fuse status --json` exposes read-cache and write-serialize meters; `fuse list` with no mounts succeeds |
| AC-009.18 | `crates/sharecli-fuse/src/agents_conf.rs` (`AgentsConf::parse`, `sanitize_agent_id`) | `agents.conf` parsing ignores comments and blank lines, matches agent ids by substring, and sanitizes agent ids |
| AC-009.19 | `crates/sharecli-fuse/src/agent_cow.rs` (`AgentCowStore`, `commit_all_for_agent`, `discard_all_for_agent`) | Agent-scoped CoW: two agents stage the same path independently, committing one leaves the other staged, and `commit_all` / `discard_all` operate per agent |
| AC-009.20 | `crates/sharecli-fuse/src/agent_cow.rs` (`with_locked_path`, `ac_009_20_no_serialize_runs_callback`) | `--no-serialize` skips the lock-acquisition path and still runs the write callback (agent CoW) |
| AC-009.21 | `tests/fr009_fuse_cli.rs` (`fr009_cli_fuse_mount_rejects_invalid_agent`, `fr009_cli_fuse_mount_rejects_missing_agents_conf`, `fr009_cli_fuse_mount_help_documents_feb_flags`) | `sharecli fuse mount` loudly rejects invalid `--agent` values and a missing `--agents-conf` before mounting; help documents the Feb flags |
| AC-009.22 | `crates/fuse-smoke-runner/src/exec.rs` (`run_matrix`); `crates/fuse-smoke-runner/src/matrix.rs` (`default_cells_for_host`, `MatrixReport`) | `fuse-smoke` CLI runs the OS × arch privileged FUSE mount-smoke matrix with loud fail reasons |
| AC-009.23 | `crates/sharecli-fuse/src/platform/ops_missing.rs` (`readlink_rel`, `symlink_rel`, `link_rel`, `op_readlink`, `op_symlink`, `op_link`); `crates/sharecli-fuse/src/lib.rs` (`readlink`, `symlink`, `link` dispatch + session-wrapper forwarding) | Symlink/hard-link ops: `readlink` returns the stored target without following; `symlink` stores a relative target verbatim; `link` creates a second name for an existing inode; creation invalidates the negative dentry and installs the new entry |
| AC-009.24 | `crates/sharecli-fuse/src/platform/ops_missing.rs` (`fsync_handle`, `flush_fh`, `release_fh`, `op_flush`, `op_fsync`); `crates/sharecli-fuse/src/lib.rs` (`release`, `flush`, `fsync` dispatch + session-wrapper forwarding) | Durability/handle lifecycle: `fsync` syncs the live handle when held else the backing path for the inode; `flush` is advisory sync-data and always succeeds; `release` drops the handle and is a no-op on a second call |
| AC-009.25 | `crates/sharecli-fuse/src/winfsp_mount.rs` (`ensure_winfsp`, `start`); `crates/sharecli-fuse/src/lib.rs` (`mount`) | Windows WinFsp mount adapter for the FUSE intercept; other platforms return an unsupported-platform error |

### FR-010 — Mesh

| AC        | Test file                        | Notes |
|-----------|----------------------------------|-------|
| AC-010.1..3 | `tests/fr010_mesh_substrate.rs` | registry primitives |
| AC-010.4..6 | `tests/fr010_mesh_substrate.rs` | MaildirQueue lifecycle |
| AC-010.7 | `tests/fr010_mesh_substrate.rs`; `smart_merge` unit tests | git merge-file fallback |
| AC-010.8 | `tests/fr010_mesh_substrate.rs`; `worktree_pool` unit tests | allocate/release + NotGitRepo |
| AC-010.9 | `tests/fr010_mesh_substrate.rs`; `tests/fr010_mesh_cli.rs` | status counts + `mesh status` CLI |
| AC-010.10 | `tests/fr010_mesh_substrate.rs`; `tests/fr010_mesh_cli.rs` | reclaim_owner + `mesh reclaim` CLI |
| AC-010.11 | `tests/fr004_status_health.rs`; `tests/fr007_thermal_tui_watch.rs`; `sharecli-mesh` operator_status tests | Maildir depth in status + thermal TUI |

### FR-011 — Thermal Gate

| AC        | Test file                     | Notes |
|-----------|-------------------------------|-------|
| AC-011.1..3 | `tests/fr011_thermal_gate.rs` | also AC-008.3 |
| AC-011.4 | `tests/fr011_agent_thermal_gate.rs`; `sharecli-core` / `sharecli-fleet` agent_contention unit tests | agent-count thermal escalation |
| AC-011.5 | `tests/fr004_status_health.rs`; `sharecli-fleet` `format_gate_status_section` unit tests | status thermal+agent gate section |
| AC-011.6 | `tests/fr011_report_gate.rs`; `src/commands/report.rs` unit tests; `src/commands/mod.rs` (`ps --all`) | report + ps --all gate parity |
| AC-011.7 | `tests/fr011_pool_health_gate.rs`; `src/commands/mod.rs` (`pool`, `health`) | pool + health gate parity |

### FR-012 — Serve JWT AuthN

| AC        | Test file                        | Test function |
|-----------|----------------------------------|---------------|
| AC-012.1..3 | `tests/fr012_serve_jwt_auth.rs` | (see file) |

---

## Change log

- **2026-07-21 — FR-007 health/pool/status CSV operator companions (AC-007.82):** `sharecli health --csv`,
  `pool --csv`, and `status --csv` emit command-specific CSV bodies then companion
  `gate` → `host_watch` → `pool` → `status` records (parity with proc CSV AC-007.79 and report
  CSV AC-007.81); reject `--json` / `--watch`; stderr silent on success.
- **2026-07-21 — FR-007 report CSV pool/status companion rows (AC-007.81):** `sharecli report --format csv`
  emits fleet analytics CSV body then companion `gate` → `host_watch` → `pool` → `status` records
  (parity with proc CSV AC-007.79 and report text AC-007.74); rejects `--watch`; stderr silent on success.
- **2026-07-21 — FR-007 WS `health_update` expanded HealthSnapshot pool/status (AC-007.80):**
  `ClientMessage::from_json` decodes typed `health_update` frames with gate → host_watch → pool →
  status siblings (parity with IPC AC-007.78); live `health.status` IPC wrap roundtrip; legacy
  frames missing pool/status yield Unknown.
- **2026-07-21 — FR-007 proc CSV pool/status companion rows (AC-007.79):** `sharecli proc --csv` and
  `proc --tree --csv` append companion `pool` + `status` CSV records after `gate` → `host_watch`
  via `PoolOperatorPanel::format_csv_companion` / `StatusOperatorPanel::format_csv_companion`
  (parity with text AC-007.75 and JSON AC-007.77); stderr silent on success (extends AC-007.33).
- **2026-07-21 — FR-007 operator CLI JSON embedded pool/status siblings (AC-007.77):**
  `health --json`, `ps --all --json`, and `proc --json` embed gate → host_watch → pool → status;
  `pool --json` / `status --json` add nested sibling only; watch NDJSON parity; stderr silent on
  one-shot success.
- **2026-07-21 — FR-007 IPC health/pool/status embedded pool/status siblings (AC-007.78):**
  IPC `health.status` embeds top-level `pool` + `status`; `pool.status` nested `status` only;
  `status.snapshot` nested `pool` only; parity with CLI AC-007.77 and monitoring.report AC-007.72.
- **2026-07-21 — FR-007 health/pool/status/ps --all text pool/proc-scan operator lines (AC-007.76):**
  `sharecli health`, `pool`, `status`, and `ps --all` (text, one-shot + `--watch`) print
  `format_pool_operator_line` / `format_status_operator_line` on stdout after gate → `host_watch`
  (parity with AC-007.74/75 report/proc text paths); stderr silent on success.
- **2026-07-21 — FR-007 proc text pool/proc-scan operator lines (AC-007.75):** `sharecli proc` and
  `sharecli proc --tree` (text, one-shot + `--watch`) print `format_pool_operator_line` /
  `format_status_operator_line` on stdout after gate → `host_watch` (parity with AC-007.74 report
  text path); stderr silent on success.
- **2026-07-21 — FR-007 report text pool/proc-scan operator lines (AC-007.74):** `sharecli report`
  (text, one-shot + `--watch`) prints `format_pool_operator_line` / `format_status_operator_line`
  on stdout after gate → `host_watch` (parity with AC-007.73 JSON sibling order); stderr silent on
  success.
- **2026-07-21 — FR-007 report JSON embedded pool/status (AC-007.73):** `sharecli report --format json`
  / `FleetReportJson` embed top-level `pool` + `status` siblings after `gate` → `host_watch`
  (parity with `monitoring.report` AC-007.72 / dashboard WS AC-007.70); `report --watch --format json`
  NDJSON lines carry the same operator envelope per refresh.
- **2026-07-21 — FR-007 monitoring.report embedded pool/status (AC-007.72):** IPC
  `monitoring.report` / `MonitoringReportSnapshot` embed top-level `pool` + `status` siblings after
  `gate` → `host_watch` (parity with dashboard WS AC-007.70); Linux/Swift/Windows tray refresh
  consumes embedded fields in one round-trip and drops supplementary `pool.status` /
  `status.snapshot` polls from AC-007.69.
- **2026-07-21 — FR-007 thermal TUI pool/status operator panels:** `sharecli thermal` surfaces
  dedicated runtime pool + proc-scan status panels (AC-007.71) via `run_with_pool_status` polling
  `build_pool_json` / `build_status_json`; shared [`PoolOperatorPanel`](crates/sharecli-fleet/src/operator_pool_status.rs)
  / [`StatusOperatorPanel`](crates/sharecli-fleet/src/operator_pool_status.rs) formatters match tray
  AC-007.69 lines; keyboard focus adds pool (`2`) and status (`3`) panels.
- **2026-07-21 — FR-007 dashboard WS pool/status envelope parity:** `sharecli serve` `/ws`
  snapshots emit top-level `pool` + `status` siblings (AC-007.70) in order
  gate → host_watch → pool → status → agents → processes; embedded dashboard operator panels
  render pool capacity + proc-scan fields; extends AC-007.41 dashboard envelope + AC-007.69
  tray format helpers.
- **2026-07-21 — FR-007 tray pool/status consume parity:** Linux/Swift/Windows tray refresh loops
  call supplementary `pool.status` + `status.snapshot` IPC round-trips alongside primary
  `monitoring.report` refresh (AC-007.69); operator panels surface dedicated pool + proc-scan
  lines via shared format helpers; extends AC-007.68 wire + AC-007.48 monitoring.report consume.
- **2026-07-21 — FR-007 tray pool/status IPC wire parity:** Linux/Windows/Swift/C# tray clients
  decode `PoolSnapshot` / `StatusSnapshot` from `pool.status` / `status.snapshot` (AC-007.68);
  parity with AC-007.47 monitoring.report wiring.
- **2026-07-21 — FR-007 IPC pool.status + status.snapshot gate/host_watch parity:** IPC
  `pool.status` / `PoolSnapshot` and `status.snapshot` / `StatusSnapshot` emit top-level `gate` +
  `host_watch` siblings after pool/status fields (AC-007.67); parity with `pool --json` AC-007.44
  and `status --json` AC-007.25; completes IPC operator snapshot surface alongside
  `health.status` AC-007.45 and `monitoring.report` AC-007.46.
- **2026-07-21 — FR-007 status --watch gate/host_watch parity:** `sharecli status --watch`
  (text, no `--json`) MUST NOT print gate/host_watch text companions or `[watch]` footer on
  stderr during refresh cycles (AC-007.66); gate → `host_watch` and footer stay on stdout only;
  parity with pool text watch AC-007.65. `sharecli status --watch --json` streams NDJSON with
  gate → host_watch per refresh; stderr text companions (parity with pool watch AC-007.65).
- **2026-07-21 — FR-007 pool --watch gate/host_watch parity:** `sharecli pool --watch`
  (text, no `--json`) MUST NOT print gate/host_watch text companions or `[watch]` footer on
  stderr during refresh cycles (AC-007.65); gate → `host_watch` and footer stay on stdout only;
  parity with health text watch AC-007.64. `sharecli pool --watch --json` streams NDJSON with
  gate → host_watch per refresh; stderr text companions (parity with health watch AC-007.64).
- **2026-07-21 — FR-007 health --watch gate/host_watch parity:** `sharecli health --watch`
  (text, no `--json`) MUST NOT print gate/host_watch text companions or `[watch]` footer on
  stderr during refresh cycles (AC-007.64); gate → `host_watch` and footer stay on stdout only;
  parity with ps text watch AC-007.50. `sharecli health --watch --json` streams NDJSON with
  gate → host_watch per refresh; stderr text companions (parity with report watch AC-007.42 /
  ps watch AC-007.49).
- **2026-07-21 — FR-007 tray thermal gate visual indicator parity:** Linux tray `icon_name` +
  SNI `NeedsAttention`, Swift menu bar icon tint + popover thermal badge, and WinUI
  `ThermalBadgeText` + colored gate row derive from `gate.thermal_pressure` +
  `gate_decision` via shared `resolve_tray_gate_visual` helpers (AC-007.57); dashboard
  `gate-admit` / `gate-deny` / `gate-unavailable` + `#thermal-status` CSS token parity;
  golden severity matrix in `tests/fr007_tray_thermal_visual.rs`; extends AC-007.56 /
  AC-007.48 monitoring.report refresh path.
- **2026-07-21 — FR-007 tray gate/host_watch UI parity:** Linux tray menu + tooltip, Swift
  `TrayPopoverView` + `HealthView`, and WinUI `TrayWindow` gate/host rows surface thermal gate +
  host_watch operator metadata from each `monitoring.report` refresh (AC-007.56); reusable
  `operator_display` format helpers in Rust/Swift/C# with golden-string tests; extends AC-007.48 /
  AC-007.51 consume path.
- **2026-07-21 — FR-007 Windows tray process grid harness parity:** WinUI `MonitoringReportSnapshot.AsProcessSummaries`
  maps `harness` into `ProcessInfo`; `TrayWindow` DataGrid binds Harness column (AC-007.55); parity
  with Linux tray submenu Harness label + Swift `DashboardView` Harness column; extends AC-007.51
  process row mapping.
- **2026-07-21 — FR-007 Windows tray kill operator parity:** WinUI `TrayWindow` wires per-process
  Kill + Kill All Managed to IPC `process.kill` / `process.kill_all` (AC-007.54); `IpcKill.cs`
  helpers + post-kill `RefreshDataAsync` / `monitoring.report` refresh; parity with Linux tray
  submenu kill + Swift `AppState.kill`/`killAll`; extends AC-007.51 consume path.
- **2026-07-21 — FR-007 Linux/Swift tray poll constant parity:** Linux tray loop and Swift
  `AppState.startPolling` wire shared `TRAY_POLL_INTERVAL_SECS` / `TrayPoll.intervalSeconds`
  on ~3 s cadence (AC-007.53); same `monitoring.report` refresh path as AC-007.48; parity with
  Windows AC-007.52.
- **2026-07-21 — FR-007 Windows tray periodic poll parity:** WinUI `TrayWindow` wires
  `DispatcherQueueTimer` on ~3 s cadence calling `RefreshDataAsync` / `monitoring.report`
  (AC-007.52); `TRAY_POLL_INTERVAL_SECS` + C# `TrayPoll.IntervalSeconds`; parity with Linux
  tray + Swift `AppState.startPolling`; extends AC-007.51 consume path.
- **2026-07-21 — FR-007 Windows tray refresh consumes monitoring.report snapshot:** WinUI
  `TrayWindow.RefreshDataAsync` drives operator gate/host_watch + process inventory from one
  `monitoring.report` IPC poll (AC-007.51); Rust + C# mapping helpers on
  `MonitoringReportSnapshot`; no split `health.status` + `process.list` operator refresh;
  parity with Linux/Swift AC-007.48.
- **2026-07-21 — FR-007 ps --all watch text stderr silence:** `sharecli ps --all --watch` (text,
  no `--json`) MUST NOT print gate/host_watch text companions or `[watch]` footer on stderr
  during refresh cycles (AC-007.50); gate → `host_watch` and footer stay on stdout only; parity
  with proc text watch AC-007.35; inverse of NDJSON stderr companions AC-007.49.
- **2026-07-21 — FR-007 ps --all watch NDJSON gate/host_watch parity:** `sharecli ps --all --watch --json`
  streams NDJSON with `ts` + managed pool + agent inventory + gate → `host_watch` per refresh
  (AC-007.49); stderr text companions + `[watch]` footer; stdout pipe-clean; parity with
  `report --watch --format json` AC-007.42 and one-shot `ps --all --json` AC-007.43.
- **2026-07-21 — FR-007 tray refresh consumes monitoring.report snapshot:** Swift `AppState.refresh`
  and Linux tray `refresh` drive operator gate/host_watch + process inventory from one
  `monitoring.report` IPC poll (AC-007.48); mapping helpers on `MonitoringReportSnapshot`; no split
  `health.status` + `process.list` operator refresh.
- **2026-07-21 — FR-007 tray/desktop monitoring.report wire parity:** `sharecli-tray-linux` and Swift
  `IPCClient` decode `MonitoringReportSnapshot` from IPC `monitoring.report` via
  `monitoring_report()` / `monitoringReport()` (AC-007.47); wire unit tests; parity with
  AC-007.45 HealthSnapshot tray wiring.
- **2026-07-21 — FR-007 IPC monitoring.report gate/host_watch parity:** IPC `monitoring.report` /
  `MonitoringReportSnapshot` emit top-level `gate` + `host_watch` JSON siblings after fleet
  monitoring fields (AC-007.46); gate → host_watch key order; parity with `report --format json`
  AC-007.40 and `health.status` AC-007.45.
- **2026-07-21 — FR-007 IPC health.status gate/host_watch parity:** IPC `health.status` /
  `HealthSnapshot` emit top-level `gate` + `host_watch` JSON siblings after runtime health
  fields (AC-007.45); gate → host_watch key order; tray/desktop wire parity with
  `health --json` AC-007.44 (`sharecli-tray-linux`, Swift `IPCClient`).
- **2026-07-21 — FR-007 health/pool JSON gate/host_watch parity:** `sharecli health --json` and
  `sharecli pool --json` emit top-level `gate` + `host_watch` JSON siblings after runtime health /
  pool status fields (AC-007.44); gate → host_watch key order; stderr silent on success (parity
  with AC-007.25 status --json and AC-007.43 ps --all --json).
- **2026-07-21 — FR-007 ps --all JSON gate/host_watch parity:** `sharecli ps --all --json`
  emits top-level `gate` + `host_watch` JSON siblings after managed pool + host agent inventory
  fields (AC-007.43); gate → host_watch key order; stderr silent on success (parity with
  AC-007.25 status --json and AC-007.38 ps --all text); `ps --json` without `--all` fails loudly.
- **2026-07-21 — FR-007 report watch NDJSON gate/host_watch parity:** `sharecli report --watch --format json`
  streams NDJSON with `ts` + fleet analytics + gate → `host_watch` per refresh (AC-007.42);
  stderr text companions + `[watch]` footer; stdout pipe-clean (parity with proc watch AC-007.28).
- **2026-07-21 — FR-007 report JSON gate/host_watch parity:** `sharecli report --format json`
  emits top-level `gate` + `host_watch` JSON siblings after fleet analytics fields
  (AC-007.40); gate → host_watch key order; stderr silent on success (parity with AC-007.25
  status --json and AC-007.24 proc JSON).
- **2026-07-20 — FR-007 report text host watch parity:** `sharecli report` (text, one-shot and
  `--watch`) prints gate → `host_watch` text sections on stdout after report body (AC-007.39);
  stderr silent on success (parity with AC-007.38 ps --all and AC-011.6 gate).
- **2026-07-20 — FR-007 ps --all one-shot text host watch parity:** `sharecli ps --all` prints
  gate → `host_watch` text sections on stdout after host agent inventory (AC-007.38); stderr
  silent on success (parity with AC-007.37 health/pool and AC-011.6 gate).
- **2026-07-20 — FR-007 health/pool one-shot text host watch parity:** `sharecli health` and
  `sharecli pool` print gate → `host_watch` text sections on stdout after pool/runtime health
  output (AC-007.37); stderr silent on success (parity with AC-007.36 status / AC-007.34 proc).
- **2026-07-20 — FR-007 status one-shot text stderr silence:** `sharecli status` (no `--json`)
  MUST NOT print gate/host_watch text companions on stderr; gate and `host_watch` stay in text
  sections on stdout only (AC-007.36); extends AC-007.27 text gate ordering with pipe-clean
  stderr (parity with AC-007.30 / AC-007.31 / AC-007.32 / AC-007.34).
- **2026-07-20 — FR-007 proc watch text stderr silence:** `sharecli proc --watch` and
  `proc --tree --watch` (no `--json`) MUST NOT print gate/host_watch text companions or
  `[watch]` footer on stderr during refresh cycles; gate, `host_watch`, and `[watch]` stay on
  stdout only (AC-007.35); inverse contract of AC-007.28 / AC-007.29 NDJSON stderr companions;
  extends AC-007.34 to watch text refresh.
- **2026-07-20 — FR-007 proc one-shot text stderr silence:** `sharecli proc`, `proc --tree`, and
  `proc --pid N` (no `--watch`) MUST NOT print gate/host_watch text companions on stderr; gate
  and `host_watch` stay in text sections on stdout only (AC-007.34); extends AC-007.17 /
  AC-007.20 / AC-007.21 text gate ordering with pipe-clean stderr (parity with AC-007.30 /
  AC-007.31 / AC-007.32 / AC-007.33).
- **2026-07-20 — FR-007 proc one-shot CSV stderr silence:** `sharecli proc --csv` and
  `proc --tree --csv` (no `--watch`) MUST NOT print gate/host_watch text companions on stderr;
  gate and `host_watch` stay in CSV companion rows on stdout only (AC-007.33); extends AC-007.19
  CSV companion contract with pipe-clean stderr (parity with AC-007.30 / AC-007.31 / AC-007.32).
- **2026-07-20 — FR-007 status one-shot JSON stderr silence:** `sharecli status --json` MUST NOT
  print gate/host_watch text companions on stderr; gate and `host_watch` stay in the JSON body
  only (AC-007.32); extends AC-007.25 JSON shape with pipe-clean stderr (parity with AC-007.30 /
  AC-007.31).
- **2026-07-20 — FR-007 proc pid one-shot JSON stderr silence:** `sharecli proc --pid N --json`
  (no `--watch`) MUST NOT print gate/host_watch text companions on stderr; gate and
  `host_watch` stay in the JSON body only (AC-007.31); extends AC-007.30 to pid detail.
- **2026-07-20 — FR-007 proc one-shot JSON stderr silence:** `sharecli proc --json` and
  `proc --tree --json` (no `--watch`) MUST NOT print gate/host_watch text companions on
  stderr; gate and `host_watch` stay in the JSON body only (AC-007.30); inverse contract of
  watch NDJSON stderr companions (AC-007.28 / AC-007.29).
- **2026-07-20 — FR-007 proc tree watch NDJSON stderr footer lock:** `sharecli proc --tree --watch --json`
  prints gate → host_watch text companions on stderr each refresh via the same helper as flat
  watch (AC-007.29); stdout stays pipe-clean NDJSON only; extends AC-007.28 to tree watch.
- **2026-07-20 — FR-007 proc watch NDJSON stderr footer lock:** `sharecli proc --watch --json`
  prints gate → host_watch text companions on stderr each refresh; stdout stays pipe-clean
  NDJSON only (AC-007.28); extends AC-006.18 stderr footer contract with gate/watch ordering.
- **2026-07-20 — FR-007 status text gate ordering:** `sharecli status` text prints
  thermal gate section before host resource watch (AC-007.27); parity with proc text
  (AC-007.21) and status --json gate → host_watch (AC-007.25).
- **2026-07-20 — FR-007 status JSON host watch:** `sharecli status --json` emits
  top-level `gate` + `host_watch` siblings with flat `agents` array (AC-007.25);
  proc-adjacent operator surface parity with `sharecli proc --json`.
- **2026-07-20 — FR-008 command_key cwd/env dimensions:** `command_key` and
  Hypervisor coalesce cache isolate `cwd` + `env_subset`, not argv alone
  (AC-008.13); closes AC-008.1 cwd/env coverage gap in coalesce mesh tests.
- **2026-07-20 — FR-007 proc one-shot JSON gate ordering:** `sharecli proc --json`,
  `proc --tree --json`, and `proc --pid --json` serialize `"gate"` before `"host_watch"`
  in raw JSON (AC-007.24); completes proc gate/watch ordering matrix; flat watch dwell
  hardened 12s→30s (AC-007.22).
- **2026-07-20 — FR-007 proc tree watch gate ordering:** `sharecli proc --tree --watch` text and
  NDJSON preserve gate → `host_watch` ordering on every refresh (AC-007.23); locks tree watch
  parity with flat watch AC-007.22 and one-shot tree text AC-007.20.
- **2026-07-20 — FR-007 proc watch gate ordering:** `sharecli proc --watch` text and
  NDJSON preserve gate → `host_watch` ordering on every refresh (AC-007.22); locks watch
  parity with one-shot proc gate sections.
- **2026-07-20 — FR-007 proc flat text gate ordering:** `sharecli proc` flat text prints
  thermal gate section before host watch footer (AC-007.21); completes proc text gate
  coverage across flat/tree/pid surfaces.
- **2026-07-20 — FR-007 proc tree text gate ordering:** `sharecli proc --tree` text
  prints thermal gate section before host watch footer (AC-007.20); completes proc text
  gate coverage across flat/tree/pid surfaces.
- **2026-07-20 — FR-007 proc CSV gate companion:** `sharecli proc --csv` and
  `--tree --csv` append companion `gate` CSV record before `host` via
  `GateStatusSnapshot::format_csv_companion` (AC-007.19); completes proc CSV gate
  coverage across flat/tree surfaces.
- **2026-07-20 — FR-007 proc tree gate parity:** `sharecli proc --tree --json` and
  `--tree --watch --json` emit `gate` on every snapshot via
  [`AgentTreeSnapshot`](src/commands/proc/mod.rs) (AC-007.18); completes proc JSON gate
  coverage across flat/tree/pid surfaces.
- **2026-07-20 — FR-007 proc pid gate parity:** `sharecli proc --pid N --json` emits
  `gate` on [`ProcDetailSnapshot`](src/commands/proc/mod.rs) and text detail prints thermal
  gate section before host watch footer (AC-007.17); completes proc detail gate coverage.
- **2026-07-20 — FR-007 proc pid JSON host watch:** `sharecli proc --pid N --json` emits
  `host_watch` on [`ProcDetailSnapshot`](src/commands/proc/mod.rs) and text detail appends host
  watch footer (AC-007.16); completes proc JSON `host_watch` coverage.
- **2026-07-20 — FR-007 proc tree JSON host watch:** `sharecli proc --tree --json` and
  `--tree --watch --json` emit `host_watch` on every snapshot via
  `AgentTreeSnapshot` (AC-007.15); parity with flat JSON from AC-007.13.
- **2026-07-20 — FR-007 proc text/CSV host watch:** `sharecli proc` text footer and
  `--csv` companion `host` record surface live FD/RSS/load/net via
  `HostResourceWatchJson` (AC-007.14); flat + `--tree` parity with JSON `host_watch`.
- **2026-07-20 — FR-006 thermal TUI flat agent state:** flat Detected Agents lines
  (full, compact, empty-forest fallback) show process state letters via
  `build_host_agent_state_map` / `state_text_for_pid` (AC-006.40).
- **2026-07-20 — FR-006 thermal TUI agent-tree state:** full-layout Detected Agents
  tree nodes show process state letters via `build_host_forest_state_map` (AC-006.39);
  forest helpers lifted to `sharecli-fleet` for proc/TUI parity.
- **2026-07-20 — FR-009 FUSE path remap + lifecycle:** SpawnOutcome exposes
  `fuse_backing` / `fuse_mountpoint`; `remap_mount_to_backing` + guard teardown
  on drop (AC-009.14).
- **2026-07-20 — FR-007 thermal TUI gate parity:** `sharecli thermal` gate panel derives
  ADMIT/DENY from `gate_status_snapshot_with_rss` with live agent RSS + contention
  (AC-007.26); replaces count-only `effective_gate_decision` in thermal TUI.
- **2026-07-20 — FR-009 SpawnOutcome FUSE session:** cache-miss outcomes expose
  `fuse_session_id` when intercept mount is active (AC-009.13).
- **2026-07-20 — FR-009 Hypervisor FUSE session:** cache-miss mounts pass
  coalesce-derived session id to `mount_with_session` (AC-009.12).
- **2026-07-20 — FR-009 fuse provenance CLI:** `sharecli fuse provenance`
  reads backing write xattrs via `read_provenance` (AC-009.11).
- **2026-07-20 — FR-008 harness debounce Hypervisor:** harness-native `debounce`
  strategy routes through `Hypervisor::run` with `debounce_ms` →
  `HypervisorConfig::coalesce_debounce` via `hypervisor_lane` (AC-008.18).
- **2026-07-20 — FR-008 harness coalesce Hypervisor:** harness-native `coalesce` /
  `cache` strategies route through `Hypervisor::run` with `SpawnRequest::from_operator`
  and `{harness_home}/var/sharecli-hypervisor` cache root; `RuleOpts` ttl/debounce/
  max_concurrent map into `HypervisorConfig` (AC-008.17).
- **2026-07-20 — FR-008 harness queue Hypervisor:** harness-native `queue` /
  `priority_queue` strategies route through `Hypervisor::run_queued` with
  `SpawnRequest::from_operator` (AC-008.16); replaces raw `Command::spawn`.
- **2026-07-20 — FR-008 operator queue priority:** `SHARECLI_QUEUE_PRIORITY` +
  rules.conf `priority=` resolve through `resolve_operator_queue_priority` into
  `SpawnRequest::queue_priority` (AC-008.15); harness-native re-exports resolver.
- **2026-07-20 — FR-008 SlotQueue operator meters:** `global_slot_queue_meters`
  + status/thermal TUI panels (AC-008.12); acquire/wait/timeout on nocache lane.
- **2026-07-20 — FR-009 write-serialize operator meters:** `global_write_serialize_meters`
  + status/thermal TUI panels (AC-009.10); mirrors read-coalesce / neg-dentry surfacing.
- **2026-07-20 — FR-008 coalesce operator meters:** `global_coalesce_meters` +
  status/thermal TUI panels (AC-008.11); mirrors FUSE read-coalesce surfacing.
- **2026-07-20 — FR-011 pool/health gate parity:** `sharecli pool` and
  `sharecli health` print `format_gate_status_section` after runtime health
  (AC-011.7).
- **2026-07-20 — FR-011 report/ps gate parity:** `sharecli report` JSON/text and
  `sharecli ps --all` surface gate fields via `gate_status_snapshot` (AC-011.6).
- **2026-07-20 — FR-011 status gate section:** `sharecli status` prints
  `format_gate_status_section` with live thermal + agent inventory (AC-011.5).
- **2026-07-20 — FR-011 agent-aware thermal gate:** `AgentAwareThermalGate` wraps
  production Hypervisor gate; proc-scan agent count escalates spawn decisions
  (AC-011.4); thermal TUI gate panel uses `gate_status_snapshot_with_rss` (AC-007.26).
- **2026-07-20 — FR-006 proc sort state:** `sharecli proc --sort state` orders flat
  inventory and tree root forests by process state letter with PID tie-break
  (AC-006.36).
- **2026-07-20 — FR-006 proc watch NDJSON state parity:** `proc --watch --json` NDJSON
  agent rows include `state` letter matching flat `--json` export (AC-006.37).
- **2026-07-20 — FR-006 proc exclude-family filter:** `sharecli proc --exclude-family <id>`
  drops matching agent families from flat inventory and `--tree` roots (negation of
  `--family`); mutually exclusive with `--family` (AC-006.38).
- **2026-07-20 — FR-006 proc tree forest state lookup:** `--tree` text/JSON/CSV resolve
  live process state for every forest PID (roots + children) via
  `build_forest_state_map` (AC-006.35).
- **2026-07-20 — FR-006 proc tree state surfaces:** `--tree` text nodes and
  `--tree --json` `AgentTreeNodeJson` rows include `state` letter (AC-006.34).
- **2026-07-20 — FR-006 proc state text surfaces:** flat text inventory and
  `proc --pid` detail expose process state letter (AC-006.33).
- **2026-07-20 — FR-006 proc state export:** flat `--json` agent rows and `--csv`
  / `--tree --csv` columns include `state` letter (AC-006.32).
- **2026-07-20 — FR-006 proc state filter:** `sharecli proc --state <letter>`
  filters flat inventory and `--tree` roots by Linux/sysinfo process state,
  composed with other proc filters (AC-006.31).
- **2026-07-20 — FR-006 proc cmdline filter:** `sharecli proc --cmdline <pattern>`
  case-insensitive joined argv/cmdline substring filter composes with other proc
  flags (AC-006.30).
- **2026-07-20 — FR-006 proc comm filter:** `sharecli proc --comm <pattern>`
  case-insensitive COMM substring filter composes with other proc flags (AC-006.29).
- **2026-07-20 — FR-006 proc min/max-fd:** `sharecli proc --min-fd N` and
  `--max-fd N` FD band filters compose with other proc flags (AC-006.28).
- **2026-07-20 — FR-006 proc max-rss:** `sharecli proc --max-rss <size>` upper RSS
  bound filter composes with `--min-rss` and other proc flags (AC-006.27).
- **2026-07-20 — FR-006 thermal TUI agent tree:** full-layout Detected Agents panel
  renders `build_host_agent_forests` subtrees with live RSS (AC-006.22).
- **2026-07-20 — FR-006 proc pid detail:** `sharecli proc --pid N` one-shot
  RSS/FD/cmdline/parent detail view with `--json` (AC-006.23).
- **2026-07-20 — FR-006 proc limit:** `sharecli proc --limit N` caps flat inventory
  rows and tree root forests after filters/sort (AC-006.21).
- **2026-07-20 — FR-006 extended fingerprints:** `amp` family plus codex/aider/cursor-agent
  wrapper argv markers and false-positive guards (AC-006.20).
- **2026-07-20 — FR-006 proc sort:** `sharecli proc --sort rss|fd|pid|state` orders
  inventory rows and tree root forests after filters (AC-006.19, AC-006.36).
- **2026-07-20 — FR-006 proc filters:** `sharecli proc --family` and `--min-rss`
  narrow inventory rows and tree root forests (AC-006.17); `parse_rss_bytes`
  accepts plain bytes or K/M/G suffixes.
- **2026-07-20 — FR-006 proc tree mode:** `sharecli proc --tree` parent-child agent
  forests via `build_agent_forests` (AC-006.16); `--tree --json` nested `forests` payload.
- **2026-07-20 — FR-006 proc watch mode:** `sharecli proc --watch N` live refresh
  for host agent inventory (AC-006.15); text + `--json` parity with one-shot proc.
- **2026-07-20 — FR-006 proc CLI + RSS gate:** `sharecli proc` / `--json`, status
  `--json` agent inventory (AC-006.11–006.13); cmdline fingerprints for ambiguous
  comms (AC-006.14); aggregate agent RSS escalates spawn gate (AC-006.12).
- **2026-07-20 — FR-006/007 per-agent PID watch:** `AgentResourceSample` +
  `watch_host_agents` attach RSS/FD samples to proc-scan agents (AC-006.10);
  thermal DetectedAgent panel + `ps --all` show per-agent RSS; `SpawnOutcome::agent_family`.
- **2026-07-20 — FR-006 thermal TUI agent panel:** `scan_host_agents` inventory
  in `sharecli thermal` DetectedAgent panel (AC-006.9); complements ps AGENT
  column from #416.
- **2026-07-20 — FR-009 negative dentry operator meters:** `global_neg_dentry_meters`
  + status/thermal TUI panels (AC-009.9); mirrors read-coalesce surfacing from #414.
- **2026-07-20 — FR-007 thermal TUI dashboard slice:** `sharecli thermal`
  polls ResourceWatchSample + FUSE read-coalesce meters each redraw
  (AC-007.9 TUI + AC-007.11); formalized AC-007.7..9 in FR.md.
- **2026-07-20 — FR-008 Hypervisor nocache e2e:** AC-008.10 side-effect
  re-exec + concurrent SlotQueue serialize + coalesce isolation
  (`tests/e2e_hypervisor_nocache.rs`).
- **2026-07-20 — FR-009 negative dentry cache:** `NegativeDentryCache` +
  `exists_rel` / FUSE lookup ENOENT TTL (AC-009.7).
- **2026-07-20 — FR-010 mesh CLI:** `MaildirQueue::status` + `reclaim_owner`
  operator surface; `sharecli mesh status|reclaim` (AC-010.9..10);
  TRACEABILITY AC-008.7..9 marked covered (was stale TBD).
- **2026-07-20 — FR-009 mount smoke provenance:** `run_mount_smoke` verifies
  write provenance xattrs on backing after live FUSE write (AC-009.6 × AC-009.8).
- **2026-07-20 — FR-009 mount smoke:** opt-in privileged FUSE read/write via
  `SHARECLI_FUSE_MOUNT_SMOKE=1` (`run_mount_smoke`, AC-009.8).
- **2026-07-20 — FR-009 write provenance:** `user.sharecli.session` /
  `user.sharecli.written_at` stamped on `write_rel` / `commit_rel` (AC-009.6).
- **2026-07-19 — FR-009/010 A+ closeout:** CoW `stage_bytes`/`commit_pending`/
  `discard_pending` (AC-009.5); `SmartMerger` + `WorktreePool` (AC-010.7..8).
- **2026-07-19 — FR-009 A+ recovery:** InterceptFs passthrough + inode map +
  read coalesce meters + write-serialize scaffold (AC-009.3..5).
- **2026-07-19 — thesis re-enum:** Published FR-006..FR-011 from origin export
  (paste + agent-mesh WBS + harness). Traceability matrix expanded to 12 FRs.
- **2026-07-12 — T-220:** Landed `tests/fr004_status_health.rs` + `tests/fr004_pool_status.rs` (AC-004.1..004.5).
- **2026-07-12 — T-210:** Landed `tests/fr003_project_registry.rs` + `tests/fr003_project_discover.rs` (AC-003.1..003.5). Also T-260 claim-lock + T-270 loop budgets.
- **2026-07-12 — T-200:** Landed `tests/fr002_config_init.rs` + `tests/fr002_config_load.rs` (AC-002.1..002.5). FR-002 Status remains ACCEPTED with files on disk. Governance: `docs/ops/governance/{WBS-PHASED,GAP-QA-MATRIX}.md`.
- **2026-06-15 — Phase 3 initial:** 5 FRs published; 10 acceptance test files added; full FR→source→test matrix established.
