// nonos-tui/src/ui.rs — NØN Sovereign UI Layer
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Sovereign Mesh Visualizer + Capsule Telemetry + Zero-Trust Verification Feed + RAM/CPU Stats + Real-Time System UI

use ratatui::{
    Frame,
    layout::{Layout, Constraint, Direction, Rect},
    style::{Style, Modifier, Color},
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph, Wrap, Row, Table, Gauge},
};

use crate::state::{UiContext, UiSnapshot};

pub fn render_main_ui<B: ratatui::backend::Backend>(f: &mut Frame<B>, snapshot: &UiSnapshot, ctx: &UiContext) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(6),  // Header
            Constraint::Length(8),  // Metrics
            Constraint::Min(10),    // Capsules
            Constraint::Length(6),  // Mesh
            Constraint::Length(3),  // Footer
        ].as_ref())
        .split(f.size());

    render_header(f, chunks[0]);
    render_metrics(f, chunks[1], snapshot);
    render_capsule_list(f, chunks[2], snapshot, ctx);
    render_mesh_peers(f, chunks[3], snapshot);
    render_footer(f, chunks[4]);
}

fn render_header<B: ratatui::backend::Backend>(f: &mut Frame<B>, area: Rect) {
    let title = Paragraph::new(vec![
        Spans::from(Span::styled("  NØN-OS :: ZeroState Boot · Sovereign Runtime Layer", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Spans::from(Span::styled("  Privacy is not a feature — it is a birthright.", Style::default().fg(Color::Gray))),
        Spans::from(Span::styled("  'q' Quit · 'r' Refresh · 'k' Kill · 'v' Verify · '↑↓' Navigate ⊘", Style::default().fg(Color::DarkGray))),
    ])
    .block(Block::default().borders(Borders::ALL).title(" NØN Sovereign UI "))
    .wrap(Wrap { trim: true });

    f.render_widget(title, area);
}

fn render_metrics<B: ratatui::backend::Backend>(f: &mut Frame<B>, area: Rect, snapshot: &UiSnapshot) {
    let rows = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(33),
            Constraint::Percentage(33),
            Constraint::Percentage(34),
        ])
        .split(area);

    let cpu = Gauge::default()
        .block(Block::default().title("CPU Usage").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Green).bg(Color::Black).add_modifier(Modifier::BOLD))
        .percent(snapshot.cpu_usage);
    f.render_widget(cpu, rows[0]);

    let ram = Gauge::default()
        .block(Block::default().title("RAM Used").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Yellow).bg(Color::Black).add_modifier(Modifier::BOLD))
        .percent(snapshot.ram_usage);
    f.render_widget(ram, rows[1]);

    let trust = Gauge::default()
        .block(Block::default().title("Zero-Trust Score").borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Magenta).bg(Color::Black))
        .percent(snapshot.trust_score);
    f.render_widget(trust, rows[2]);
}

fn render_capsule_list<B: ratatui::backend::Backend>(f: &mut Frame<B>, area: Rect, snapshot: &UiSnapshot, ctx: &UiContext) {
    let rows: Vec<Row> = snapshot.capsules.iter().enumerate().map(|(i, proc)| {
        let style = if i == ctx.selected {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else {
            match proc.status.as_str() {
                "Running" => Style::default().fg(Color::Green),
                "Failed" => Style::default().fg(Color::Red),
                _ => Style::default().fg(Color::Gray),
            }
        };

        Row::new(vec![
            proc.name.clone(),
            proc.status.clone(),
            proc.pid.to_string(),
            proc.start_time.clone(),
            proc.capsule_type.clone(),
        ]).style(style)
    }).collect();

    let table = Table::new(rows)
        .header(Row::new(vec!["Name", "Status", "PID", "Start Time", "Type"])
            .style(Style::default().fg(Color::LightCyan).add_modifier(Modifier::BOLD)))
        .block(Block::default().title(" Sovereign Capsules ").borders(Borders::ALL))
        .widths(&[
            Constraint::Length(20),
            Constraint::Length(12),
            Constraint::Length(6),
            Constraint::Length(24),
            Constraint::Length(10),
        ]);

    f.render_widget(table, area);
}

fn render_mesh_peers<B: ratatui::backend::Backend>(f: &mut Frame<B>, area: Rect, snapshot: &UiSnapshot) {
    let peer_info = format!(
        "Mesh Peers: {} · Last Sync: {}",
        snapshot.mesh_peers.len(),
        snapshot.last_sync.clone().unwrap_or_else(|| "never".into())
    );

    let paragraph = Paragraph::new(Spans::from(Span::styled(peer_info, Style::default().fg(Color::Gray))))
        .block(Block::default().borders(Borders::ALL).title(" Mesh Layer Status "))
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn render_footer<B: ratatui::backend::Backend>(f: &mut Frame<B>, area: Rect) {
    let footer = Paragraph::new(
        Spans::from(vec![Span::styled(
            "NØN-OS is a sovereign computation environment. There is no telemetry, no fingerprinting, no central control.",
            Style::default().fg(Color::Gray)
        )])
    )
    .block(Block::default().borders(Borders::TOP))
    .wrap(Wrap { trim: true });

    f.render_widget(footer, area);
}

