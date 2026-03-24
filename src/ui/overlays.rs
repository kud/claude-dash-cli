use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::utils::abbreviate_home;

pub fn render_permission(frame: &mut Frame, area: Rect, app: &App) {
    let Some(perm) = app.selected_pending_permission() else {
        return;
    };

    let cwd = abbreviate_home(&perm.cwd);
    let body_lines = build_permission_body(perm);
    // +2 borders, +3 header rows (session/tool/blank), +1 buttons, +1 breathing room
    let content_height = body_lines.len() as u16;
    let popup = centered_rect(area, area.width * 80 / 100, content_height + 8);

    frame.render_widget(Clear, popup);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Yellow))
        .title(Line::from(vec![
            Span::styled(
                " ⚠ Permission Request ",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            ),
        ]));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    // Split inner: buttons pinned at bottom, content fills the rest.
    use ratatui::layout::{Constraint, Layout};
    let [content_area, buttons_area] = Layout::vertical([
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(inner);

    let mut lines = vec![
        Line::from(vec![
            Span::styled("session ", Style::default().fg(Color::DarkGray)),
            Span::raw(&perm.session_id[..8.min(perm.session_id.len())]),
            Span::styled("  ", Style::default()),
            Span::styled(cwd, Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("tool    ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format_tool_name(&perm.tool_name),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw(""),
    ];
    lines.extend(body_lines);

    frame.render_widget(
        Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
        content_area,
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("[a] Allow", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("      "),
            Span::styled("[s] Allow for session", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("      "),
            Span::styled("[d] Deny", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ])),
        buttons_area,
    );
}

fn format_tool_name(name: &str) -> String {
    // mcp__namespace__tool_name → namespace › tool_name
    if let Some(rest) = name.strip_prefix("mcp__") {
        if let Some((ns, tool)) = rest.split_once("__") {
            return format!("{} › {}", ns, tool);
        }
    }
    name.to_string()
}

fn build_permission_body(perm: &crate::types::PendingPermission) -> Vec<Line<'static>> {
    match perm.tool_name.as_str() {
        "Edit" | "MultiEdit" => build_edit_diff(&perm.tool_input),
        "Write" => build_write_diff(&perm.tool_input),
        "Bash" => build_bash_body(&perm.tool_input),
        _ => build_json_body(&perm.tool_input),
    }
}

fn build_edit_diff(input: &serde_json::Value) -> Vec<Line<'static>> {
    let file = input
        .get("file_path")
        .and_then(|v| v.as_str())
        .map(abbreviate_home)
        .unwrap_or_default();
    let old = input.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
    let new = input.get("new_string").and_then(|v| v.as_str()).unwrap_or("");

    let mut lines = vec![Line::from(vec![
        Span::styled("file  ", Style::default().fg(Color::DarkGray)),
        Span::styled(file, Style::default().fg(Color::White)),
    ])];

    if input.get("replace_all").and_then(|v| v.as_bool()).unwrap_or(false) {
        lines.push(Line::from(Span::styled(
            "      replace all occurrences",
            Style::default().fg(Color::Yellow),
        )));
    }

    lines.push(Line::raw(""));

    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();
    const MAX: usize = 30;

    for (i, l) in old_lines.iter().enumerate().take(MAX) {
        lines.push(diff_line("-", l, Color::Red, i == 0 && old_lines.len() > MAX));
    }
    if old_lines.len() > MAX {
        lines.push(elided(old_lines.len() - MAX));
    }

    for (i, l) in new_lines.iter().enumerate().take(MAX) {
        lines.push(diff_line("+", l, Color::Green, i == 0 && new_lines.len() > MAX));
    }
    if new_lines.len() > MAX {
        lines.push(elided(new_lines.len() - MAX));
    }

    lines
}

fn build_write_diff(input: &serde_json::Value) -> Vec<Line<'static>> {
    let file = input
        .get("file_path")
        .and_then(|v| v.as_str())
        .map(abbreviate_home)
        .unwrap_or_default();
    let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");

    let mut lines = vec![
        Line::from(vec![
            Span::styled("file  ", Style::default().fg(Color::DarkGray)),
            Span::styled(file, Style::default().fg(Color::White)),
        ]),
        Line::raw(""),
    ];

    let content_lines: Vec<&str> = content.lines().collect();
    const MAX: usize = 30;
    for l in content_lines.iter().take(MAX) {
        lines.push(diff_line("+", l, Color::Green, false));
    }
    if content_lines.len() > MAX {
        lines.push(elided(content_lines.len() - MAX));
    }

    lines
}

fn build_bash_body(input: &serde_json::Value) -> Vec<Line<'static>> {
    let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
    let cmd_lines: Vec<&str> = cmd.lines().collect();
    let mut lines = vec![
        Line::from(Span::styled("  command", Style::default().fg(Color::DarkGray))),
        Line::raw(""),
    ];
    for l in cmd_lines.iter().take(20) {
        lines.push(Line::from(vec![
            Span::styled("  $ ", Style::default().fg(Color::DarkGray)),
            Span::styled(l.to_string(), Style::default().fg(Color::Cyan)),
        ]));
    }
    if cmd_lines.len() > 20 {
        lines.push(Line::from(Span::styled(
            format!("  … {} more lines", cmd_lines.len() - 20),
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines
}

fn build_json_body(input: &serde_json::Value) -> Vec<Line<'static>> {
    let json = serde_json::to_string_pretty(input).unwrap_or_default();
    let mut lines = Vec::new();
    for l in json.lines().take(20) {
        lines.push(Line::from(Span::styled(
            l.to_string(),
            Style::default().fg(Color::DarkGray),
        )));
    }
    lines
}

fn diff_line(prefix: &'static str, content: &str, color: Color, _first: bool) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{} ", prefix), Style::default().fg(color)),
        Span::styled(content.to_string(), Style::default().fg(color)),
    ])
}

fn elided(n: usize) -> Line<'static> {
    Line::from(Span::styled(
        format!("  … {} more line{}", n, if n == 1 { "" } else { "s" }),
        Style::default().fg(Color::DarkGray),
    ))
}

pub fn render_new_session(frame: &mut Frame, area: Rect, app: &App) {
    let popup = centered_rect(area, 64, 12);
    frame.render_widget(Clear, popup);

    let in_tmux = std::env::var("TMUX").is_ok();
    let in_iterm = std::env::var("TERM_PROGRAM").map(|v| v == "iTerm.app").unwrap_or(false)
        || std::env::var("ITERM_SESSION_ID").is_ok();
    let title = if in_tmux {
        " ◆ New Session "
    } else if in_iterm {
        " ◆ New Session (opens iTerm2) "
    } else {
        " ◆ New Session (opens Terminal.app) "
    };

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Line::from(Span::styled(
            title,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let input_display = format!("{}▌", app.new_session_input);
    let mut lines = vec![
        Line::from(Span::styled("Working directory:", Style::default().fg(Color::DarkGray))),
        Line::raw(""),
        Line::from(vec![
            Span::raw(" "),
            Span::styled(input_display, Style::default().fg(Color::White)),
        ]),
        Line::raw(""),
    ];

    let recent = app.recent_cwds();
    if !recent.is_empty() && !app.new_session_launched {
        lines.push(Line::from(Span::styled("Recent:", Style::default().fg(Color::DarkGray))));
        for cwd in recent.iter().take(4) {
            lines.push(Line::from(Span::styled(
                format!("  {}", abbreviate_home(cwd)),
                Style::default().fg(Color::DarkGray),
            )));
        }
        lines.push(Line::raw(""));
    }

    if app.new_session_launched {
        lines.push(Line::from(Span::styled(
            format!("✓ launching claude in {}…", abbreviate_home(&app.new_session_input)),
            Style::default().fg(Color::Green),
        )));
    }

    if let Some(err) = &app.new_session_error {
        lines.push(Line::from(Span::styled(
            format!("✗ {}", err),
            Style::default().fg(Color::Red),
        )));
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "[enter] launch   [esc] cancel",
        Style::default().fg(Color::DarkGray),
    )));

    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}

pub fn render_rename(frame: &mut Frame, area: Rect, app: &App) {
    let Some(session) = app.selected_session() else { return };
    let popup = centered_rect(area, 52, 7);
    frame.render_widget(Clear, popup);

    let current = app.session_display_name(&session.session_id);
    let title = format!(" ✎ Rename — {} ", current);

    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan))
        .title(Line::from(Span::styled(
            title,
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )));

    let inner = block.inner(popup);
    frame.render_widget(block, popup);

    let input_display = format!("{}▌", app.rename_input);
    let lines = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw(" "),
            Span::styled(input_display, Style::default().fg(Color::White)),
        ]),
        Line::raw(""),
        Line::from(Span::styled(
            " empty name → restore original id",
            Style::default().fg(Color::DarkGray),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            "[enter] confirm   [esc] cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    frame.render_widget(Paragraph::new(Text::from(lines)), inner);
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}
