# claude-dash

A fast, minimal terminal dashboard for monitoring your [Claude Code](https://claude.ai/code) sessions in real time.

Built in Rust with [Ratatui](https://ratatui.rs). Zero lag, no Electron, no browser.

```
┌──────────────────────────────────────────────────────────────────┐
│ ◆ claude-dash  2 active  │  today $14.65 · 27.71M tok  month $334.80 │
├──────────────────────────────────────────────────────────────────┤
│  Agents · 2 active · 3 total                                     │
│                                                                  │
│  󰐊 ACTIVE                                                        │
│  ▶ ◆ 7b25a5bf  ~/Projects/work/mcp-archer  running  2m 52s      │
│       mcp__archer__raw_api_call                                  │
│                                                                  │
│    ● 3beaf61f  ~/Projects/home/my-app  processing  45s           │
│       ⠹ thinking…                                                │
│                                                                  │
│  󰒲 IDLE                                                          │
│    ○ a1c3e9f2  ~/Projects/home/website  idle  1h 12m             │
│                                                                  │
│  ────────────────────────────────────────────────────────────    │
│  Usage  $14.65 today                                             │
│  Today        $14.65  ·  27.71M tok                              │
│  This Month   $334.80  ·  639.21M tok                            │
│  All Time     $576.41  ·  1056.68M tok   36 sessions             │
└──────────────────────────────────────────────────────────────────┘
  [q] quit  [↑↓] select  [e] rename  [n] new  [r] refresh  ● connected
```

## Features

- **Live session list** — see all running Claude Code agents at a glance, grouped by status (waiting for approval → active → idle → ended)
- **Permission modals** — review and approve/deny tool permission requests (Edit, Write, Bash) with diff previews, without leaving the terminal
- **Usage panel** — today's cost, monthly spend, all-time totals, 7-day cost chart, token breakdown and model breakdown — all pulled from [ccusage](https://github.com/ryoppippi/ccusage)
- **Usage cache** — usage data is persisted locally so it appears instantly on next launch
- **Animated indicators** — braille spinner `⠋⠙⠹⠸⠼⠴⠦⠧` while agents are thinking, Nerd Font section icons
- **Session management** — rename sessions, launch new Claude sessions (tmux window, iTerm2 tab, or Terminal.app), clear ended sessions
- **Auto-spawns daemon** — the companion Node.js daemon starts automatically; no separate step needed
- **Mouse support** — scroll the agent list with the mouse wheel

## Requirements

- macOS (Linux untested but should work for the TUI; `osascript` launch only works on macOS)
- [Claude Code](https://claude.ai/code) installed and hooks set up (`npm run install:hooks`)
- [Node.js](https://nodejs.org) 18+ (for the daemon)
- A [Nerd Font](https://www.nerdfonts.com) terminal font for icons (optional but recommended)

## Installation

### From source

```bash
git clone https://github.com/kud/claude-dash-cli
cd claude-dash-cli

# Install hooks into ~/.claude/settings.json
npm run install:hooks

# Build and run
cd tui-rs
cargo run --release
```

### From crates.io

```bash
cargo install claude-dash
```

Then set up hooks in your Claude Code settings:

```bash
# From the cloned repo
npm run install:hooks
```

## Usage

```bash
claude-dash
```

The TUI launches immediately. The daemon is auto-spawned in the background on first run.

### Keybindings

| Key       | Action                                                                         |
| --------- | ------------------------------------------------------------------------------ |
| `↑` / `k` | Select previous session                                                        |
| `↓` / `j` | Select next session                                                            |
| `a`       | Allow pending permission                                                       |
| `s`       | Allow permission for this session (auto-approve future requests for same tool) |
| `d`       | Deny pending permission                                                        |
| `e`       | Rename selected session                                                        |
| `n`       | Launch a new Claude session                                                    |
| `r`       | Refresh usage data                                                             |
| `x`       | Clear ended sessions                                                           |
| `q`       | Quit                                                                           |
| `Q`       | Quit and kill daemon                                                           |

## Architecture

```
claude-dash-cli/
├── src/                    # Node.js daemon + hooks
│   ├── daemon/index.ts     # Unix socket server, session tracking
│   ├── hook/index.ts       # Claude Code PreToolUse/PostToolUse hook handler
│   └── install/index.ts    # Hook installer
└── tui-rs/                 # Rust TUI
    └── src/
        ├── main.rs         # Event loop, terminal setup, daemon spawn
        ├── app.rs          # State machine, key handling
        ├── types.rs        # Domain types
        ├── daemon.rs       # Unix socket client
        ├── usage.rs        # ccusage + Anthropic API rate limits
        ├── utils.rs        # Formatting helpers
        └── ui/
            ├── mod.rs          # Layout
            ├── header.rs       # Top bar
            ├── footer.rs       # Key hints bar
            ├── session_list.rs # Agent list with status groups
            ├── usage_panel.rs  # Cost/token/chart panel
            └── overlays.rs     # Permission modal, new session, rename
```

The daemon communicates with the TUI over a Unix domain socket (`/tmp/claude-dash-tui.sock`) using newline-delimited JSON. Claude Code hooks send events to the daemon on every tool use.

## License

MIT
