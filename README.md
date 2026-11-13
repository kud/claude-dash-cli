<div align="center">

<img src="assets/icon.svg" width="128" alt="Claude Dash icon" />

📊

# Claude Dash

![Rust](https://img.shields.io/badge/Rust-000000?style=flat-square&logo=rust&logoColor=white)
![crates.io](https://img.shields.io/crates/v/claude-dash?style=flat-square&color=E6822A)
![MIT](https://img.shields.io/badge/licence-MIT-22C55E?style=flat-square)

**📊 Pure-Rust terminal dashboard for Claude Code — live sessions, prompts and usage in real time**

<a href="https://kud.io/projects/claude-dash-cli">Website</a> · <a href="https://kud.io/projects/claude-dash-cli/docs">Documentation</a>

</div>

## Features

A fast, minimal terminal dashboard for monitoring your [Claude Code](https://claude.ai/code) sessions in real time — built entirely in Rust.

- **Live session list** — every running Claude Code agent at a glance, grouped by status (waiting for approval → active → idle → ended)
- **Permission modals** — review and approve or deny tool permission requests (Edit, Write, Bash) without leaving the terminal
- **Usage panel** — today's cost, monthly spend and all-time totals, with a 7-day cost chart and token and model breakdowns, pulled from [ccusage](https://github.com/ryoppippi/ccusage)
- **Session management** — rename sessions, launch new ones (tmux window, iTerm2 tab, or Terminal.app), and clear ended ones
- **Auto-spawned daemon** — the background daemon starts on first launch, no separate step
- **Pure Rust** — a single binary, no Node.js or npm required

## Install

```sh
cargo install claude-dash
claude-dash install
```

## Usage

Run `claude-dash` — the TUI launches immediately and auto-spawns its background daemon on first run.

```console
◆ claude-dash  2 active  │  today $4.20 · 9.8M tok  month $52.10
┌──────────────────────────────────────────────────────────────────┐
│ Agents · 2 active · 4 total                                      │
│                                                                  │
│  ▶ ACTIVE                                                        │
│  ▶ ◆ abc12345  ~/projects/api  running  1m 42s                   │
│       Bash: cargo test --release                                 │
│    ● def56789  ~/projects/web  processing  28s                   │
│       ⠹ thinking…                                                │
│                                                                  │
│  ~ IDLE                                                          │
│    ○ gh901234  ~/projects/docs  idle  1h 03m                     │
│                                                                  │
│  ✗ ENDED                                                         │
│    ✗ ij567890  ~/projects/old  ended  3h 22m                     │
│                                                                  │
│──────────────────────────────────────────────────────────────────│
│ Usage  $4.20 today                                               │
│ Today        $4.20  ·  9.80M tok                                 │
│ This Month   $52.10  ·  136.20M tok                              │
│ All Time     $248.75  ·  612.40M tok   18 sessions               │
└──────────────────────────────────────────────────────────────────┘
[q] quit  [↑↓] select  [e] rename  [n] new  [r] refresh  ● connected
```

## Development

```sh
git clone https://github.com/kud/claude-dash-cli.git
cd claude-dash-cli
cargo build
cargo run
```

📚 **Full documentation → [claude-dash-cli/docs](https://kud.io/projects/claude-dash-cli/docs)**
