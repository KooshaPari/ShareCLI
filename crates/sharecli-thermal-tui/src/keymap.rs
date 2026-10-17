//! Keyboard input layer for the thermal TUI (PanelFocus, KeyAction, handle_key, apply_key_action).
//!
//! Splits the input surface from application state so the keymap can be unit-tested
//! without touching `App` (the binding matrix is pure; only `apply_key_action`
//! mutates live state).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Footer help overlay copy (shown when `?` toggles help).
pub const HELP_OVERLAY_HINT: &str =
    " Tab/Shift-Tab cycle  1 gate  2 pool  3 status  4 watch  5 agents  r poll  ? hide";

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
pub fn apply_key_action(app: &mut crate::app::App, action: KeyAction) {
    match action {
        KeyAction::FocusNext => app.focus = app.focus.next(),
        KeyAction::FocusPrev => app.focus = app.focus.prev(),
        KeyAction::FocusPanel(panel) => app.focus = panel,
        KeyAction::ToggleHelp => app.show_help_overlay = !app.show_help_overlay,
        KeyAction::Quit | KeyAction::ForcePoll | KeyAction::Noop => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crossterm::event::{KeyEventKind, KeyEventState};

    fn key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    // --- is_quit_key ---
    #[test]
    fn test_is_quit_key_q() {
        assert!(is_quit_key(&key_event(KeyCode::Char('q'), KeyModifiers::NONE)));
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

    // --- PanelFocus ordering (C07 L65) ---
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
}
