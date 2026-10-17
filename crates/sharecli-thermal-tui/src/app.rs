//! Live application state for the thermal TUI.

use std::time::{Duration, Instant};

use std::collections::HashMap;

use sharecli_fleet::{
    build_host_agent_forests, build_host_agent_state_map, build_host_forest_state_map,
    global_coalesce_meters, global_slot_queue_meters, watch_host_agents, AgentTreeNode,
    CoalesceMeters, DetectedAgentWatch, ResourceWatchSample, SlotQueueMeters,
};
use sharecli_fleet::operator_pool_status::{PoolOperatorPanel, StatusOperatorPanel};
use sharecli_fleet::thermal::ThermalLevel;
use sharecli_fuse::{
    global_neg_dentry_meters, global_read_cache_meters, global_write_serialize_meters,
    NegDentryMeters, ReadCacheMeters, WriteSerializeMeters,
};
use sharecli_mesh::{capture_maildir_status, MaildirStatus};

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
    pub(crate) forest_state_by_pid: Option<HashMap<u32, char>>,
    /// Runtime pool operator panel (FR-007 / AC-007.71).
    pub pool_panel: Option<PoolOperatorPanel>,
    /// Proc-scan / managed-process operator panel (FR-007 / AC-007.71).
    pub status_panel: Option<StatusOperatorPanel>,
    /// Focused operator panel for keyboard navigation (C09 L81.3).
    pub focus: crate::keymap::PanelFocus,
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
            focus: crate::keymap::PanelFocus::default(),
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

    /// Resolve per-PID state letters for the agent panel.
    ///
    /// Precedence:
    /// 1. Test-pinned `forest_state_by_pid` (overrides everything)
    /// 2. Forest scan from live proc read when `agent_forests` is non-empty
    /// 3. Flat per-PID state from the proc scan fallback
    pub(crate) fn agent_panel_state_by_pid(&self) -> HashMap<u32, char> {
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

#[cfg(test)]
mod tests {
    use super::*;

    use crate::keymap::{apply_key_action, KeyAction, PanelFocus};

    // --- App::new defaults ---
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

    // --- App::poll_operator_meters / apply_pool_status_panels ---
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
        let status = StatusOperatorPanel {
            scanned: 50,
            watched: 1,
            total_processes: 2,
            agent_rows: 1,
        };
        app.apply_pool_status_panels(Some(pool), Some(status));
        assert_eq!(app.pool_panel, Some(pool));
        assert_eq!(app.status_panel, Some(status));
    }

    // --- App::agent_panel_state_by_pid precedence ---
    #[test]
    fn test_agent_panel_state_by_pid_pinned_wins() {
        let mut app = App::new(4);
        let pinned: HashMap<u32, char> = HashMap::from([(42, 'S')]);
        app.forest_state_by_pid = Some(pinned.clone());
        assert_eq!(app.agent_panel_state_by_pid(), pinned);
    }

    // --- apply_key_action (mutates App) ---
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
}
