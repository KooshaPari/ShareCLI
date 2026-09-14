//! `sharecli-thermal-tui` — live TUI for thermal-gate / hypervisor state.
//!
//! # Design
//!
//! All display transforms (pressure → style/label, count → gauge, decision →
//! indicator) are **pure functions** so they can be unit-tested without a
//! terminal.  The `App` struct holds only data-model state; the `render`
//! function (also pure, takes `&mut Frame`) performs the layout.
//!
//! The event loop in [`run`] polls the [`ThermalGovernor`] on a configurable
//! interval and redraws until the user presses `q` or `Ctrl-C`.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};
use sharecli_fleet::proc_scan::DetectedAgent;
use sharecli_fleet::thermal::{ThermalGovernor, ThermalLevel};
use sharecli_fleet::{
    build_host_agent_forests, build_host_agent_state_map, build_host_forest_state_map,
    format_pool_operator_line, format_rss_bytes, format_status_operator_line,
    gate_status_snapshot_with_rss, global_coalesce_meters, global_slot_queue_meters,
    state_text_for_pid, sum_detected_agent_rss_bytes, watch_host_agents, AgentTreeNode,
    CoalesceMeters, DetectedAgentWatch, GateStatusSnapshot, PoolOperatorPanel, ResourceWatchSample,
    SlotQueueMeters, StatusOperatorPanel,
};
use sharecli_fuse::{
    global_neg_dentry_meters, global_read_cache_meters, global_write_serialize_meters,
    NegDentryMeters, ReadCacheMeters, WriteSerializeMeters,
};
use sharecli_mesh::{capture_maildir_status, MaildirStatus};

// ---------------------------------------------------------------------------
// Pure transforms — unit-testable
// ---------------------------------------------------------------------------

/// Returns true when the key event should exit the TUI (`q` or `Ctrl-C`).
pub fn is_quit_key(key: &KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') => true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => true,
        _ => false,
    }
}

/// Keyboard-focusable operator panels (C09 L81.3): gate → pool → status → host watch → agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PanelFocus {
    #[default]
    Gate,
    Pool,
    Status,
    HostWatch,
    Agents,
}

impl PanelFocus {
    const ORDER: [PanelFocus; 5] = [
        PanelFocus::Gate,
        PanelFocus::Pool,
        PanelFocus::Status,
        PanelFocus::HostWatch,
        PanelFocus::Agents,
    ];

    /// Advance focus: gate → pool → status → host watch → agents → gate.
    pub fn next(self) -> Self {
        let idx = Self::ORDER.iter().position(|&p| p == self).unwrap_or(0);
        Self::ORDER[(idx + 1) % Self::ORDER.len()]
    }

    /// Retreat focus: agents → host watch → status → pool → gate → agents.
    pub fn prev(self) -> Self {
        let idx = Self::ORDER.iter().position(|&p| p == self).unwrap_or(0);
        Self::ORDER[(idx + Self::ORDER.len() - 1) % Self::ORDER.len()]
    }

    /// Map digit keys `1`–`5` to panels.
    pub fn from_digit(ch: char) -> Option<Self> {
        match ch {
            '1' => Some(PanelFocus::Gate),
            '2' => Some(PanelFocus::Pool),
            '3' => Some(PanelFocus::Status),
            '4' => Some(PanelFocus::HostWatch),
            '5' => Some(PanelFocus::Agents),
            _ => None,
        }
    }
}

/// Semantic action from a keyboard event in the thermal TUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyAction {
    Quit,
    FocusNext,
    FocusPrev,
    FocusPanel(PanelFocus),
    ForcePoll,
    ToggleHelp,
    Noop,
}

/// Pure keybinding matrix for the thermal TUI (C09 L81.3).
pub fn handle_key(key: &KeyEvent) -> KeyAction {
    if is_quit_key(key) {
        return KeyAction::Quit;
    }
    match key.code {
        KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => KeyAction::FocusPrev,
        KeyCode::Tab => KeyAction::FocusNext,
        KeyCode::Char('1') => KeyAction::FocusPanel(PanelFocus::Gate),
        KeyCode::Char('2') => KeyAction::FocusPanel(PanelFocus::Pool),
        KeyCode::Char('3') => KeyAction::FocusPanel(PanelFocus::Status),
        KeyCode::Char('4') => KeyAction::FocusPanel(PanelFocus::HostWatch),
        KeyCode::Char('5') => KeyAction::FocusPanel(PanelFocus::Agents),
        KeyCode::Char('r') => KeyAction::ForcePoll,
        KeyCode::Char('?') => KeyAction::ToggleHelp,
        _ => KeyAction::Noop,
    }
}

/// Apply a non-quit [`KeyAction`] to live application state.
pub fn apply_key_action(app: &mut App, action: KeyAction) {
    match action {
        KeyAction::FocusNext => app.focus = app.focus.next(),
        KeyAction::FocusPrev => app.focus = app.focus.prev(),
        KeyAction::FocusPanel(panel) => app.focus = panel,
        KeyAction::ToggleHelp => app.show_help_overlay = !app.show_help_overlay,
        KeyAction::Quit | KeyAction::ForcePoll | KeyAction::Noop => {}
    }
}

/// Footer help overlay copy (shown when `?` toggles help).
pub const HELP_OVERLAY_HINT: &str =
    " Tab/Shift-Tab cycle  1 gate  2 pool  3 status  4 watch  5 agents  r poll  ? hide";

/// Map a [`ThermalLevel`] to a human-readable label.
pub fn level_label(level: ThermalLevel) -> &'static str {
    match level {
        ThermalLevel::Green => "GREEN",
        ThermalLevel::Yellow => "YELLOW",
        ThermalLevel::Red => "RED",
    }
}

/// Map a [`ThermalLevel`] to a foreground [`Color`].
pub fn level_color(level: ThermalLevel) -> Color {
    match level {
        ThermalLevel::Green => Color::Green,
        ThermalLevel::Yellow => Color::Yellow,
        ThermalLevel::Red => Color::Red,
    }
}

/// Map a [`ThermalLevel`] to the integer pressure value returned by sysctl.
pub fn level_pressure_raw(level: ThermalLevel) -> u8 {
    match level {
        ThermalLevel::Green => 1,
        ThermalLevel::Yellow => 2,
        ThermalLevel::Red => 4,
    }
}

/// The gate's admit/deny decision label given a [`ThermalLevel`].
///
/// Green and Yellow → ADMIT; Red → DENY.
pub fn gate_decision(level: ThermalLevel) -> &'static str {
    match level {
        ThermalLevel::Green | ThermalLevel::Yellow => "ADMIT",
        ThermalLevel::Red => "DENY",
    }
}

/// Color for the gate decision indicator.
pub fn decision_color(level: ThermalLevel) -> Color {
    match level {
        ThermalLevel::Green | ThermalLevel::Yellow => Color::Green,
        ThermalLevel::Red => Color::Red,
    }
}

/// Compute a gauge ratio for the build-slot indicator.
///
/// Returns a value in `[0.0, 1.0]`.  `active` is clamped to `[0, cap]`.
pub fn slot_ratio(active: u32, cap: u32) -> f64 {
    if cap == 0 {
        return 0.0;
    }
    let clamped = active.min(cap) as f64;
    clamped / cap as f64
}

/// Color for the slot-usage gauge: green < 50 %, yellow < 90 %, red otherwise.
pub fn slot_color(active: u32, cap: u32) -> Color {
    let ratio = slot_ratio(active, cap);
    if ratio < 0.5 {
        Color::Green
    } else if ratio < 0.9 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Columns below this width switch the TUI to compact layout (L81.11).
pub const COMPACT_WIDTH: u16 = 80;

/// True when the terminal is narrower than [`COMPACT_WIDTH`].
///
/// Prefer `frame.area().width` (ratatui already reflects `crossterm::terminal::size`)
/// or an explicit `COLUMNS`-derived width in tests.
pub fn is_compact(width: u16) -> bool {
    width < COMPACT_WIDTH
}

/// Lines for the thermal gate decision panel (FR-007 / AC-007.26 TUI slice).
///
/// Derives ADMIT/DENY from [`GateStatusSnapshot`] built with live agent count + RSS
/// (parity with `sharecli proc` / `status --json` gate).
pub fn gate_panel_lines(
    snap: &GateStatusSnapshot,
    thermal: ThermalLevel,
    compact: bool,
) -> Vec<Line<'static>> {
    let decision = snap.gate_decision.as_str();
    let color = if decision == "DENY" { Color::Red } else { decision_color(thermal) };
    let decision_span = Span::styled(
        format!("[ {decision} ]"),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    );

    if compact {
        return vec![Line::from(vec![
            Span::raw(" Gate: "),
            decision_span,
            Span::raw(format!(" RSS:{} {}", snap.agent_total_rss_bytes, snap.agent_contention)),
        ])];
    }

    let hint = if decision == "DENY" {
        if thermal == ThermalLevel::Red {
            "  — hypervisor will retry up to 5x before returning Err"
        } else {
            "  — agent contention limit reached; hypervisor will back-pressure"
        }
    } else {
        ""
    };

    vec![
        Line::from(vec![Span::raw("  Gate decision: "), decision_span, Span::raw(hint)]),
        Line::from(format!(
            "  Agent RSS total: {} bytes ({})",
            snap.agent_total_rss_bytes,
            format_rss_bytes(snap.agent_total_rss_bytes)
        )),
        Line::from(format!("  Agent contention: {}", snap.agent_contention)),
    ]
}

/// Lines for the runtime pool operator panel (FR-007 / AC-007.71 TUI slice).
pub fn pool_panel_lines(pool: Option<PoolOperatorPanel>, compact: bool) -> Vec<Line<'static>> {
    let Some(pool) = pool else {
        let msg = if compact { " pool unavailable" } else { "  Runtime pool unavailable" };
        return vec![Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)))];
    };

    let summary = format_pool_operator_line(&pool);
    if compact {
        return vec![Line::from(format!(" {summary}"))];
    }

    vec![
        Line::from(format!("  Node idle/total: {}/{}", pool.node_idle, pool.node_total)),
        Line::from(format!("  Bun idle/total:  {}/{}", pool.bun_idle, pool.bun_total)),
        Line::from(format!(
            "  Max per type: {} · {}",
            pool.max_per_type,
            if pool.healthy { "healthy" } else { "degraded" }
        )),
    ]
}

/// Lines for the proc-scan / status snapshot operator panel (FR-007 / AC-007.71 TUI slice).
pub fn status_panel_lines(
    status: Option<StatusOperatorPanel>,
    compact: bool,
) -> Vec<Line<'static>> {
    let Some(status) = status else {
        let msg = if compact { " status unavailable" } else { "  Proc scan status unavailable" };
        return vec![Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)))];
    };

    let summary = format_status_operator_line(&status);
    if compact {
        return vec![Line::from(format!(" {summary}"))];
    }

    vec![
        Line::from(format!("  Scanned:  {}", status.scanned)),
        Line::from(format!("  Watched:  {}", status.watched)),
        Line::from(format!(
            "  Managed:  {} · agent rows: {}",
            status.total_processes, status.agent_rows
        )),
    ]
}

/// Lines for the host resource watch panel (FR-007 / AC-007.10, AC-007.12 TUI slice).
pub fn resource_watch_lines(
    sample: Option<ResourceWatchSample>,
    compact: bool,
) -> Vec<Line<'static>> {
    let Some(sample) = sample else {
        let msg = if compact {
            " watch unavailable"
        } else {
            "  Resource watch unavailable on this host"
        };
        return vec![Line::from(Span::styled(msg, Style::default().fg(Color::Red)))];
    };

    if compact {
        return vec![Line::from(format!(
            " FD:{} RSS:{} L:{:.1} RX:{} TX:{}",
            sample.fd_count,
            sample.mem_rss_bytes,
            sample.load_1m,
            sample.net_rx_bytes,
            sample.net_tx_bytes,
        ))];
    }

    vec![
        Line::from(format!("  Open FDs:  {}", sample.fd_count)),
        Line::from(format!("  RSS:       {} bytes", sample.mem_rss_bytes)),
        Line::from(format!("  Load (1m): {:.2}", sample.load_1m)),
        Line::from(format!("  Net RX:    {} bytes", sample.net_rx_bytes)),
        Line::from(format!("  Net TX:    {} bytes", sample.net_tx_bytes)),
    ]
}

/// Lines for host agent inventory (FR-006 / thermal operator panel).
pub fn host_agent_lines(agents: &[DetectedAgent], compact: bool) -> Vec<Line<'static>> {
    if agents.is_empty() {
        let msg = if compact { " agents: none" } else { "  Host agents: none detected" };
        return vec![Line::from(Span::raw(msg))];
    }

    if compact {
        let summary =
            agents.iter().map(|a| format!("{}:{}", a.family, a.pid)).collect::<Vec<_>>().join(" ");
        return vec![Line::from(format!(" agents: {summary}"))];
    }

    let mut lines = vec![Line::from("  Host agents:")];
    for agent in agents.iter().take(4) {
        lines.push(Line::from(format!("    {} pid={} ({})", agent.family, agent.pid, agent.comm)));
    }
    if agents.len() > 4 {
        lines.push(Line::from(format!("    … +{} more", agents.len() - 4)));
    }
    lines
}

/// Lines for the FUSE negative-dentry panel (FR-009 / AC-009.9 TUI slice).
pub fn fuse_neg_dentry_lines(meters: NegDentryMeters, compact: bool) -> Vec<Line<'static>> {
    if compact {
        return vec![Line::from(format!(
            " neg:{} miss:{} {}%",
            meters.hits,
            meters.misses,
            meters.hit_rate_pct()
        ))];
    }

    vec![
        Line::from(format!("  Neg hits:     {}", meters.hits)),
        Line::from(format!("  Neg misses:   {}", meters.misses)),
        Line::from(format!("  Hit rate:     {}%", meters.hit_rate_pct())),
    ]
}

/// Lines for the Hypervisor coalesce cache panel (FR-008 / AC-008.11 TUI slice).
pub fn hypervisor_coalesce_lines(meters: CoalesceMeters, compact: bool) -> Vec<Line<'static>> {
    if compact {
        return vec![Line::from(format!(
            " coalesce:{} miss:{} nocache:{} {}%",
            meters.hits,
            meters.misses,
            meters.nocache_runs,
            meters.hit_rate_pct()
        ))];
    }

    vec![
        Line::from(format!("  Coalesce hits:   {}", meters.hits)),
        Line::from(format!("  Coalesce misses: {}", meters.misses)),
        Line::from(format!("  Nocache runs:    {}", meters.nocache_runs)),
        Line::from(format!("  Hit rate:        {}%", meters.hit_rate_pct())),
    ]
}

/// Lines for the Hypervisor SlotQueue panel (FR-008 / AC-008.12 TUI slice).
pub fn hypervisor_slot_queue_lines(meters: SlotQueueMeters, compact: bool) -> Vec<Line<'static>> {
    if compact {
        return vec![Line::from(format!(
            " slot:{} wait:{} to:{}",
            meters.acquires, meters.waits, meters.timeouts
        ))];
    }

    vec![
        Line::from(format!("  Slot acquires: {}", meters.acquires)),
        Line::from(format!("  Slot waits:    {}", meters.waits)),
        Line::from(format!("  Slot timeouts: {}", meters.timeouts)),
    ]
}

/// Lines for the mesh Maildir queue depth panel (FR-010 / AC-010.11 TUI slice).
pub fn mesh_maildir_lines(status: Option<MaildirStatus>, compact: bool) -> Vec<Line<'static>> {
    match status {
        Some(st) if compact => {
            vec![Line::from(format!(" mesh r:{} f:{} p:{}", st.ready, st.in_flight, st.pending))]
        }
        Some(st) => vec![
            Line::from(format!("  Mesh ready:     {}", st.ready)),
            Line::from(format!("  Mesh in-flight: {}", st.in_flight)),
            Line::from(format!("  Mesh pending:   {}", st.pending)),
        ],
        None => vec![Line::from("  Mesh queue:     unavailable")],
    }
}

/// Lines for the FUSE write-serialize / CoW panel (FR-009 / AC-009.10 TUI slice).
pub fn fuse_write_serialize_lines(
    meters: WriteSerializeMeters,
    compact: bool,
) -> Vec<Line<'static>> {
    if compact {
        return vec![Line::from(format!(
            " wr:{} st:{} cm:{} ds:{}",
            meters.passthrough_writes, meters.stages, meters.commits, meters.discards
        ))];
    }

    vec![
        Line::from(format!("  Passthrough:  {}", meters.passthrough_writes)),
        Line::from(format!("  Stages:       {}", meters.stages)),
        Line::from(format!("  Commits:      {}", meters.commits)),
        Line::from(format!("  Discards:     {}", meters.discards)),
    ]
}

/// Lines for the FUSE read-coalesce panel (FR-007 / AC-007.9 TUI slice).
pub fn fuse_coalesce_lines(meters: ReadCacheMeters, compact: bool) -> Vec<Line<'static>> {
    if compact {
        return vec![Line::from(format!(
            " hits:{} miss:{} {}%",
            meters.hits,
            meters.misses,
            meters.hit_rate_pct()
        ))];
    }

    vec![
        Line::from(format!("  Cache hits:   {}", meters.hits)),
        Line::from(format!("  Cache misses: {}", meters.misses)),
        Line::from(format!("  Hit rate:     {}%", meters.hit_rate_pct())),
    ]
}

/// Max agent rows rendered in the full-layout DetectedAgent panel.
pub const MAX_AGENT_LINES: usize = 4;

/// Lines for the host agent inventory panel (FR-006 / AC-006.9, AC-006.40 TUI slice).
pub fn agent_lines(
    agents: &[DetectedAgentWatch],
    state_by_pid: &HashMap<u32, char>,
    compact: bool,
) -> Vec<Line<'static>> {
    if agents.is_empty() {
        let msg = if compact { " none" } else { "  No agent processes detected" };
        return vec![Line::from(Span::styled(msg, Style::default().fg(Color::DarkGray)))];
    }

    if compact {
        let summary: String = agents
            .iter()
            .take(2)
            .map(|row| {
                let state = state_text_for_pid(state_by_pid, row.agent.pid);
                format!(
                    "{}:{}:{}@{}",
                    row.agent.family,
                    row.agent.pid,
                    state,
                    format_rss_bytes(row.resource.mem_rss_bytes)
                )
            })
            .collect::<Vec<_>>()
            .join(" ");
        let extra =
            if agents.len() > 2 { format!(" +{}", agents.len() - 2) } else { String::new() };
        return vec![Line::from(format!(" {summary}{extra}"))];
    }

    let mut lines = vec![Line::from(format!("  Agents: {}", agents.len()))];
    for row in agents.iter().take(MAX_AGENT_LINES) {
        let state = state_text_for_pid(state_by_pid, row.agent.pid);
        let fd = row.resource.fd_count.map(|n| format!(" FD {n}")).unwrap_or_default();
        lines.push(Line::from(format!(
            "    PID {}  {}  {}  RSS {}{}  ({})",
            row.agent.pid,
            state,
            row.agent.family,
            format_rss_bytes(row.resource.mem_rss_bytes),
            fd,
            row.agent.comm
        )));
    }
    if agents.len() > MAX_AGENT_LINES {
        lines.push(Line::from(format!("    … +{} more", agents.len() - MAX_AGENT_LINES)));
    }
    lines
}

/// Max tree lines rendered in the full-layout DetectedAgent panel (AC-006.22).
pub const MAX_AGENT_TREE_LINES: usize = 4;

fn format_tree_node_line(
    node: &AgentTreeNode,
    rss_by_pid: &HashMap<u32, u64>,
    state_by_pid: &HashMap<u32, char>,
) -> String {
    let family = node.family.map(|f| format!("{f} ")).unwrap_or_default();
    let state = state_text_for_pid(state_by_pid, node.pid);
    let rss = rss_by_pid
        .get(&node.pid)
        .map(|bytes| format!(" RSS {}", format_rss_bytes(*bytes)))
        .unwrap_or_default();
    format!("[{}] {state} {family}{}{}", node.pid, rss, format_comm_suffix(&node.comm))
}

fn format_comm_suffix(comm: &str) -> String {
    if comm.is_empty() {
        String::new()
    } else {
        format!(" ({comm})")
    }
}

fn append_agent_tree_lines(
    node: &AgentTreeNode,
    prefix: &str,
    is_last: bool,
    rss_by_pid: &HashMap<u32, u64>,
    state_by_pid: &HashMap<u32, char>,
    lines: &mut Vec<Line<'static>>,
    budget: &mut usize,
) {
    if *budget == 0 {
        return;
    }
    let connector = if prefix.is_empty() {
        String::new()
    } else if is_last {
        "└── ".to_string()
    } else {
        "├── ".to_string()
    };
    let lead = if prefix.is_empty() { "  " } else { prefix };
    lines.push(Line::from(format!(
        "{lead}{connector}{}",
        format_tree_node_line(node, rss_by_pid, state_by_pid)
    )));
    *budget -= 1;

    let child_prefix = if prefix.is_empty() {
        "    ".to_string()
    } else {
        format!("{}{}", prefix, if is_last { "    " } else { "│   " })
    };
    for (i, child) in node.children.iter().enumerate() {
        append_agent_tree_lines(
            child,
            &child_prefix,
            i + 1 == node.children.len(),
            rss_by_pid,
            state_by_pid,
            lines,
            budget,
        );
        if *budget == 0 {
            break;
        }
    }
}

/// Lines for the host agent inventory panel — tree when forests are present (AC-006.22, AC-006.39).
pub fn agent_forest_lines(
    forests: &[AgentTreeNode],
    watched: &[DetectedAgentWatch],
    state_by_pid: &HashMap<u32, char>,
    compact: bool,
) -> Vec<Line<'static>> {
    if compact || forests.is_empty() {
        return agent_lines(watched, state_by_pid, compact);
    }
    let rss_by_pid: HashMap<u32, u64> =
        watched.iter().map(|row| (row.agent.pid, row.resource.mem_rss_bytes)).collect();
    let mut lines = vec![Line::from(format!("  Forests: {}", forests.len()))];
    let mut budget = MAX_AGENT_TREE_LINES;
    for (i, root) in forests.iter().enumerate() {
        if budget == 0 {
            if i < forests.len() {
                lines.push(Line::from(format!("    … +{} more roots", forests.len() - i)));
            }
            break;
        }
        append_agent_tree_lines(root, "", true, &rss_by_pid, state_by_pid, &mut lines, &mut budget);
    }
    lines
}

/// Short pressure blurb for compact terminals; full sentence otherwise.
pub fn thermal_blurb(level: ThermalLevel, compact: bool) -> &'static str {
    if compact {
        match level {
            ThermalLevel::Green => "[GREEN] cool",
            ThermalLevel::Yellow => "[YELLOW] warm",
            ThermalLevel::Red => "[RED] hot — back-pressure",
        }
    } else {
        match level {
            ThermalLevel::Green => "[ GREEN  ] device is cool — spawns proceed",
            ThermalLevel::Yellow => "[ YELLOW ] device is warm — spawns proceed w/ warning",
            ThermalLevel::Red => "[ RED    ] device is hot — spawns BACK-PRESSURED",
        }
    }
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

/// Build-slot cap (max concurrent `cargo build|check|test` processes).
pub const DEFAULT_SLOT_CAP: u32 = 4;

/// Poll interval for the thermal governor.
pub const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Live application state.
pub struct App {
    /// Most-recent thermal level from the governor.
    pub thermal_level: ThermalLevel,
    /// Number of active build slots (detected via pgrep).
    pub active_slots: u32,
    /// Build-slot cap.
    pub slot_cap: u32,
    /// Timestamp of last poll.
    pub last_poll: Instant,
    /// Total number of polls performed.
    pub poll_count: u64,
    /// Latest host resource watch sample (None when capture fails on this host).
    pub resource_watch: Option<ResourceWatchSample>,
    /// Process-wide FUSE read-coalesce meters.
    pub fuse_meters: ReadCacheMeters,
    /// Process-wide FUSE negative-dentry meters.
    pub neg_dentry_meters: NegDentryMeters,
    /// Process-wide Hypervisor coalesce cache meters.
    pub coalesce_meters: CoalesceMeters,
    /// Process-wide Hypervisor SlotQueue meters.
    pub slot_queue_meters: SlotQueueMeters,
    /// Process-wide FUSE write-serialize / CoW meters.
    pub write_serialize_meters: WriteSerializeMeters,
    /// Mesh Maildir queue depth snapshot (FR-010 / AC-010.11).
    pub maildir_status: Option<MaildirStatus>,
    /// Host agent inventory with per-PID resource watch (FR-006 × FR-007).
    pub detected_agents: Vec<DetectedAgentWatch>,
    /// Agent-rooted process forests from proc scan (FR-006 / AC-006.22).
    pub agent_forests: Vec<AgentTreeNode>,
    /// Pinned forest-wide process state for tests; live render resolves via proc scan when unset.
    forest_state_by_pid: Option<HashMap<u32, char>>,
    /// Runtime pool operator panel (FR-007 / AC-007.71).
    pub pool_panel: Option<PoolOperatorPanel>,
    /// Proc-scan / managed-process operator panel (FR-007 / AC-007.71).
    pub status_panel: Option<StatusOperatorPanel>,
    /// Focused operator panel for keyboard navigation (C09 L81.3).
    pub focus: PanelFocus,
    /// When true, footer shows extended keybinding help.
    pub show_help_overlay: bool,
}

impl App {
    /// Create with a default state (Green, 0 active slots).
    pub fn new(slot_cap: u32) -> Self {
        Self {
            thermal_level: ThermalLevel::Green,
            active_slots: 0,
            slot_cap,
            last_poll: Instant::now(),
            poll_count: 0,
            resource_watch: None,
            fuse_meters: ReadCacheMeters::default(),
            neg_dentry_meters: NegDentryMeters::default(),
            coalesce_meters: CoalesceMeters::default(),
            slot_queue_meters: SlotQueueMeters::default(),
            write_serialize_meters: WriteSerializeMeters::default(),
            maildir_status: None,
            detected_agents: Vec::new(),
            agent_forests: Vec::new(),
            forest_state_by_pid: None,
            pool_panel: None,
            status_panel: None,
            focus: PanelFocus::default(),
            show_help_overlay: false,
        }
    }

    /// Update state from a new governor poll result.
    pub fn update(&mut self, level: ThermalLevel, active_slots: u32) {
        self.thermal_level = level;
        self.active_slots = active_slots;
        self.last_poll = Instant::now();
        self.poll_count += 1;
    }

    /// Refresh operator watch panels from live OS samples + global FUSE meters.
    pub fn poll_operator_meters(&mut self) {
        self.resource_watch = ResourceWatchSample::capture().ok();
        self.fuse_meters = global_read_cache_meters();
        self.neg_dentry_meters = global_neg_dentry_meters();
        self.coalesce_meters = global_coalesce_meters();
        self.slot_queue_meters = global_slot_queue_meters();
        self.write_serialize_meters = global_write_serialize_meters();
        self.maildir_status = capture_maildir_status().ok().flatten();
        self.detected_agents = watch_host_agents();
        self.agent_forests = build_host_agent_forests();
        self.forest_state_by_pid = None;
    }

    /// Apply pool + proc-scan operator panels from CLI/IPC-shaped snapshots (AC-007.71).
    pub fn apply_pool_status_panels(
        &mut self,
        pool: Option<PoolOperatorPanel>,
        status: Option<StatusOperatorPanel>,
    ) {
        self.pool_panel = pool;
        self.status_panel = status;
    }

    /// Test helper — pin pool/status operator panels for headless render tests.
    pub fn with_pool_status_panels(
        mut self,
        pool: Option<PoolOperatorPanel>,
        status: Option<StatusOperatorPanel>,
    ) -> Self {
        self.pool_panel = pool;
        self.status_panel = status;
        self
    }

    /// Test/golden helper — pin deterministic operator panel values.
    pub fn with_operator_meters(
        mut self,
        watch: Option<ResourceWatchSample>,
        fuse: ReadCacheMeters,
        neg: NegDentryMeters,
        coalesce: CoalesceMeters,
        slot_queue: SlotQueueMeters,
        write_serialize: WriteSerializeMeters,
    ) -> Self {
        self.resource_watch = watch;
        self.fuse_meters = fuse;
        self.neg_dentry_meters = neg;
        self.coalesce_meters = coalesce;
        self.slot_queue_meters = slot_queue;
        self.write_serialize_meters = write_serialize;
        self
    }

    /// Pin mesh Maildir depth for tests / goldens (FR-010 / AC-010.11).
    pub fn with_maildir_status(mut self, status: Option<MaildirStatus>) -> Self {
        self.maildir_status = status;
        self
    }

    /// Test helper — pin deterministic agent inventory for headless render tests.
    pub fn with_detected_agents(mut self, agents: Vec<DetectedAgentWatch>) -> Self {
        self.detected_agents = agents;
        self
    }

    /// Test helper — pin agent process forests for headless render tests (AC-006.22).
    pub fn with_agent_forests(mut self, forests: Vec<AgentTreeNode>) -> Self {
        self.agent_forests = forests;
        self
    }

    /// Test helper — pin forest-wide process state letters (AC-006.39).
    pub fn with_agent_forest_state(mut self, state_by_pid: HashMap<u32, char>) -> Self {
        self.forest_state_by_pid = Some(state_by_pid);
        self
    }

    fn agent_panel_state_by_pid(&self) -> HashMap<u32, char> {
        if let Some(map) = &self.forest_state_by_pid {
            return map.clone();
        }
        if !self.agent_forests.is_empty() {
            return build_host_forest_state_map(&self.agent_forests);
        }
        let pids: Vec<u32> = self.detected_agents.iter().map(|row| row.agent.pid).collect();
        build_host_agent_state_map(&pids)
    }
}

// ---------------------------------------------------------------------------
// Active-slot detection
// ---------------------------------------------------------------------------

/// Count running `cargo (build|check|test)` processes via `pgrep`.
///
/// Returns 0 on any error (pgrep missing, etc.) so the TUI degrades gracefully.
pub fn count_cargo_builds() -> u32 {
    let output =
        std::process::Command::new("pgrep").args(["-f", "cargo (build|check|test)"]).output();
    match output {
        Ok(out) => {
            let s = String::from_utf8_lossy(&out.stdout);
            s.lines().filter(|l| !l.trim().is_empty()).count() as u32
        }
        Err(_) => 0,
    }
}

// ---------------------------------------------------------------------------
// Render (pure, takes &mut Frame + &App)
// ---------------------------------------------------------------------------

/// Render the full TUI into `frame`.
///
/// Layout adapts to `frame.area().width` (backed by terminal size / `COLUMNS`).
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let compact = is_compact(area.width);
    let cramped = area.height < 58;
    let margin = if compact || area.height < 40 { 0 } else { 1 };
    let thermal_h = if compact { 4 } else { 5 };
    let gate_h = if compact { 4 } else { 5 };
    let pool_h = if compact { 3 } else { 4 };
    let status_h = if compact { 3 } else { 4 };
    let agents_h = if compact { 3 } else { 6 };
    let watch_h = if compact { 3 } else { 7 };
    let io_min = if compact { 8 } else { 6 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(margin)
        .constraints([
            Constraint::Length(3),         // title
            Constraint::Length(thermal_h), // thermal pressure block
            Constraint::Length(gate_h),    // gate decision
            Constraint::Length(3),         // slot gauge
            Constraint::Length(pool_h),    // runtime pool
            Constraint::Length(status_h),  // proc scan status
            Constraint::Length(agents_h),  // host agent inventory
            Constraint::Length(watch_h),   // host resource watch
            Constraint::Min(io_min),       // hypervisor + FUSE IO meters (flex)
            Constraint::Length(3),         // footer
        ])
        .split(area);

    render_title(frame, chunks[0], compact);
    render_thermal(frame, chunks[1], app, compact);
    render_decision(frame, chunks[2], app, compact);
    render_slots(frame, chunks[3], app, compact);
    render_pool_panel(frame, chunks[4], app, compact);
    render_status_panel(frame, chunks[5], app, compact);
    render_agents(frame, chunks[6], app, compact);
    render_resource_watch(frame, chunks[7], app, compact);
    render_fuse_coalesce(frame, chunks[8], app, compact || cramped);
    render_footer(frame, chunks[9], app, compact);
}

fn render_title(frame: &mut Frame, area: Rect, compact: bool) {
    let text = if compact { " thermal" } else { "  sharecli thermal monitor" };
    let title = Paragraph::new(Line::from(vec![Span::styled(
        text,
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )]))
    .block(Block::default().borders(Borders::ALL).title(" sharecli "));
    frame.render_widget(title, area);
}

fn render_thermal(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    let level = app.thermal_level;
    let color = level_color(level);
    let label = level_label(level);
    let raw = level_pressure_raw(level);

    let pressure = if compact {
        format!("{label} ({raw})")
    } else {
        format!("{label}  (kern.memorystatus_vm_pressure_level = {raw})")
    };

    let mut lines = vec![Line::from(vec![
        Span::raw(if compact { " P: " } else { "  Pressure level: " }),
        Span::styled(pressure, Style::default().fg(color).add_modifier(Modifier::BOLD)),
    ])];
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(thermal_blurb(level, compact), Style::default().fg(color)),
    ]));

    let title = if compact { " Thermal " } else { " Thermal Pressure " };
    let block = Block::default().borders(Borders::ALL).title(title);
    let para = Paragraph::new(lines).block(block);
    frame.render_widget(para, area);
}

fn focused_block(title: &str, focused: bool) -> Block<'_> {
    let block = Block::default().borders(Borders::ALL).title(title);
    if focused {
        block.border_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
    } else {
        block
    }
}

fn render_decision(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    let level = app.thermal_level;
    let agent_count = app.detected_agents.len();
    let total_rss = sum_detected_agent_rss_bytes(&app.detected_agents);
    let snap = gate_status_snapshot_with_rss(level, agent_count, total_rss);
    let lines = gate_panel_lines(&snap, level, compact);

    let title = if compact { " Gate " } else { " Gate Decision " };
    let block = focused_block(title, app.focus == PanelFocus::Gate);
    let para = Paragraph::new(lines).block(block);
    frame.render_widget(para, area);
}

fn render_slots(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    let ratio = slot_ratio(app.active_slots, app.slot_cap);
    let color = slot_color(app.active_slots, app.slot_cap);
    let label = if compact {
        format!(" {}/{} ", app.active_slots, app.slot_cap)
    } else {
        format!(" Build slots: {}/{} active ", app.active_slots, app.slot_cap)
    };

    let title = if compact { " Slots " } else { " Build Slots " };
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title(title))
        .gauge_style(Style::default().fg(color).bg(Color::DarkGray))
        .ratio(ratio)
        .label(label);
    frame.render_widget(gauge, area);
}

fn render_pool_panel(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    let title = if compact { " Pool " } else { " Runtime Pool " };
    let lines = pool_panel_lines(app.pool_panel, compact);
    let block = focused_block(title, app.focus == PanelFocus::Pool);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_status_panel(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    let title = if compact { " Status " } else { " Proc Scan Status " };
    let lines = status_panel_lines(app.status_panel, compact);
    let block = focused_block(title, app.focus == PanelFocus::Status);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_agents(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    let title = if compact { " Agents " } else { " Detected Agents " };
    let state_by_pid = app.agent_panel_state_by_pid();
    let lines =
        agent_forest_lines(&app.agent_forests, &app.detected_agents, &state_by_pid, compact);
    let block = focused_block(title, app.focus == PanelFocus::Agents);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_resource_watch(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    let title = if compact { " Watch " } else { " Host Resource Watch " };
    let lines = resource_watch_lines(app.resource_watch, compact);
    let block = focused_block(title, app.focus == PanelFocus::HostWatch);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_fuse_coalesce(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    let title = if compact { " IO " } else { " Hypervisor IO Meters " };
    let mut lines = hypervisor_coalesce_lines(app.coalesce_meters, compact);
    lines.extend(hypervisor_slot_queue_lines(app.slot_queue_meters, compact));
    lines.extend(mesh_maildir_lines(app.maildir_status.clone(), compact));
    lines.extend(fuse_coalesce_lines(app.fuse_meters, compact));
    lines.extend(fuse_neg_dentry_lines(app.neg_dentry_meters, compact));
    lines.extend(fuse_write_serialize_lines(app.write_serialize_meters, compact));
    let block = Block::default().borders(Borders::ALL).title(title);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    if app.show_help_overlay {
        let help = Paragraph::new(Line::from(vec![Span::styled(
            HELP_OVERLAY_HINT,
            Style::default().fg(Color::Yellow),
        )]))
        .block(Block::default().borders(Borders::ALL).title(" help "));
        frame.render_widget(help, area);
        return;
    }

    let elapsed = app.last_poll.elapsed().as_secs();
    let meta = if compact {
        format!(" polls:{} last:{}s", app.poll_count, elapsed)
    } else {
        format!(
            "  polls: {}  last: {}s ago  interval: {}s",
            app.poll_count,
            elapsed,
            POLL_INTERVAL.as_secs()
        )
    };
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" q", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" quit  "),
        Span::styled(" Ctrl-C", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" quit  "),
        Span::styled(" ?", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" help  "),
        Span::raw(meta),
    ]))
    .block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, area);
}

// ---------------------------------------------------------------------------
// Event loop
// ---------------------------------------------------------------------------

/// Supplementary pool/status poll hook (CLI/IPC-shaped snapshots for AC-007.71).
pub type PoolStatusPollFn = dyn FnMut() -> (Option<PoolOperatorPanel>, Option<StatusOperatorPanel>);

fn poll_pool_status_panels(app: &mut App, poll_pool_status: Option<&mut PoolStatusPollFn>) {
    if let Some(poll) = poll_pool_status {
        let (pool, status) = poll();
        app.apply_pool_status_panels(pool, status);
    }
}

/// Launch the TUI, polling `governor` every [`POLL_INTERVAL`].
///
/// Returns when the user presses `q` or `Ctrl-C`.
pub fn run(governor: &ThermalGovernor, slot_cap: u32) -> Result<()> {
    run_with_pool_status(governor, slot_cap, None)
}

/// Launch the TUI with optional live pool/status operator panel refresh (AC-007.71).
pub fn run_with_pool_status(
    governor: &ThermalGovernor,
    slot_cap: u32,
    mut poll_pool_status: Option<Box<PoolStatusPollFn>>,
) -> Result<()> {
    use std::io;

    use crossterm::{
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    };
    use ratatui::backend::CrosstermBackend;
    use ratatui::Terminal;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(slot_cap);

    // Initial poll before first draw.
    let initial_level = governor.poll().unwrap_or(ThermalLevel::Green);
    let initial_slots = count_cargo_builds();
    app.update(initial_level, initial_slots);
    app.poll_operator_meters();
    poll_pool_status_panels(&mut app, poll_pool_status.as_deref_mut());

    let result = event_loop(&mut terminal, &mut app, governor, poll_pool_status.as_deref_mut());

    // Always restore terminal even on error.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn event_loop(
    terminal: &mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
    governor: &ThermalGovernor,
    mut poll_pool_status: Option<&mut PoolStatusPollFn>,
) -> Result<()> {
    loop {
        terminal.draw(|f| render(f, app))?;

        // Poll for input with a timeout equal to the poll interval.
        // Also accept Resize so layout reflows when COLUMNS / terminal size changes.
        if event::poll(POLL_INTERVAL)? {
            match event::read()? {
                Event::Key(key) => match handle_key(&key) {
                    KeyAction::Quit => break,
                    KeyAction::ForcePoll => {
                        let level = governor.poll().unwrap_or(ThermalLevel::Green);
                        let slots = count_cargo_builds();
                        app.update(level, slots);
                        app.poll_operator_meters();
                        poll_pool_status_panels(app, poll_pool_status.as_deref_mut());
                    }
                    action => apply_key_action(app, action),
                },
                Event::Resize(_, _) => {
                    // Next draw uses the new frame.area().width (compact vs full).
                }
                _ => {}
            }
        }

        // Refresh thermal + slot state.
        let level = governor.poll().unwrap_or(ThermalLevel::Green);
        let slots = count_cargo_builds();
        app.update(level, slots);
        app.poll_operator_meters();
        poll_pool_status_panels(app, poll_pool_status.as_deref_mut());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — pure-function coverage
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use proptest::prelude::*;

    use super::*;

    fn key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent { code, modifiers, kind: KeyEventKind::Press, state: KeyEventState::NONE }
    }

    // --- is_quit_key ---
    #[test]
    fn test_is_quit_key_q() {
        assert!(is_quit_key(&key_event(KeyCode::Char('q'), KeyModifiers::NONE)));
    }

    #[test]
    fn test_is_compact_columns() {
        assert!(is_compact(40));
        assert!(is_compact(79));
        assert!(!is_compact(80));
        assert!(!is_compact(120));
    }

    #[test]
    fn test_thermal_blurb_adapts() {
        assert_eq!(thermal_blurb(ThermalLevel::Red, true), "[RED] hot — back-pressure");
        assert!(thermal_blurb(ThermalLevel::Red, false).contains("BACK-PRESSURED"));
    }

    #[test]
    fn test_is_quit_key_ctrl_c() {
        assert!(is_quit_key(&key_event(KeyCode::Char('c'), KeyModifiers::CONTROL)));
    }

    #[test]
    fn test_is_quit_key_other_ignored() {
        assert!(!is_quit_key(&key_event(KeyCode::Char('a'), KeyModifiers::NONE)));
        assert!(!is_quit_key(&key_event(KeyCode::Char('c'), KeyModifiers::NONE)));
    }

    // --- handle_key matrix (C09 L81.3) ---
    #[test]
    fn test_handle_key_tab_cycles_focus_forward() {
        assert_eq!(handle_key(&key_event(KeyCode::Tab, KeyModifiers::NONE)), KeyAction::FocusNext);
    }

    #[test]
    fn test_handle_key_shift_tab_cycles_focus_backward() {
        assert_eq!(handle_key(&key_event(KeyCode::Tab, KeyModifiers::SHIFT)), KeyAction::FocusPrev);
    }

    #[test]
    fn test_handle_key_digit_jumps_to_panel() {
        assert_eq!(
            handle_key(&key_event(KeyCode::Char('1'), KeyModifiers::NONE)),
            KeyAction::FocusPanel(PanelFocus::Gate)
        );
        assert_eq!(
            handle_key(&key_event(KeyCode::Char('2'), KeyModifiers::NONE)),
            KeyAction::FocusPanel(PanelFocus::Pool)
        );
        assert_eq!(
            handle_key(&key_event(KeyCode::Char('3'), KeyModifiers::NONE)),
            KeyAction::FocusPanel(PanelFocus::Status)
        );
        assert_eq!(
            handle_key(&key_event(KeyCode::Char('4'), KeyModifiers::NONE)),
            KeyAction::FocusPanel(PanelFocus::HostWatch)
        );
        assert_eq!(
            handle_key(&key_event(KeyCode::Char('5'), KeyModifiers::NONE)),
            KeyAction::FocusPanel(PanelFocus::Agents)
        );
    }

    #[test]
    fn test_handle_key_r_force_poll() {
        assert_eq!(
            handle_key(&key_event(KeyCode::Char('r'), KeyModifiers::NONE)),
            KeyAction::ForcePoll
        );
    }

    #[test]
    fn test_handle_key_question_toggle_help() {
        assert_eq!(
            handle_key(&key_event(KeyCode::Char('?'), KeyModifiers::NONE)),
            KeyAction::ToggleHelp
        );
    }

    #[test]
    fn test_panel_focus_tab_cycle_order() {
        let mut focus = PanelFocus::Gate;
        focus = focus.next();
        assert_eq!(focus, PanelFocus::Pool);
        focus = focus.next();
        assert_eq!(focus, PanelFocus::Status);
        focus = focus.next();
        assert_eq!(focus, PanelFocus::HostWatch);
        focus = focus.next();
        assert_eq!(focus, PanelFocus::Agents);
        focus = focus.next();
        assert_eq!(focus, PanelFocus::Gate);

        focus = focus.prev();
        assert_eq!(focus, PanelFocus::Agents);
    }

    #[test]
    fn test_apply_key_action_toggles_help_overlay() {
        let mut app = App::new(4);
        assert!(!app.show_help_overlay);
        apply_key_action(&mut app, KeyAction::ToggleHelp);
        assert!(app.show_help_overlay);
        apply_key_action(&mut app, KeyAction::ToggleHelp);
        assert!(!app.show_help_overlay);
    }

    #[test]
    fn test_render_footer_shows_help_hint() {
        use ratatui::{backend::TestBackend, Terminal};
        let backend = TestBackend::new(120, 64);
        let mut terminal = Terminal::new(backend).unwrap();
        let app = App::new(4);
        terminal.draw(|f| render(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let rendered: String = buf.content.iter().map(|c| c.symbol().to_string()).collect();
        assert!(rendered.contains("help"), "footer must document ? help; got: {rendered}");
    }

    #[test]
    fn test_render_help_overlay_when_enabled() {
        use ratatui::{backend::TestBackend, Terminal};
        let backend = TestBackend::new(120, 64);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(4);
        app.show_help_overlay = true;
        terminal.draw(|f| render(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let rendered: String = buf.content.iter().map(|c| c.symbol().to_string()).collect();
        assert!(rendered.contains("Tab"), "help overlay must list Tab cycle; got: {rendered}");
        assert!(rendered.contains("agents"), "help overlay must list agents panel");
    }

    #[test]
    fn test_pool_panel_lines_full() {
        let pool = PoolOperatorPanel {
            node_total: 2,
            node_idle: 1,
            bun_total: 1,
            bun_idle: 0,
            max_per_type: 4,
            healthy: true,
        };
        let lines = pool_panel_lines(Some(pool), false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Node idle/total: 1/2"));
        assert!(text.contains("Bun idle/total:  0/1"));
        assert!(text.contains("Max per type: 4 · healthy"));
    }

    #[test]
    fn test_status_panel_lines_full() {
        let status =
            StatusOperatorPanel { scanned: 50, watched: 1, total_processes: 2, agent_rows: 1 };
        let lines = status_panel_lines(Some(status), false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Scanned:  50"));
        assert!(text.contains("Watched:  1"));
        assert!(text.contains("Managed:  2 · agent rows: 1"));
    }

    #[test]
    fn test_pool_status_panel_lines_compact_match_tray() {
        let pool = PoolOperatorPanel {
            node_total: 2,
            node_idle: 1,
            bun_total: 1,
            bun_idle: 0,
            max_per_type: 4,
            healthy: true,
        };
        let status =
            StatusOperatorPanel { scanned: 50, watched: 1, total_processes: 2, agent_rows: 1 };
        let pool_text: String =
            pool_panel_lines(Some(pool), true).iter().map(|l| l.to_string()).collect();
        let status_text: String =
            status_panel_lines(Some(status), true).iter().map(|l| l.to_string()).collect();
        assert!(pool_text.contains("Pool node 2/1 idle · bun 1/0 idle · max 4 · healthy"));
        assert!(status_text.contains("Proc scan 50 · watched 1 · 2 managed · 1 agent row(s)"));
    }

    // --- level_label ---
    #[test]
    fn test_level_label_green() {
        assert_eq!(level_label(ThermalLevel::Green), "GREEN");
    }

    #[test]
    fn test_level_label_yellow() {
        assert_eq!(level_label(ThermalLevel::Yellow), "YELLOW");
    }

    #[test]
    fn test_level_label_red() {
        assert_eq!(level_label(ThermalLevel::Red), "RED");
    }

    // --- level_color ---
    #[test]
    fn test_level_color_green() {
        assert_eq!(level_color(ThermalLevel::Green), Color::Green);
    }

    #[test]
    fn test_level_color_yellow() {
        assert_eq!(level_color(ThermalLevel::Yellow), Color::Yellow);
    }

    #[test]
    fn test_level_color_red() {
        assert_eq!(level_color(ThermalLevel::Red), Color::Red);
    }

    // --- level_pressure_raw ---
    #[test]
    fn test_pressure_raw_green() {
        assert_eq!(level_pressure_raw(ThermalLevel::Green), 1);
    }

    #[test]
    fn test_pressure_raw_yellow() {
        assert_eq!(level_pressure_raw(ThermalLevel::Yellow), 2);
    }

    #[test]
    fn test_pressure_raw_red() {
        assert_eq!(level_pressure_raw(ThermalLevel::Red), 4);
    }

    // --- gate_decision ---
    #[test]
    fn test_decision_green_admit() {
        assert_eq!(gate_decision(ThermalLevel::Green), "ADMIT");
    }

    // --- gate_panel_lines (AC-007.26) ---
    #[test]
    fn test_gate_panel_lines_rss_refuse_denies_on_green() {
        const RSS_REFUSE: u64 = 32 * 1_073_741_824;
        let snap = gate_status_snapshot_with_rss(ThermalLevel::Green, 1, RSS_REFUSE);
        assert_eq!(snap.gate_decision, "DENY");
        assert_eq!(snap.agent_contention, "REFUSE");

        let lines = gate_panel_lines(&snap, ThermalLevel::Green, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("DENY"), "full gate panel MUST show DENY; got: {text}");
        assert!(
            text.contains(&RSS_REFUSE.to_string()),
            "full gate panel MUST show agent RSS total; got: {text}"
        );
        assert!(text.contains("REFUSE"), "full gate panel MUST show contention; got: {text}");

        let compact = gate_panel_lines(&snap, ThermalLevel::Green, true);
        let compact_text: String = compact.iter().map(|l| l.to_string()).collect();
        assert!(compact_text.contains("DENY"), "compact gate MUST show DENY; got: {compact_text}");
        assert!(
            compact_text.contains(&RSS_REFUSE.to_string()),
            "compact gate MUST show agent RSS total; got: {compact_text}"
        );
        assert!(
            compact_text.contains("REFUSE"),
            "compact gate MUST show contention; got: {compact_text}"
        );
    }

    #[test]
    fn test_gate_panel_lines_count_only_warn_still_admits() {
        let snap = gate_status_snapshot_with_rss(ThermalLevel::Green, 4, 0);
        assert_eq!(snap.gate_decision, "ADMIT");
        assert_eq!(snap.agent_contention, "WARN");

        let lines = gate_panel_lines(&snap, ThermalLevel::Green, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("ADMIT"));
        assert!(text.contains("WARN"));
    }

    #[test]
    fn test_decision_yellow_admit() {
        assert_eq!(gate_decision(ThermalLevel::Yellow), "ADMIT");
    }

    #[test]
    fn test_decision_red_deny() {
        assert_eq!(gate_decision(ThermalLevel::Red), "DENY");
    }

    // --- decision_color ---
    #[test]
    fn test_decision_color_green() {
        assert_eq!(decision_color(ThermalLevel::Green), Color::Green);
    }

    #[test]
    fn test_decision_color_yellow() {
        assert_eq!(decision_color(ThermalLevel::Yellow), Color::Green);
    }

    #[test]
    fn test_decision_color_red() {
        assert_eq!(decision_color(ThermalLevel::Red), Color::Red);
    }

    // --- slot_ratio ---
    #[test]
    fn test_slot_ratio_zero_active() {
        assert!((slot_ratio(0, 4) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_slot_ratio_half() {
        assert!((slot_ratio(2, 4) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn test_slot_ratio_full() {
        assert!((slot_ratio(4, 4) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_slot_ratio_overflow_clamped() {
        // active > cap should clamp to 1.0
        assert!((slot_ratio(10, 4) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn test_slot_ratio_zero_cap() {
        assert!((slot_ratio(5, 0) - 0.0).abs() < 1e-9);
    }

    // --- slot_color ---
    #[test]
    fn test_slot_color_green_below_half() {
        assert_eq!(slot_color(1, 4), Color::Green);
    }

    #[test]
    fn test_slot_color_yellow_between_half_and_90() {
        assert_eq!(slot_color(2, 4), Color::Yellow); // 0.5 → yellow
        assert_eq!(slot_color(3, 4), Color::Yellow); // 0.75 → yellow
    }

    /// FR-003 / C07 L65 — catch `< 0.9` vs `<= 0.9` mutant (exact 90% is red).
    #[test]
    fn test_slot_color_red_at_exact_90_percent() {
        assert_eq!(slot_color(9, 10), Color::Red); // 0.9 → red (not yellow)
    }

    #[test]
    fn test_slot_color_red_at_cap() {
        assert_eq!(slot_color(4, 4), Color::Red); // 1.0 → red
    }

    #[test]
    fn test_slot_color_red_overflow_clamped() {
        assert_eq!(slot_color(10, 4), Color::Red);
    }

    // --- App::update ---
    #[test]
    fn test_app_update_increments_poll_count() {
        let mut app = App::new(4);
        assert_eq!(app.poll_count, 0);
        app.update(ThermalLevel::Yellow, 2);
        assert_eq!(app.poll_count, 1);
        app.update(ThermalLevel::Red, 3);
        assert_eq!(app.poll_count, 2);
    }

    #[test]
    fn test_app_update_stores_level() {
        let mut app = App::new(4);
        app.update(ThermalLevel::Red, 0);
        assert_eq!(app.thermal_level, ThermalLevel::Red);
    }

    #[test]
    fn test_app_update_stores_slots() {
        let mut app = App::new(4);
        app.update(ThermalLevel::Green, 3);
        assert_eq!(app.active_slots, 3);
    }

    use sharecli_fleet::AgentResourceSample;

    fn agent_watch(
        pid: u32,
        family: &'static str,
        comm: &str,
        rss: u64,
        fds: Option<u64>,
    ) -> DetectedAgentWatch {
        DetectedAgentWatch {
            agent: DetectedAgent { pid, family, comm: comm.into() },
            resource: AgentResourceSample { mem_rss_bytes: rss, fd_count: fds },
        }
    }

    #[test]
    fn test_agent_lines_empty_full() {
        let lines = agent_lines(&[], &HashMap::new(), false);
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(rendered.contains("No agent processes detected"));
    }

    #[test]
    fn test_agent_lines_lists_detected_agents() {
        let agents = vec![
            agent_watch(100, "claude", "claude", 52_428_800, Some(42)),
            agent_watch(200, "cursor", "cursor-agent", 104_857_600, None),
        ];
        let mut state = HashMap::new();
        state.insert(100, 'S');
        state.insert(200, 'R');
        let lines = agent_lines(&agents, &state, false);
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(rendered.contains("Agents: 2"));
        assert!(rendered.contains("PID 100  S  claude"));
        assert!(rendered.contains("RSS 50M"));
        assert!(rendered.contains("FD 42"));
        assert!(rendered.contains("PID 200  R  cursor"));
    }

    #[test]
    fn test_agent_lines_truncates_overflow() {
        let agents: Vec<DetectedAgentWatch> = (0..6)
            .map(|i| agent_watch(100 + i, "claude", &format!("claude-{i}"), 1_048_576, None))
            .collect();
        let lines = agent_lines(&agents, &HashMap::new(), false);
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(rendered.contains("+2 more"));
    }

    #[test]
    fn test_agent_lines_missing_state_dash() {
        let agents = vec![agent_watch(100, "claude", "claude", 1_048_576, None)];
        let lines = agent_lines(&agents, &HashMap::new(), false);
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(rendered.contains("PID 100  -  claude"), "missing state MUST show `-`");
    }

    #[test]
    fn test_agent_lines_compact_includes_state() {
        let agents = vec![agent_watch(100, "claude", "claude", 52_428_800, None)];
        let mut state = HashMap::new();
        state.insert(100, 'S');
        let lines = agent_lines(&agents, &state, true);
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(rendered.contains("claude:100:S@"), "compact MUST show state after PID");
    }

    #[test]
    fn test_resource_watch_lines_full() {
        let sample = ResourceWatchSample {
            fd_count: 42,
            net_rx_bytes: 8192,
            net_tx_bytes: 4096,
            mem_rss_bytes: 1_048_576,
            load_1m: 1.25,
        };
        let lines = resource_watch_lines(Some(sample), false);
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(rendered.contains("Open FDs:") && rendered.contains("42"));
        assert!(rendered.contains("RSS:") && rendered.contains("1048576"));
        assert!(rendered.contains("Load (1m):"));
        assert!(rendered.contains("Net RX:") && rendered.contains("8192"));
        assert!(rendered.contains("Net TX:") && rendered.contains("4096"));
    }

    #[test]
    fn test_fuse_coalesce_lines_full() {
        let meters = ReadCacheMeters { hits: 7, misses: 3 };
        let lines = fuse_coalesce_lines(meters, false);
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(rendered.contains("Cache hits:") && rendered.contains("7"));
        assert!(rendered.contains("Cache misses:") && rendered.contains("3"));
        assert!(rendered.contains("Hit rate:") && rendered.contains("70"));
    }

    #[test]
    fn test_resource_watch_lines_unavailable() {
        let lines = resource_watch_lines(None, false);
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(rendered.contains("unavailable"));
    }

    // --- Headless render smoke test (FakeThermalGate via ThermalGovernor mock) ---
    #[test]
    fn test_host_agent_lines_full() {
        let agents = vec![DetectedAgent { pid: 42, family: "claude", comm: "claude".into() }];
        let lines = host_agent_lines(&agents, false);
        let rendered: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(rendered.contains("Host agents:") && rendered.contains("claude"));
    }

    #[test]
    fn test_render_green_headless() {
        use ratatui::{backend::TestBackend, Terminal};
        let backend = TestBackend::new(120, 64);
        let mut terminal = Terminal::new(backend).unwrap();
        let sample = ResourceWatchSample {
            fd_count: 12,
            net_rx_bytes: 100,
            net_tx_bytes: 50,
            mem_rss_bytes: 4096,
            load_1m: 0.5,
        };
        let mut app = App::new(4)
            .with_operator_meters(
                Some(sample),
                ReadCacheMeters { hits: 2, misses: 1 },
                NegDentryMeters { hits: 1, misses: 0 },
                CoalesceMeters { hits: 4, misses: 1, nocache_runs: 2 },
                SlotQueueMeters { acquires: 3, waits: 2, timeouts: 0 },
                WriteSerializeMeters { passthrough_writes: 2, stages: 1, commits: 1, discards: 0 },
            )
            .with_detected_agents(vec![agent_watch(4242, "claude", "claude", 4_096, Some(12))]);
        app.update(ThermalLevel::Green, 0);
        // Should not panic.
        terminal.draw(|f| render(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        // Title must appear somewhere.
        let rendered: String = buf.content.iter().map(|c| c.symbol().to_string()).collect();
        assert!(rendered.contains("GREEN"), "expected GREEN in rendered output");
        assert!(rendered.contains("ADMIT"), "expected ADMIT in rendered output");
        assert!(rendered.contains("Host Resource Watch"), "expected watch panel");
        assert!(rendered.contains("Runtime Pool"), "expected pool panel (AC-007.71)");
        assert!(rendered.contains("Proc Scan Status"), "expected status panel (AC-007.71)");
        assert!(rendered.contains("Detected Agents"), "expected agent panel");
        assert!(rendered.contains("PID 4242"), "expected agent row");
        assert!(rendered.contains("RSS"), "expected per-agent RSS in agent panel");
        assert!(rendered.contains("Hypervisor IO Meters"), "expected io panel");
        assert!(rendered.contains("Open FDs:"), "expected FD watch line");
        assert!(rendered.contains("Coalesce hits:"), "expected coalesce meters");
        assert!(rendered.contains("Slot acquires:"), "expected slot queue meters");
        assert!(rendered.contains("Cache hits:"), "expected fuse meters");
        assert!(rendered.contains("Neg hits:"), "expected neg dentry meters");
        assert!(rendered.contains("Passthrough:"), "expected write-serialize meters");
    }

    #[test]
    fn test_render_red_headless() {
        use ratatui::{backend::TestBackend, Terminal};
        let backend = TestBackend::new(120, 64);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(4).with_operator_meters(
            Some(ResourceWatchSample {
                fd_count: 8,
                net_rx_bytes: 0,
                net_tx_bytes: 0,
                mem_rss_bytes: 8192,
                load_1m: 2.0,
            }),
            ReadCacheMeters { hits: 0, misses: 0 },
            NegDentryMeters { hits: 0, misses: 0 },
            CoalesceMeters::default(),
            SlotQueueMeters::default(),
            WriteSerializeMeters::default(),
        );
        app.update(ThermalLevel::Red, 4);
        terminal.draw(|f| render(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let rendered: String = buf.content.iter().map(|c| c.symbol().to_string()).collect();
        assert!(rendered.contains("RED"), "expected RED in rendered output");
        assert!(rendered.contains("DENY"), "expected DENY in rendered output");
    }

    #[test]
    fn test_render_yellow_headless() {
        use ratatui::{backend::TestBackend, Terminal};
        let backend = TestBackend::new(120, 64);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(4).with_operator_meters(
            Some(ResourceWatchSample {
                fd_count: 16,
                net_rx_bytes: 0,
                net_tx_bytes: 0,
                mem_rss_bytes: 2048,
                load_1m: 1.0,
            }),
            ReadCacheMeters { hits: 1, misses: 1 },
            NegDentryMeters { hits: 0, misses: 1 },
            CoalesceMeters::default(),
            SlotQueueMeters::default(),
            WriteSerializeMeters::default(),
        );
        app.update(ThermalLevel::Yellow, 2);
        terminal.draw(|f| render(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let rendered: String = buf.content.iter().map(|c| c.symbol().to_string()).collect();
        assert!(rendered.contains("YELLOW"), "expected YELLOW in rendered output");
        assert!(rendered.contains("ADMIT"), "expected ADMIT in rendered output");
    }

    // --- FakeThermalGate (via ThermalGovernor::with_mock) poll round-trip ---
    #[test]
    fn test_fake_gate_green_poll() {
        let gov = ThermalGovernor::with_mock(ThermalLevel::Green);
        let level = gov.poll().unwrap();
        assert_eq!(level, ThermalLevel::Green);
        assert_eq!(gate_decision(level), "ADMIT");
    }

    #[test]
    fn test_fake_gate_red_poll() {
        let gov = ThermalGovernor::with_mock(ThermalLevel::Red);
        let level = gov.poll().unwrap();
        assert_eq!(level, ThermalLevel::Red);
        assert_eq!(gate_decision(level), "DENY");
    }

    #[test]
    fn test_fake_gate_yellow_poll() {
        let gov = ThermalGovernor::with_mock(ThermalLevel::Yellow);
        let level = gov.poll().unwrap();
        assert_eq!(level, ThermalLevel::Yellow);
        assert_eq!(gate_decision(level), "ADMIT");
    }

    // --- PanelFocus::from_digit (C07 L65 mutant coverage) ---
    #[test]
    fn test_panel_focus_from_digit_maps_1_to_5() {
        assert_eq!(PanelFocus::from_digit('1'), Some(PanelFocus::Gate));
        assert_eq!(PanelFocus::from_digit('2'), Some(PanelFocus::Pool));
        assert_eq!(PanelFocus::from_digit('3'), Some(PanelFocus::Status));
        assert_eq!(PanelFocus::from_digit('4'), Some(PanelFocus::HostWatch));
        assert_eq!(PanelFocus::from_digit('5'), Some(PanelFocus::Agents));
    }

    #[test]
    fn test_panel_focus_from_digit_rejects_others() {
        assert_eq!(PanelFocus::from_digit('0'), None);
        assert_eq!(PanelFocus::from_digit('6'), None);
        assert_eq!(PanelFocus::from_digit('q'), None);
        assert_eq!(PanelFocus::from_digit('\0'), None);
    }

    // --- gate_panel_lines hint + color branches (C07 L65) ---
    #[test]
    fn test_gate_panel_lines_deny_red_mentions_hypervisor_retry() {
        // RED + DENY hint branch: "hypervisor will retry up to 5x".
        let snap = gate_status_snapshot_with_rss(ThermalLevel::Red, 1, 0);
        assert_eq!(snap.gate_decision, "DENY");
        let lines = gate_panel_lines(&snap, ThermalLevel::Red, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            text.contains("retry up to 5x"),
            "RED+DENY MUST show the hypervisor-retry hint; got: {text}"
        );
    }

    #[test]
    fn test_gate_panel_lines_deny_green_mentions_back_pressure() {
        // GREEN + DENY (contention REFUSE) hint branch: back-pressure copy.
        const RSS_REFUSE: u64 = 32 * 1_073_741_824;
        let snap = gate_status_snapshot_with_rss(ThermalLevel::Green, 1, RSS_REFUSE);
        assert_eq!(snap.gate_decision, "DENY");
        let lines = gate_panel_lines(&snap, ThermalLevel::Green, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(
            text.contains("back-pressure"),
            "DENY+non-RED MUST show the back-pressure hint; got: {text}"
        );
    }

    #[test]
    fn test_gate_panel_lines_compact_deny_uses_red_span() {
        // Compact branch picks the color by decision: DENY -> Red span style.
        const RSS_REFUSE: u64 = 32 * 1_073_741_824;
        let snap = gate_status_snapshot_with_rss(ThermalLevel::Green, 1, RSS_REFUSE);
        let lines = gate_panel_lines(&snap, ThermalLevel::Green, true);
        let line = &lines[0];
        let decision = line
            .spans
            .iter()
            .find(|span| span.content.contains("[ DENY ]"))
            .expect("compact gate line MUST render the decision span");
        assert_eq!(
            decision.style.fg,
            Some(Color::Red),
            "DENY decision span MUST be styled red in compact layout"
        );
    }

    // --- host_agent_lines truncation (C07 L65) ---
    #[test]
    fn test_host_agent_lines_truncates_beyond_four() {
        let agents: Vec<DetectedAgent> = (0..5)
            .map(|i| DetectedAgent { pid: 100 + i, family: "claude", comm: format!("claude-{i}") })
            .collect();
        let lines = host_agent_lines(&agents, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("+1 more"), "5 agents MUST show +1 more; got: {text}");
    }

    #[test]
    fn test_host_agent_lines_exactly_four_no_truncation() {
        let agents: Vec<DetectedAgent> = (0..4)
            .map(|i| DetectedAgent { pid: 100 + i, family: "claude", comm: format!("claude-{i}") })
            .collect();
        let lines = host_agent_lines(&agents, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(!text.contains("more"), "4 agents MUST NOT truncate; got: {text}");
    }

    // --- mesh_maildir_lines (C07 L65) ---
    #[test]
    fn test_mesh_maildir_compact_and_full() {
        use std::path::PathBuf;
        let status =
            MaildirStatus { path: PathBuf::from("/tmp/q"), ready: 3, in_flight: 2, pending: 5 };
        let compact: String =
            mesh_maildir_lines(Some(status.clone()), true).iter().map(|l| l.to_string()).collect();
        assert!(compact.contains("r:3 f:2 p:5"), "compact MUST show depths; got: {compact}");

        let full: String =
            mesh_maildir_lines(Some(status), false).iter().map(|l| l.to_string()).collect();
        assert!(full.contains("Mesh ready:     3"), "full MUST show ready; got: {full}");
        assert!(full.contains("Mesh in-flight: 2"), "full MUST show in-flight; got: {full}");
        assert!(full.contains("Mesh pending:   5"), "full MUST show pending; got: {full}");
    }

    #[test]
    fn test_mesh_maildir_lines_unavailable() {
        let text: String = mesh_maildir_lines(None, false).iter().map(|l| l.to_string()).collect();
        assert!(text.contains("unavailable"), "None MUST render unavailable; got: {text}");
    }

    // --- agent_lines compact overflow + exact cap (C07 L65) ---
    #[test]
    fn test_agent_lines_compact_shows_extra_over_two() {
        let agents: Vec<DetectedAgentWatch> =
            (0..3).map(|i| agent_watch(100 + i, "claude", "claude", 1_048_576, None)).collect();
        let lines = agent_lines(&agents, &HashMap::new(), true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("+1"), "3 agents compact MUST show +1; got: {text}");
    }

    #[test]
    fn test_agent_lines_compact_exactly_two_no_extra() {
        let agents: Vec<DetectedAgentWatch> =
            (0..2).map(|i| agent_watch(100 + i, "claude", "claude", 1_048_576, None)).collect();
        let lines = agent_lines(&agents, &HashMap::new(), true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(!text.contains('+'), "2 agents compact MUST NOT add an extra; got: {text}");
    }

    #[test]
    fn test_agent_lines_full_exactly_cap_no_truncation() {
        // MAX_AGENT_LINES == 4: exact cap MUST NOT render "+N more".
        let agents: Vec<DetectedAgentWatch> =
            (0..4).map(|i| agent_watch(100 + i, "claude", "claude", 1_048_576, None)).collect();
        let lines = agent_lines(&agents, &HashMap::new(), false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(!text.contains("more"), "4 agents MUST NOT truncate; got: {text}");
    }

    // --- tree-line formatters (C07 L65) ---
    fn tree_node(
        pid: u32,
        comm: &str,
        family: Option<&'static str>,
        children: Vec<AgentTreeNode>,
    ) -> AgentTreeNode {
        AgentTreeNode { pid, ppid: 0, comm: comm.into(), family, children }
    }

    #[test]
    fn test_format_tree_node_line_full_annotation() {
        let node = tree_node(100, "claude", Some("claude"), vec![]);
        let rss: HashMap<u32, u64> = HashMap::from([(100, 52_428_800)]);
        let state: HashMap<u32, char> = HashMap::from([(100, 'S')]);
        // family carries a trailing space and rss a leading one, matching the
        // existing render (family + rss + comm suffix concatenated).
        assert_eq!(format_tree_node_line(&node, &rss, &state), "[100] S claude  RSS 50M (claude)");
    }

    #[test]
    fn test_format_tree_node_sparse_fields() {
        // No family, no RSS entry, empty comm -> no extra tokens, `-` state.
        let node = tree_node(200, "", None, vec![]);
        assert_eq!(format_tree_node_line(&node, &HashMap::new(), &HashMap::new()), "[200] - ");
    }

    #[test]
    fn test_comm_suffix_empty_vs_present() {
        assert_eq!(format_comm_suffix(""), "");
        assert_eq!(format_comm_suffix("claude"), " (claude)");
    }

    // --- append_agent_tree_lines budget + connectors (C07 L65) ---
    #[test]
    fn test_append_agent_tree_lines_zero_budget_noop() {
        let node = tree_node(100, "claude", Some("claude"), vec![]);
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut budget = 0usize;
        append_agent_tree_lines(
            &node,
            "",
            true,
            &HashMap::new(),
            &HashMap::new(),
            &mut lines,
            &mut budget,
        );
        assert!(lines.is_empty(), "zero budget MUST append nothing");
    }

    #[test]
    fn test_append_agent_tree_lines_budget_exhausts_after_root() {
        let node = tree_node(
            100,
            "claude",
            Some("claude"),
            vec![tree_node(101, "sh", None, vec![]), tree_node(102, "bash", None, vec![])],
        );
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut budget = 1usize;
        append_agent_tree_lines(
            &node,
            "",
            true,
            &HashMap::new(),
            &HashMap::new(),
            &mut lines,
            &mut budget,
        );
        assert_eq!(lines.len(), 1, "budget 1 MUST stop after the root; got {lines:?}");
    }

    #[test]
    fn test_append_agent_tree_lines_last_child_uses_elbow() {
        let node = tree_node(
            100,
            "claude",
            Some("claude"),
            vec![tree_node(101, "sh", None, vec![]), tree_node(102, "bash", None, vec![])],
        );
        let mut lines: Vec<Line<'static>> = Vec::new();
        let mut budget = 3usize;
        append_agent_tree_lines(
            &node,
            "",
            true,
            &HashMap::new(),
            &HashMap::new(),
            &mut lines,
            &mut budget,
        );
        assert_eq!(lines.len(), 3, "root + two children MUST render; got {lines:?}");
        // Per-line connector assertions: a bare `contains("└── ")` cannot
        // distinguish an is_last inversion (mutant swaps which child gets the
        // elbow), so pin the connectors by line position.
        assert!(
            lines[1].to_string().contains("├── "),
            "first child MUST use the tee connector; got: {}",
            lines[1]
        );
        assert!(
            lines[2].to_string().contains("└── "),
            "last child MUST use the elbow connector; got: {}",
            lines[2]
        );
    }

    // --- agent_forest_lines budget / root overflow (C07 L65) ---
    #[test]
    fn test_agent_forest_lines_reports_extra_roots() {
        let forests: Vec<AgentTreeNode> =
            (0..5).map(|i| tree_node(100 + i, "claude", Some("claude"), vec![])).collect();
        let lines = agent_forest_lines(&forests, &[], &HashMap::new(), false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("+1 more roots"), "5 roots MUST report +1 more; got: {text}");
    }

    #[test]
    fn test_agent_forest_lines_exact_budget_no_overflow_line() {
        // MAX_AGENT_TREE_LINES == 4 single-node roots: no "+N more roots".
        let forests: Vec<AgentTreeNode> =
            (0..4).map(|i| tree_node(100 + i, "claude", Some("claude"), vec![])).collect();
        let lines = agent_forest_lines(&forests, &[], &HashMap::new(), false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(!text.contains("more roots"), "4 roots MUST NOT report more roots; got: {text}");
    }

    #[test]
    fn test_agent_forest_lines_empty_falls_back_to_agent_lines() {
        let lines = agent_forest_lines(&[], &[], &HashMap::new(), false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("No agent processes detected"), "empty MUST fall back; got: {text}");
    }

    // --- App::poll_operator_meters / apply_pool_status_panels (C07 L65) ---
    #[test]
    fn test_poll_operator_meters_clears_pinned_forest_state() {
        let mut app = App::new(4);
        app.forest_state_by_pid = Some(HashMap::from([(1, 'S')]));
        app.poll_operator_meters();
        assert!(
            app.forest_state_by_pid.is_none(),
            "poll MUST clear the pinned forest-state test override"
        );
    }

    #[test]
    fn test_apply_pool_status_panels_updates_both_panels() {
        let mut app = App::new(4);
        let pool = PoolOperatorPanel {
            node_total: 2,
            node_idle: 1,
            bun_total: 1,
            bun_idle: 0,
            max_per_type: 4,
            healthy: true,
        };
        let status =
            StatusOperatorPanel { scanned: 50, watched: 1, total_processes: 2, agent_rows: 1 };
        app.apply_pool_status_panels(Some(pool), Some(status));
        assert_eq!(app.pool_panel, Some(pool));
        assert_eq!(app.status_panel, Some(status));
    }

    // --- App::agent_panel_state_by_pid precedence (C07 L65) ---
    #[test]
    fn test_agent_panel_state_by_pid_pinned_wins() {
        let mut app = App::new(4);
        let pinned: HashMap<u32, char> = HashMap::from([(42, 'S')]);
        app.forest_state_by_pid = Some(pinned.clone());
        assert_eq!(app.agent_panel_state_by_pid(), pinned);
    }

    /// Forests take precedence over the flat agent list: with a forest
    /// present, the map is derived from the forest pids, not the flat
    /// inventory. Linux-gated because the host proc source reads /proc.
    #[cfg(target_os = "linux")]
    #[test]
    fn test_agent_panel_state_by_pid_forests_take_precedence() {
        let mut app = App::new(4);
        app.agent_forests = vec![tree_node(999_999, "fake-root", None, vec![])];
        app.detected_agents = vec![agent_watch(std::process::id(), "claude", "claude", 0, None)];
        let map = app.agent_panel_state_by_pid();
        assert!(
            !map.contains_key(&std::process::id()),
            "forest path MUST win over the flat agent list"
        );
    }

    // --- poll_pool_status_panels (C07 L65) ---
    #[test]
    fn test_poll_pool_status_panels_applies_snapshot() {
        let mut app = App::new(4);
        let pool = PoolOperatorPanel {
            node_total: 4,
            node_idle: 3,
            bun_total: 2,
            bun_idle: 1,
            max_per_type: 8,
            healthy: false,
        };
        let status =
            StatusOperatorPanel { scanned: 10, watched: 2, total_processes: 3, agent_rows: 2 };
        let mut poll: Box<PoolStatusPollFn> = Box::new(move || (Some(pool), Some(status)));
        poll_pool_status_panels(&mut app, Some(poll.as_mut()));
        assert!(app.pool_panel.is_some(), "poll MUST apply the pool snapshot");
        assert!(app.status_panel.is_some(), "poll MUST apply the status snapshot");
    }

    // --- Additional edge-case tests ---

    #[test]
    fn test_slot_ratio_zero_active_zero_cap() {
        assert_eq!(slot_ratio(0, 0), 0.0);
    }

    #[test]
    fn test_slot_ratio_active_with_zero_cap() {
        assert_eq!(slot_ratio(5, 0), 0.0);
    }

    #[test]
    fn test_slot_ratio_normal() {
        assert_eq!(slot_ratio(1, 4), 0.25);
        assert_eq!(slot_ratio(2, 4), 0.5);
        assert_eq!(slot_ratio(4, 4), 1.0);
    }

    #[test]
    fn test_slot_ratio_clamps() {
        assert_eq!(slot_ratio(100, 4), 1.0, "active > cap MUST clamp to 1.0");
    }

    #[test]
    fn test_slot_color_green() {
        assert_eq!(slot_color(1, 4), Color::Green, "< 50% MUST be green");
    }

    #[test]
    fn test_slot_color_yellow() {
        assert_eq!(slot_color(2, 4), Color::Yellow, "50-90% MUST be yellow");
        assert_eq!(slot_color(3, 4), Color::Yellow);
    }

    #[test]
    fn test_slot_color_red() {
        assert_eq!(slot_color(4, 4), Color::Red, ">= 90% MUST be red");
        assert_eq!(slot_color(40, 42), Color::Red);
    }

    #[test]
    fn test_host_agent_lines_empty() {
        let lines = host_agent_lines(&[], false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("none detected"));
    }

    #[test]
    fn test_host_agent_lines_compact_empty() {
        let lines = host_agent_lines(&[], true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("none"));
    }

    #[test]
    fn test_host_agent_lines_with_agents() {
        let agents = vec![
            DetectedAgent { pid: 100, family: "claude", comm: "claude".into() },
            DetectedAgent { pid: 200, family: "forge", comm: "forge".into() },
        ];
        let lines = host_agent_lines(&agents, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("claude"));
        assert!(text.contains("forge"));
        assert!(text.contains("Host agents:"));
    }

    #[test]
    fn test_host_agent_lines_compact_with_agents() {
        let agents = vec![DetectedAgent { pid: 100, family: "claude", comm: "claude".into() }];
        let lines = host_agent_lines(&agents, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("claude:100"));
    }

    #[test]
    fn test_host_agent_lines_truncates_at_four() {
        let agents: Vec<_> = (0..6)
            .map(|i| DetectedAgent { pid: 100 + i, family: "test", comm: format!("test-{i}") })
            .collect();
        let lines = host_agent_lines(&agents, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("+2 more"), "MUST truncate at 4 and show overflow count");
    }

    #[test]
    fn test_fuse_neg_dentry_lines_full() {
        let meters = NegDentryMeters { hits: 80, misses: 20 };
        let lines = fuse_neg_dentry_lines(meters, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Neg hits:     80"));
        assert!(text.contains("Neg misses:   20"));
        assert!(text.contains("Hit rate:     80%"));
    }

    #[test]
    fn test_fuse_neg_dentry_lines_compact() {
        let meters = NegDentryMeters { hits: 80, misses: 20 };
        let lines = fuse_neg_dentry_lines(meters, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("neg:80"));
        assert!(text.contains("miss:20"));
    }

    #[test]
    fn test_hypervisor_coalesce_lines_full() {
        let meters = CoalesceMeters { hits: 90, misses: 10, nocache_runs: 5 };
        let lines = hypervisor_coalesce_lines(meters, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Coalesce hits:   90"));
        assert!(text.contains("Coalesce misses: 10"));
        assert!(text.contains("Nocache runs:    5"));
        assert!(text.contains("Hit rate:        90%"));
    }

    #[test]
    fn test_hypervisor_coalesce_lines_compact() {
        let meters = CoalesceMeters { hits: 90, misses: 10, nocache_runs: 5 };
        let lines = hypervisor_coalesce_lines(meters, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("coalesce:90"));
        assert!(text.contains("miss:10"));
        assert!(text.contains("nocache:5"));
    }

    #[test]
    fn test_hypervisor_slot_queue_lines_full() {
        let meters = SlotQueueMeters { acquires: 100, waits: 5, timeouts: 2 };
        let lines = hypervisor_slot_queue_lines(meters, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Slot acquires: 100"));
        assert!(text.contains("Slot waits:    5"));
        assert!(text.contains("Slot timeouts: 2"));
    }

    #[test]
    fn test_hypervisor_slot_queue_lines_compact() {
        let meters = SlotQueueMeters { acquires: 100, waits: 5, timeouts: 2 };
        let lines = hypervisor_slot_queue_lines(meters, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("slot:100"));
        assert!(text.contains("wait:5"));
        assert!(text.contains("to:2"));
    }

    #[test]
    fn test_fuse_write_serialize_lines_full() {
        let meters =
            WriteSerializeMeters { passthrough_writes: 50, stages: 30, commits: 25, discards: 5 };
        let lines = fuse_write_serialize_lines(meters, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Passthrough:  50"));
        assert!(text.contains("Stages:       30"));
        assert!(text.contains("Commits:      25"));
        assert!(text.contains("Discards:     5"));
    }

    #[test]
    fn test_fuse_write_serialize_lines_compact() {
        let meters =
            WriteSerializeMeters { passthrough_writes: 50, stages: 30, commits: 25, discards: 5 };
        let lines = fuse_write_serialize_lines(meters, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("wr:50"));
        assert!(text.contains("st:30"));
        assert!(text.contains("cm:25"));
        assert!(text.contains("ds:5"));
    }

    #[test]
    fn test_fuse_coalesce_lines_full_100pct() {
        let meters = ReadCacheMeters { hits: 100, misses: 0 };
        let lines = fuse_coalesce_lines(meters, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Cache hits:   100"));
        assert!(text.contains("Cache misses: 0"));
        assert!(text.contains("Hit rate:     100%"));
    }

    #[test]
    fn test_fuse_coalesce_lines_compact() {
        let meters = ReadCacheMeters { hits: 75, misses: 25 };
        let lines = fuse_coalesce_lines(meters, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("hits:75"));
        assert!(text.contains("miss:25"));
    }

    #[test]
    fn test_mesh_maildir_lines_none() {
        let lines = mesh_maildir_lines(None, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("unavailable"));
    }

    #[test]
    fn test_mesh_maildir_lines_some_full() {
        let status = MaildirStatus {
            path: std::path::PathBuf::from("/tmp/maildir"),
            ready: 5,
            in_flight: 2,
            pending: 3,
        };
        let lines = mesh_maildir_lines(Some(status), false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Mesh ready:     5"));
        assert!(text.contains("Mesh in-flight: 2"));
        assert!(text.contains("Mesh pending:   3"));
    }

    #[test]
    fn test_mesh_maildir_lines_some_compact() {
        let status = MaildirStatus {
            path: std::path::PathBuf::from("/tmp/maildir"),
            ready: 5,
            in_flight: 2,
            pending: 3,
        };
        let lines = mesh_maildir_lines(Some(status), true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("mesh r:5 f:2 p:3"));
    }

    #[test]
    fn test_resource_watch_lines_none_full() {
        let lines = resource_watch_lines(None, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("unavailable"));
    }

    #[test]
    fn test_resource_watch_lines_none_compact() {
        let lines = resource_watch_lines(None, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("unavailable"));
    }

    #[test]
    fn test_resource_watch_lines_some_full() {
        let sample = ResourceWatchSample {
            fd_count: 100,
            net_rx_bytes: 5000,
            net_tx_bytes: 3000,
            mem_rss_bytes: 2048,
            load_1m: 0.5,
        };
        let lines = resource_watch_lines(Some(sample), false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Open FDs:  100"));
        assert!(text.contains("RSS:       2048 bytes"));
        assert!(text.contains("Load (1m): 0.50"));
    }

    #[test]
    fn test_resource_watch_lines_some_compact() {
        let sample = ResourceWatchSample {
            fd_count: 100,
            net_rx_bytes: 5000,
            net_tx_bytes: 3000,
            mem_rss_bytes: 2048,
            load_1m: 0.5,
        };
        let lines = resource_watch_lines(Some(sample), true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("FD:100"));
        assert!(text.contains("RSS:2048"));
    }

    #[test]
    fn test_pool_panel_lines_none_full() {
        let lines = pool_panel_lines(None, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Runtime pool unavailable"));
    }

    #[test]
    fn test_pool_panel_lines_none_compact() {
        let lines = pool_panel_lines(None, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("pool unavailable"));
    }

    #[test]
    fn test_status_panel_lines_none_full() {
        let lines = status_panel_lines(None, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Proc scan status unavailable"));
    }

    #[test]
    fn test_status_panel_lines_none_compact() {
        let lines = status_panel_lines(None, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("status unavailable"));
    }

    #[test]
    fn test_apply_key_action_focus_next() {
        let mut app = App::new(4);
        apply_key_action(&mut app, KeyAction::FocusNext);
        assert_eq!(app.focus, PanelFocus::Pool);
    }

    #[test]
    fn test_apply_key_action_focus_prev() {
        let mut app = App::new(4);
        apply_key_action(&mut app, KeyAction::FocusPrev);
        assert_eq!(app.focus, PanelFocus::Agents);
    }

    #[test]
    fn test_apply_key_action_focus_panel() {
        let mut app = App::new(4);
        apply_key_action(&mut app, KeyAction::FocusPanel(PanelFocus::Status));
        assert_eq!(app.focus, PanelFocus::Status);
    }

    #[test]
    fn test_app_new_defaults() {
        let app = App::new(8);
        assert_eq!(app.thermal_level, ThermalLevel::Green);
        assert_eq!(app.active_slots, 0);
        assert_eq!(app.slot_cap, 8);
        assert_eq!(app.poll_count, 0);
        assert!(!app.show_help_overlay);
        assert_eq!(app.focus, PanelFocus::Gate);
    }

    #[test]
    fn test_app_update() {
        let mut app = App::new(4);
        app.update(ThermalLevel::Red, 3);
        assert_eq!(app.thermal_level, ThermalLevel::Red);
        assert_eq!(app.active_slots, 3);
        assert_eq!(app.poll_count, 1);
    }

    #[test]
    fn test_panel_focus_from_digit() {
        assert_eq!(PanelFocus::from_digit('1'), Some(PanelFocus::Gate));
        assert_eq!(PanelFocus::from_digit('2'), Some(PanelFocus::Pool));
        assert_eq!(PanelFocus::from_digit('3'), Some(PanelFocus::Status));
        assert_eq!(PanelFocus::from_digit('4'), Some(PanelFocus::HostWatch));
        assert_eq!(PanelFocus::from_digit('5'), Some(PanelFocus::Agents));
        assert_eq!(PanelFocus::from_digit('0'), None);
        assert_eq!(PanelFocus::from_digit('6'), None);
        assert_eq!(PanelFocus::from_digit('a'), None);
    }

    #[test]
    fn test_agent_lines_empty_full_message() {
        let lines = agent_lines(&[], &std::collections::HashMap::new(), false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("No agent processes detected"));
    }

    #[test]
    fn test_agent_lines_empty_compact() {
        let lines = agent_lines(&[], &std::collections::HashMap::new(), true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("none"));
    }

    #[test]
    fn test_agent_lines_with_agents_full() {
        let agents = vec![DetectedAgentWatch {
            agent: DetectedAgent { pid: 100, family: "claude", comm: "claude".into() },
            resource: AgentResourceSample { mem_rss_bytes: 1_000_000, fd_count: Some(10) },
        }];
        let state = HashMap::from([(100, 'S')]);
        let lines = agent_lines(&agents, &state, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("PID 100"));
        assert!(text.contains("claude"));
        assert!(text.contains("FD 10"));
        assert!(text.contains("Agents: 1"));
    }

    #[test]
    fn test_agent_lines_with_agents_compact() {
        let agents = vec![DetectedAgentWatch {
            agent: DetectedAgent { pid: 100, family: "claude", comm: "claude".into() },
            resource: AgentResourceSample { mem_rss_bytes: 1_000_000, fd_count: None },
        }];
        let state = HashMap::new();
        let lines = agent_lines(&agents, &state, true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("claude:100"));
    }

    #[test]
    fn test_agent_lines_truncates_at_max() {
        let agents: Vec<_> = (0..6)
            .map(|i| DetectedAgentWatch {
                agent: DetectedAgent { pid: 100 + i, family: "test", comm: format!("test-{i}") },
                resource: AgentResourceSample { mem_rss_bytes: 1000, fd_count: None },
            })
            .collect();
        let state = HashMap::new();
        let lines = agent_lines(&agents, &state, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("+2 more"), "MUST truncate at MAX_AGENT_LINES");
    }

    #[test]
    fn test_agent_forest_lines_empty_falls_back() {
        let lines = agent_forest_lines(&[], &[], &HashMap::new(), false);
        assert!(lines.iter().map(|l| l.to_string()).collect::<String>().contains("No agent"));
    }

    #[test]
    fn test_agent_forest_lines_compact_falls_back_to_agent_lines() {
        let lines = agent_forest_lines(&[], &[], &HashMap::new(), true);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("none"));
    }

    #[test]
    fn test_agent_forest_lines_with_forests() {
        let forest = AgentTreeNode {
            pid: 100,
            ppid: 1,
            comm: "claude".into(),
            family: Some("claude"),
            children: vec![AgentTreeNode {
                pid: 200,
                ppid: 100,
                comm: "bash".into(),
                family: None,
                children: vec![],
            }],
        };
        let watched = vec![DetectedAgentWatch {
            agent: DetectedAgent { pid: 100, family: "claude", comm: "claude".into() },
            resource: AgentResourceSample { mem_rss_bytes: 500_000, fd_count: None },
        }];
        let state = HashMap::from([(100, 'S'), (200, 'R')]);
        let lines = agent_forest_lines(&[forest], &watched, &state, false);
        let text: String = lines.iter().map(|l| l.to_string()).collect();
        assert!(text.contains("Forests: 1"));
        assert!(text.contains("100"));
        assert!(text.contains("claude"));
        assert!(text.contains("200"));
    }

    // --- proptest (C07 L66) ---
    proptest::proptest! {
        #![proptest_config(proptest::test_runner::Config {
            failure_persistence: Some(Box::new(
                proptest::test_runner::FileFailurePersistence::SourceParallel("proptest-regressions"),
            )),
            ..proptest::test_runner::Config::default()
        })]

        #[test]
        fn prop_slot_ratio_bounded(active in 0u32..10_000u32, cap in 0u32..10_000u32) {
            let r = slot_ratio(active, cap);
            prop_assert!((0.0..=1.0).contains(&r));
            if cap == 0 {
                prop_assert_eq!(r, 0.0);
            } else if active >= cap {
                prop_assert!((r - 1.0).abs() < 1e-12);
            }
        }

        #[test]
        fn prop_is_compact_threshold(width in 0u16..500u16) {
            prop_assert_eq!(is_compact(width), width < COMPACT_WIDTH);
        }

        #[test]
        fn prop_gate_decision_red_only_denies(level in prop_thermal_level()) {
            let decision = gate_decision(level);
            if level == ThermalLevel::Red {
                prop_assert_eq!(decision, "DENY");
            } else {
                prop_assert_eq!(decision, "ADMIT");
            }
        }
    }

    fn prop_thermal_level() -> impl proptest::strategy::Strategy<Value = ThermalLevel> {
        proptest::sample::select(vec![ThermalLevel::Green, ThermalLevel::Yellow, ThermalLevel::Red])
    }
}
