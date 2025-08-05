// nonos-tui/src/main.rs — NØN Sovereign Terminal Interface
// Maintained by ek@nonos-tech.xyz | © 2025 NØN Technologies
// Capsule UI w/ Realtime Mesh, zkProof Display, Runtime Sync, and Process Telemetry

use std::error::Error;
use std::time::{Duration, Instant};
use std::sync::{Arc, Mutex};

use ratatui::{
    backend::CrosstermBackend,
    Terminal,
    widgets::*,
    layout::*,
    style::*,
};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tokio::sync::mpsc;
use chrono::Utc;

use crate::{input, panel, state, ui, utils};

mod state;
mod ui;
mod input;
mod panel;
mod utils;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Enable raw mode and setup screen
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Runtime State Channel
    let (tx, mut rx) = mpsc::channel(64);
    let tx_clone = tx.clone();
    let ui_state = Arc::new(Mutex::new(state::UiContext::default()));

    // Poll Capsule State
    let ui_state_clone = ui_state.clone();
    tokio::spawn(async move {
        loop {
            let snapshot = state::gather_composite_state();
            tx_clone.send(snapshot).await.ok();
            tokio::time::sleep(Duration::from_millis(800)).await;
        }
    });

    // Initial Draw
    let mut last_tick = Instant::now();
    if let Ok(snapshot) = state::gather_composite_state() {
        terminal.draw(|f| ui::render_main_ui(f, &snapshot, &ui_state.lock().unwrap()))?;
    }

    // UI Event Loop
    loop {
        if event::poll(Duration::from_millis(120))? {
            match event::read()? {
                Event::Key(key) => {
                    let mut ctx = ui_state.lock().unwrap();
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('r') => {
                            state::force_refresh();
                            utils::audit_log("manual_refresh");
                        },
                        KeyCode::Char('k') => {
                            input::kill_selected_capsule(&ctx);
                            utils::audit_log("capsule_kill");
                        },
                        KeyCode::Char('v') => {
                            input::verify_selected_capsule(&ctx);
                            utils::audit_log("capsule_verify");
                        },
                        KeyCode::Down => ctx.select_next(),
                        KeyCode::Up => ctx.select_prev(),
                        _ => {}
                    }
                },
                _ => {}
            }
        }

        // Redraw on Tick
        if let Ok(Some(snapshot)) = rx.try_recv() {
            let ctx = ui_state.lock().unwrap();
            terminal.draw(|f| ui::render_main_ui(f, &snapshot, &ctx))?;
        }

        if last_tick.elapsed() >= Duration::from_secs(1) {
            last_tick = Instant::now();
        }
    }

    // Shutdown
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

