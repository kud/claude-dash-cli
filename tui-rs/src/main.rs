mod app;
mod daemon;
mod transcript;
mod types;
mod ui;
mod usage;
mod utils;

use std::io;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, MouseEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::{mpsc, watch};

use app::{App, AppEvent};
use daemon::DaemonCommand;

extern crate libc;

fn spawn_daemon() {
    use std::os::unix::net::UnixStream;
    if UnixStream::connect("/tmp/claude-dash-tui.sock").is_ok() {
        return;
    }

    // Discover daemon location relative to the compiled binary.
    // Binary sits at  <project>/tui-rs/target/{debug,release}/claude-dash
    // so ancestors().nth(4) is <project>.
    let project_root = std::env::current_exe()
        .ok()
        .and_then(|p| p.ancestors().nth(4).map(|a| a.to_path_buf()));

    let spawned = project_root.as_ref().and_then(|root| {
        // Prefer compiled JS (npm run build)
        let dist = root.join("dist/daemon/index.js");
        if dist.exists() {
            return std::process::Command::new("node")
                .arg(&dist)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .ok();
        }
        // Fall back to TypeScript source (dev mode)
        let src = root.join("src/daemon/index.ts");
        if src.exists() {
            return std::process::Command::new("npx")
                .args(["tsx", src.to_str().unwrap_or("")])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .ok();
        }
        None
    });

    // Last resort: assume it's in PATH
    if spawned.is_none() {
        let _ = std::process::Command::new("claude-dash-daemon")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }

    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(100));
        if std::os::unix::net::UnixStream::connect("/tmp/claude-dash-tui.sock").is_ok() {
            break;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    if !app::hooks_installed() {
        eprintln!("⚠  hooks not installed — run: npm run install:hooks");
        eprintln!("   Sessions won't be tracked until hooks are active.");
        std::process::exit(1);
    }
    spawn_daemon();
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

async fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let (app_tx, mut app_rx) = mpsc::unbounded_channel::<AppEvent>();
    let (daemon_cmd_tx, daemon_cmd_rx) = mpsc::unbounded_channel::<DaemonCommand>();
    let (usage_refresh_tx, usage_refresh_rx) = mpsc::unbounded_channel::<()>();
    let (transcript_path_tx, transcript_path_rx) = watch::channel::<Option<String>>(None);

    tokio::spawn(daemon::run(app_tx.clone(), daemon_cmd_rx));
    tokio::spawn(usage::run(app_tx.clone(), usage_refresh_rx));
    tokio::spawn(transcript::run(app_tx.clone(), transcript_path_rx));

    let mut app = App::new(daemon_cmd_tx, usage_refresh_tx, transcript_path_tx);
    let mut reader = EventStream::new();
    // Background timer: redraws every second to keep elapsed times fresh.
    // Input events and app state changes trigger immediate redraws.
    let mut refresh_tick = tokio::time::interval(Duration::from_millis(200));
    refresh_tick.tick().await;

    // Initial draw
    terminal.draw(|f| ui::render(f, &app))?;

    'main: loop {
        tokio::select! {
            biased;

            Some(Ok(event)) = reader.next() => {
                let changed = match event {
                    Event::Key(key) if key.kind == crossterm::event::KeyEventKind::Press => {
                        if app.handle_key(key.code, key.modifiers) {
                            break 'main;
                        }
                        true
                    }
                    Event::Mouse(mouse) => {
                        let term_width = terminal.size().map(|s| s.width).unwrap_or(80);
                        let left_panel_end = term_width * 30 / 100;
                        let in_right_panel = mouse.column > left_panel_end;
                        match mouse.kind {
                            MouseEventKind::ScrollUp if in_right_panel => {
                                app.detail_scroll = app.detail_scroll.saturating_add(3);
                            }
                            MouseEventKind::ScrollDown if in_right_panel => {
                                app.detail_scroll = app.detail_scroll.saturating_sub(3);
                            }
                            MouseEventKind::ScrollUp => app.select_prev_pub(),
                            MouseEventKind::ScrollDown => app.select_next_pub(),
                            _ => {}
                        }
                        true
                    }
                    _ => false,
                };
                if changed {
                    terminal.draw(|f| ui::render(f, &app))?;
                }
            }

            Some(event) = app_rx.recv() => {
                app.handle_event(event);
                // Drain all queued app events before redrawing to avoid
                // multiple draws for a burst (e.g. daemon reconnect + snapshot).
                while let Ok(e) = app_rx.try_recv() {
                    app.handle_event(e);
                }
                terminal.draw(|f| ui::render(f, &app))?;
            }

            _ = refresh_tick.tick() => {
                app.tick();
                // Skip tick redraws while a modal is open — keypress events
                // already redraw on every character, and the heavy transcript
                // render underneath is invisible anyway.
                let modal_open = app.show_input || app.show_rename || app.show_new_session;
                if !modal_open {
                    terminal.draw(|f| ui::render(f, &app))?;
                }
            }
        }
    }

    Ok(())
}
