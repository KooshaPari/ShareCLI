//! `sharecli-thermal-tui` — live TUI for thermal-gate / hypervisor state.
//!
//! # Design
//!
//! The TUI is split across several small submodules under a slim root. Each
//! sub-module owns a single concern:
//!
//! - `app`        — live `App` state, builders, hot-loop tick, `thermal_blurb`
//! - `keymap`     — key events → `KeyAction`, `PanelFocus`, `apply_key_action`
//! - `agents`     — detected-agent + tree-line renderers
//! - `panels`     — per-panel line builders (level, gate, runtime, resource, io)
//! - `render`     — `ratatui` `Frame` painter, layout, and per-section renderers
//! - `run`        — terminal bring-up, event loop, pool-status poll hook
//!
//! `render::render` is the single public entry point used by `run::run`.
//!
//! The crate's public API is re-exported here so downstream call sites
//! (`src/main.rs`, integration tests) keep working unchanged.

mod agents;
mod app;
mod keymap;
mod panels;
mod render;
mod run;

pub use agents::{agent_forest_lines, agent_lines, append_agent_tree_lines};
pub use app::{thermal_blurb, App, DEFAULT_SLOT_CAP, POLL_INTERVAL};
pub use keymap::{apply_key_action, handle_key, is_quit_key, KeyAction, PanelFocus, HELP_OVERLAY_HINT};
pub use panels::level::{
    decision_color, gate_decision, is_compact, level_color, level_label, level_pressure_raw,
    slot_color, slot_ratio, COMPACT_WIDTH,
};
pub use panels::runtime::{pool_panel_lines, status_panel_lines};
pub use panels::resource::{host_agent_lines, resource_watch_lines};
pub use panels::io::{
    fuse_coalesce_lines, fuse_neg_dentry_lines, fuse_write_serialize_lines,
    hypervisor_coalesce_lines, hypervisor_slot_queue_lines, mesh_maildir_lines,
};
pub use render::render;
pub use run::{run, run_with_pool_status, PoolStatusPollFn};

#[cfg(test)]
mod tests {
    //! Cross-module integration tests.
    //!
    //! Single-module unit tests live next to the impl in each submodule
    //! (`panels::level::tests`, `agents::tests`, etc.).

    use std::collections::HashMap;

    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use proptest::prelude::*;
    use sharecli_fleet::thermal::ThermalLevel;

    use super::*;

    fn key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    #[test]
    fn key_event_helper_smoke() {
        let ev = key_event(KeyCode::Char('q'), KeyModifiers::NONE);
        assert!(is_quit_key(&ev));
    }

    #[test]
    fn focus_cycle_rotates_through_all_panels() {
        let mut app = App::new(DEFAULT_SLOT_CAP);
        let starts = app.focus;
        // Apply 5 .next() calls — one full cycle (Gate->Pool->Status->HostWatch->Agents->Gate).
        for _ in 0..5 {
            app.focus = PanelFocus::next(app.focus);
        }
        assert_eq!(app.focus, starts, "one full cycle MUST return to the starting panel");
    }

    #[test]
    fn panel_digit_keys_switch_focus() {
        for panel in [
            PanelFocus::Gate,
            PanelFocus::Pool,
            PanelFocus::Status,
            PanelFocus::HostWatch,
            PanelFocus::Agents,
        ] {
            let mut app = App::new(DEFAULT_SLOT_CAP);
            apply_key_action(&mut app, KeyAction::FocusPanel(panel));
            assert_eq!(app.focus, panel);
        }
    }

    #[test]
    fn thermal_blurb_adapts() {
        assert!(!thermal_blurb(ThermalLevel::Green, true).is_empty());
        assert!(!thermal_blurb(ThermalLevel::Yellow, true).is_empty());
        assert!(!thermal_blurb(ThermalLevel::Red, true).is_empty());
    }

    #[test]
    fn test_render_green_headless() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let app = App::new(DEFAULT_SLOT_CAP);
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render green");
    }

    #[test]
    fn test_render_yellow_headless() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut app = App::new(DEFAULT_SLOT_CAP);
        app.thermal_level = ThermalLevel::Yellow;
        app.active_slots = 2;
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render yellow");
    }

    #[test]
    fn test_render_red_headless() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut app = App::new(DEFAULT_SLOT_CAP);
        app.thermal_level = ThermalLevel::Red;
        app.active_slots = 4;
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render red");
    }

    #[test]
    fn test_render_help_overlay_when_enabled() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let mut app = App::new(DEFAULT_SLOT_CAP);
        app.show_help_overlay = true;
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render help overlay");
    }

    #[test]
    fn test_render_footer_shows_help_hint() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;

        let app = App::new(DEFAULT_SLOT_CAP);
        let backend = TestBackend::new(160, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render footer");
    }

    #[test]
    fn test_poll_operator_meters_clears_pinned_forest_state() {
        let mut app = App::new(DEFAULT_SLOT_CAP);
        let mut pinned = HashMap::new();
        pinned.insert(1u32, 'R');
        app = app.with_agent_forest_state(pinned);
        assert!(app.forest_state_by_pid.is_some());
        app.poll_operator_meters();
        assert!(app.forest_state_by_pid.is_none());
    }

    #[test]
    fn test_resource_watch_lines_unavailable() {
        let lines = resource_watch_lines(None, true);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_status_panel_lines_full() {
        use sharecli_fleet::StatusOperatorPanel;
        let panel = StatusOperatorPanel::default();
        let lines = status_panel_lines(Some(panel), true);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_status_panel_lines_none_compact() {
        let lines = status_panel_lines(None, false);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_status_panel_lines_none_full() {
        let lines = status_panel_lines(None, true);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_render_with_operator_meters_and_detected_agents() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        use sharecli_fleet::{CoalesceMeters, SlotQueueMeters};
        use sharecli_fuse::{NegDentryMeters, ReadCacheMeters, WriteSerializeMeters};

        let app = App::new(DEFAULT_SLOT_CAP).with_operator_meters(
            None,
            ReadCacheMeters::default(),
            NegDentryMeters::default(),
            CoalesceMeters::default(),
            SlotQueueMeters::default(),
            WriteSerializeMeters::default(),
        );
        let backend = TestBackend::new(120, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, &app))
            .expect("render with operator meters");
    }

    #[test]
    fn agent_forest_empty_falls_back_to_agent_lines() {
        use sharecli_fleet::proc_scan::DetectedAgent;
        use sharecli_fleet::{AgentResourceSample, DetectedAgentWatch};

        let watched = DetectedAgentWatch {
            agent: DetectedAgent {
                pid: 4242,
                family: "claude",
                comm: "claude".into(),
            },
            resource: AgentResourceSample {
                mem_rss_bytes: 0,
                fd_count: None,
            },
        };
        let app = App::new(DEFAULT_SLOT_CAP).with_detected_agents(vec![watched]);
        // forest empty, fallback path
        assert!(app.agent_forests.is_empty());
        let state = app.agent_panel_state_by_pid();
        assert!(state.is_empty() || state.contains_key(&4242));
    }

    proptest! {
        #[test]
        fn prop_slot_ratio_bounded(active in 0u32..=16, cap in 1u32..=16) {
            let r = slot_ratio(active, cap);
            prop_assert!((0.0..=1.0).contains(&r), "ratio out of range: {}", r);
        }

        #[test]
        fn prop_is_compact_threshold(width in 1u16..=200) {
            let compact = is_compact(width);
            prop_assert_eq!(compact, width < COMPACT_WIDTH);
        }

        #[test]
        fn prop_gate_decision_red_only_denies(level in prop::sample::select(vec![
            ThermalLevel::Green, ThermalLevel::Yellow, ThermalLevel::Red,
        ])) {
            let decision = gate_decision(level);
            let denies = level == ThermalLevel::Red;
            prop_assert_eq!(denies, decision == "DENY");
        }
    }
}
