//! TUI event loop and entry points (run / run_with_pool_status).
//!
//! The loop polls both input and the [`ThermalGovernor`] at a configurable
//! interval; the [`PoolStatusPollFn`] hook is wired in for AC-007.71 (CLI/IPC
//! can push fresh operator-panel snapshots into the running TUI).

use anyhow::Result;
use crossterm::event::{self, Event};
use sharecli_fleet::thermal::{ThermalGovernor, ThermalLevel};

use sharecli_fleet::{PoolOperatorPanel, StatusOperatorPanel};

use crate::app::{count_cargo_builds, App};
use crate::keymap::{apply_key_action, handle_key, KeyAction};
use crate::render::render;

/// Supplementary pool/status poll hook (CLI/IPC-shaped snapshots for AC-007.71).
pub type PoolStatusPollFn = dyn FnMut() -> (Option<PoolOperatorPanel>, Option<StatusOperatorPanel>);

fn poll_pool_status_panels(
    app: &mut App,
    poll_pool_status: Option<&mut PoolStatusPollFn>,
) {
    if let Some(poll) = poll_pool_status {
        let (pool, status) = poll();
        app.apply_pool_status_panels(pool, status);
    }
}

/// Launch the TUI, polling `governor` every [`POLL_INTERVAL`](crate::app::POLL_INTERVAL).
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
        if event::poll(crate::app::POLL_INTERVAL)? {
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
