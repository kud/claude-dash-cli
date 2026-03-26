mod app;
mod cmd;
mod daemon;
mod types;
mod ui;
mod usage;
mod utils;

use std::io;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream, MouseEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use app::{App, AppEvent};
use daemon::DaemonCommand;

extern crate libc;

#[derive(Parser)]
#[command(name = "claude-dash", about = "Terminal dashboard for Claude Code sessions")]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run the background daemon (auto-spawned by the TUI)
    Daemon,
    /// Handle a Claude Code hook event — reads JSON from stdin
    Hook,
    /// Install hooks into ~/.claude/settings.json
    Install,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Cmd::Daemon) => return cmd::daemon::run().await,
        Some(Cmd::Hook) => return cmd::hook::run().await,
        Some(Cmd::Install) => {
            cmd::install::run()?;
            return Ok(());
        }
        None => {}
    }

    // Default: launch the TUI
    if !app::hooks_installed() {
        eprintln!("⚠  hooks not installed — run: claude-dash install");
        eprintln!("   Sessions won't be tracked until hooks are active.");
        std::process::exit(1);
    }

    spawn_daemon();

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_tui(&mut terminal).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

fn spawn_daemon() {
    use std::os::unix::net::UnixStream;
    if UnixStream::connect("/tmp/claude-dash-tui.sock").is_ok() {
        return;
    }

    // Spawn ourselves as a daemon subprocess
    let exe = std::env::current_exe()
        .unwrap_or_else(|_| std::path::PathBuf::from("claude-dash"));

    let _ = std::process::Command::new(&exe)
        .arg("daemon")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();

    // Wait up to 2 s for the TUI socket to appear
    for _ in 0..20 {
        std::thread::sleep(Duration::from_millis(100));
        if std::os::unix::net::UnixStream::connect("/tmp/claude-dash-tui.sock").is_ok() {
            break;
        }
    }
}

async fn run_tui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    let (app_tx, mut app_rx) = mpsc::unbounded_channel::<AppEvent>();
    let (daemon_cmd_tx, daemon_cmd_rx) = mpsc::unbounded_channel::<DaemonCommand>();
    let (usage_refresh_tx, usage_refresh_rx) = mpsc::unbounded_channel::<()>();

    tokio::spawn(daemon::run(app_tx.clone(), daemon_cmd_rx));
    tokio::spawn(usage::run(app_tx.clone(), usage_refresh_rx));

    let mut app = App::new(daemon_cmd_tx, usage_refresh_tx);
    let mut reader = EventStream::new();
    let mut refresh_tick = tokio::time::interval(Duration::from_millis(60));
    refresh_tick.tick().await;

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
                        match mouse.kind {
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
                while let Ok(e) = app_rx.try_recv() {
                    app.handle_event(e);
                }
                terminal.draw(|f| ui::render(f, &app))?;
            }

            _ = refresh_tick.tick() => {
                app.tick();
                let modal_open = app.show_rename || app.show_new_session;
                if !modal_open {
                    terminal.draw(|f| ui::render(f, &app))?;
                }
            }
        }
    }

    Ok(())
}
