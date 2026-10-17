//! Pure rendering: takes `&mut Frame` + `&App`, paints the full TUI.
//!
//! `render` is the only public entry point; helper `render_*` functions are
//! kept crate-private to make the layout easy to reason about.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
    Frame,
};

use sharecli_fleet::sum_detected_agent_rss_bytes;

use crate::app::{thermal_blurb, App, POLL_INTERVAL};
use crate::keymap::PanelFocus;
use crate::panels::{
    gate_panel_lines, host_agent_lines, pool_panel_lines, resource_watch_lines,
    status_panel_lines,
};
use crate::panels::io::{
    fuse_coalesce_lines, fuse_neg_dentry_lines, fuse_write_serialize_lines,
    hypervisor_coalesce_lines, hypervisor_slot_queue_lines, mesh_maildir_lines,
};
use crate::panels::level::{level_color, level_label, level_pressure_raw, slot_color, slot_ratio};
use crate::agents::{agent_forest_lines};
use crate::keymap::HELP_OVERLAY_HINT;

/// Render the full TUI into `frame`.
///
/// Layout adapts to `frame.area().width` (backed by terminal size / `COLUMNS`).
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let compact = crate::panels::is_compact(area.width);
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
    let snap = sharecli_fleet::gate_status_snapshot_with_rss(level, agent_count, total_rss);
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

pub(crate) fn render_agents(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    let title = if compact { " Agents " } else { " Detected Agents " };
    let state_by_pid = app.agent_panel_state_by_pid();
    let lines = agent_forest_lines(&app.agent_forests, &app.detected_agents, &state_by_pid, compact);
    let block = focused_block(title, app.focus == PanelFocus::Agents);
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn render_resource_watch(frame: &mut Frame, area: Rect, app: &App, compact: bool) {
    let title = if compact { " Watch " } else { " Host Resource Watch " };
    // Compose the resource watch panel with the host-agent inventory under it.
    let mut lines = resource_watch_lines(app.resource_watch, compact);
    // Project DetectedAgentWatch → DetectedAgent for the host_agent_lines consumer.
    let agents: Vec<sharecli_fleet::proc_scan::DetectedAgent> = app
        .detected_agents
        .iter()
        .map(|w| sharecli_fleet::proc_scan::DetectedAgent {
            pid: w.agent.pid,
            family: w.agent.family,
            comm: w.agent.comm.clone(),
        })
        .collect();
    lines.extend(host_agent_lines(&agents, compact));
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    use crate::app::App;
    use sharecli_fleet::thermal::ThermalLevel;

    #[test]
    fn test_render_footer_shows_help_hint() {
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
    fn test_render_with_red_level() {
        let backend = TestBackend::new(120, 64);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(4);
        app.thermal_level = ThermalLevel::Red;
        terminal.draw(|f| render(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let rendered: String = buf.content.iter().map(|c| c.symbol().to_string()).collect();
        assert!(rendered.contains("RED"), "render MUST show RED label; got: {rendered}");
    }

    #[test]
    fn test_render_with_pinned_panels_compact() {
        let backend = TestBackend::new(60, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut app = App::new(4);
        app.pool_panel = Some(sharecli_fleet::PoolOperatorPanel {
            node_total: 2,
            node_idle: 1,
            bun_total: 1,
            bun_idle: 0,
            max_per_type: 4,
            healthy: true,
        });
        app.status_panel = Some(sharecli_fleet::StatusOperatorPanel {
            scanned: 50,
            watched: 1,
            total_processes: 2,
            agent_rows: 1,
        });
        terminal.draw(|f| render(f, &app)).unwrap();
        let buf = terminal.backend().buffer().clone();
        let rendered: String = buf.content.iter().map(|c| c.symbol().to_string()).collect();
        assert!(
            rendered.contains("Pool node"),
            "compact render MUST show pool summary; got: {rendered}"
        );
        assert!(
            rendered.contains("Proc scan"),
            "compact render MUST show status summary; got: {rendered}"
        );
    }
}
