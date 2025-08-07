// nonos-tui/src/main.rs — NØN Sovereign Terminal Interface
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Capsule UI w/ Realtime Mesh, zkProof Display, Runtime Sync, Process Telemetry, and Crash-Aware Audit Channels

use std::error::Error;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};
use std::fs::OpenOptions;
use std::io::Write;

use ratatui::{
    backend::CrosstermBackend,
    Terminal,
    widgets::*,
    layout::*,
    style::*,
};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tokio::sync::mpsc;
use chrono::Utc;

use crate::{input, panel, state, ui, utils, router};

mod state;
mod ui;
mod input;
mod panel;
mod utils;
mod router;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    ui::draw_splash(&mut terminal)?;

    let (tx, mut rx) = mpsc::channel(64);
    let tx_clone = tx.clone();
    let ui_state = Arc::new(Mutex::new(state::UiContext::default()));
    let router_state = Arc::new(Mutex::new(router::Router::default()));

    let ui_state_clone = ui_state.clone();
    tokio::spawn(async move {
        loop {
            match state::gather_composite_state() {
                Ok(snapshot) => {
                    if snapshot.has_crash_events() {
                        utils::audit_log("capsule_crash_detected");
                    }
                    tx_clone.send(snapshot).await.ok();
                },
                Err(e) => {
                    log_error("snapshot_error", &e.to_string());
                }
            }
            tokio::time::sleep(Duration::from_millis(800)).await;
        }
    });

    let mut last_tick = Instant::now();
    let mut previous_size = terminal.size()?;
    if let Ok(snapshot) = state::gather_composite_state() {
        terminal.draw(|f| ui::render_main_ui(f, &snapshot, &ui_state.lock().unwrap(), &router_state.lock().unwrap()))?;
    }

    let mut q_pressed = false;
    let mut profiling = false;

    loop {
        if event::poll(Duration::from_millis(120))? {
            match event::read()? {
                Event::Key(key) => {
                    let mut ctx = ui_state.lock().unwrap();
                    let mut router = router_state.lock().unwrap();
                    match key.code {
                        KeyCode::Char('q') => {
                            if q_pressed {
                                break;
                            } else {
                                q_pressed = true;
                                utils::audit_log("first_q_press");
                            }
                        },
                        KeyCode::Esc => {
                            break;
                        },
                        KeyCode::Char('r') => {
                            state::force_refresh();
                            utils::audit_log("manual_refresh");
                        },
                        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            profiling = !profiling;
                            utils::audit_log("profiling_toggled");
                        },
                        KeyCode::Char('k') => {
                            input::kill_selected_capsule(&ctx);
                            utils::audit_log("capsule_kill");
                        },
                        KeyCode::Char('v') => {
                            input::verify_selected_capsule(&ctx);
                            utils::audit_log("capsule_verify");
                        },
                        KeyCode::Left => router.prev_tab(),
                        KeyCode::Right => router.next_tab(),
                        KeyCode::Down => ctx.select_next(),
                        KeyCode::Up => ctx.select_prev(),
                        _ => {}
                    }
                },
                _ => {}
            }
        }

        if let Ok(Some(snapshot)) = rx.try_recv() {
            let ctx = ui_state.lock().unwrap();
            let router = router_state.lock().unwrap();
            terminal.draw(|f| ui::render_main_ui(f, &snapshot, &ctx, &router))?;
            q_pressed = false;
        }

        if terminal.size()? != previous_size {
            utils::audit_log("terminal_resize");
            previous_size = terminal.size()?;
        }

        if profiling {
            let tick_latency = Instant::now().duration_since(last_tick).as_millis();
            println!("[tick] {}ms", tick_latency);
        }

        if last_tick.elapsed() >= Duration::from_secs(1) {
            last_tick = Instant::now();
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn log_error(label: &str, msg: &str) {
    let log_path = dirs::home_dir()
        .unwrap_or_else(|| ".".into())
        .join(".nonos_tui.log");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .unwrap();
    let _ = writeln!(file, "[{}] {} — {}", Utc::now().to_rfc3339(), label, msg);
}
