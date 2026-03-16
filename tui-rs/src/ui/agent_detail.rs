use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::types::{MessageRole, SessionState, ToolHistoryEntry, TranscriptMessage};
use crate::utils::{abbreviate_home, format_duration, format_tool_input, now_ms, wrap_text};

const MAX_DIFF_LINES: usize = 12;

enum TimelineItem<'a> {
    Message(&'a TranscriptMessage),
    Tool(&'a ToolHistoryEntry),
}

fn build_timeline<'a>(
    messages: &'a [TranscriptMessage],
    tool_history: &'a [ToolHistoryEntry],
) -> Vec<TimelineItem<'a>> {
    let mut items: Vec<TimelineItem> = messages
        .iter()
        .map(TimelineItem::Message)
        .chain(tool_history.iter().map(TimelineItem::Tool))
        .collect();
    items.sort_by_key(|item| match item {
        TimelineItem::Message(m) => m.timestamp,
        TimelineItem::Tool(e) => e.started_at,
    });
    items
}

fn is_settled(item: &TimelineItem) -> bool {
    match item {
        TimelineItem::Message(_) => true,
        TimelineItem::Tool(e) => e.ended_at.is_some(),
    }
}

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let Some(session) = app.selected_session() else {
        frame.render_widget(
            Paragraph::new("No session selected").style(Style::default().fg(Color::DarkGray)),
            area,
        );
        return;
    };

    let width = area.width as usize;
    let display_name = app.session_display_name(&session.session_id).to_string();
    let timeline = build_timeline(&app.transcript, &session.tool_history);
    let settled: Vec<_> = timeline.iter().filter(|i| is_settled(i)).collect();
    let active = timeline.iter().rev().find(|i| !is_settled(i));

    let mut lines: Vec<Line<'static>> = Vec::new();

    // Session info line
    lines.push(session_info_line(session, &display_name));
    lines.push(Line::raw(""));

    // Settled timeline items
    for item in &settled {
        push_timeline_item(&mut lines, item, width);
    }

    // Active item or thinking indicator
    if let Some(item) = active {
        lines.push(Line::from(Span::styled("── live ──", Style::default().fg(Color::DarkGray))));
        push_timeline_item(&mut lines, item, width);
    } else if timeline.is_empty() {
        lines.push(Line::from(Span::styled("No activity yet", Style::default().fg(Color::DarkGray))));
    } else {
        use crate::types::SessionStatus;
        match &session.status {
            SessionStatus::Processing | SessionStatus::Compacting => {
                let spinner = thinking_spinner(app.tick_count);
                lines.push(Line::from(vec![
                    Span::styled(spinner, Style::default().fg(Color::Yellow)),
                    Span::styled(" thinking…", Style::default().fg(Color::DarkGray)),
                ]));
            }
            _ => {
                let tool_count = session.tool_history.len();
                let elapsed = format_duration(now_ms() - session.started_at);
                lines.push(Line::from(vec![
                    Span::styled("◦ idle", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        format!("  {} tools  {}", tool_count, elapsed),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }
        }
    }

    let total_rows = visual_row_count(&lines, width);
    let visible = area.height as usize;
    let scroll_from_top: u16 = if total_rows > visible {
        let bottom_offset = total_rows - visible;
        let clamped = app.detail_scroll.min(bottom_offset);
        (bottom_offset - clamped) as u16
    } else {
        0
    };

    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .scroll((scroll_from_top, 0))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn session_info_line(session: &SessionState, display_name: &str) -> Line<'static> {
    let elapsed = format_duration(now_ms() - session.started_at);
    let cwd = abbreviate_home(&session.cwd);
    Line::from(vec![
        Span::styled(
            display_name.to_string(),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  {}", cwd), Style::default().fg(Color::DarkGray)),
        Span::styled(format!("  {}", session.status.label()), Style::default().fg(Color::DarkGray)),
        Span::styled(format!("  {}", elapsed), Style::default().fg(Color::DarkGray)),
    ])
}

fn push_timeline_item(lines: &mut Vec<Line<'static>>, item: &TimelineItem, width: usize) {
    match item {
        TimelineItem::Message(m) => push_message(lines, m, width),
        TimelineItem::Tool(e) => push_tool(lines, e, width),
    }
    lines.push(Line::raw(""));
}

fn push_message(lines: &mut Vec<Line<'static>>, msg: &TranscriptMessage, width: usize) {
    match msg.role {
        MessageRole::User => {
            let max_wrap = (width * 3 / 4).min(90);
            let inner_wrap = max_wrap.saturating_sub(4);
            let wrapped = wrap_text(&msg.text, inner_wrap);
            let inner_width = wrapped.iter().map(|l| l.len()).max().unwrap_or(0);
            let pad = width.saturating_sub(inner_width + 4);
            let border_style = Style::default().fg(Color::Rgb(60, 90, 140));
            let top = format!("{}╭{}╮", " ".repeat(pad), "─".repeat(inner_width + 2));
            let bot = format!("{}╰{}╯", " ".repeat(pad), "─".repeat(inner_width + 2));
            lines.push(Line::from(Span::styled(top, border_style)));
            for line in wrapped {
                let padded = format!("{:<inner_width$}", line, inner_width = inner_width);
                lines.push(Line::from(vec![
                    Span::styled(" ".repeat(pad), Style::default()),
                    Span::styled("│ ", border_style),
                    Span::styled(padded, Style::default().fg(Color::White)),
                    Span::styled(" │", border_style),
                ]));
            }
            lines.push(Line::from(Span::styled(bot, border_style)));
        }
        MessageRole::Assistant => {
            for raw_line in msg.text.lines() {
                if raw_line.trim_start().starts_with("```") {
                    continue;
                }
                if raw_line.is_empty() {
                    lines.push(Line::raw(""));
                    continue;
                }
                let wrap_width = width.saturating_sub(4);
                let (styled_lines, line_style) = if raw_line.starts_with("## ") || raw_line.starts_with("# ") {
                    let content = raw_line.trim_start_matches('#').trim_start().to_string();
                    let wrapped = wrap_text(&content, wrap_width);
                    (wrapped, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                } else if raw_line.starts_with("- ") || raw_line.starts_with("* ") || raw_line.starts_with("• ") {
                    let content = raw_line.trim_start_matches(|c| c == '-' || c == '*' || c == '•').trim_start().to_string();
                    let bullet_content = format!("▸ {}", content);
                    let wrapped = wrap_text(&bullet_content, wrap_width);
                    (wrapped, Style::default().fg(Color::White))
                } else if raw_line.starts_with("    ") || raw_line.starts_with('\t') {
                    let wrapped = wrap_text(raw_line, wrap_width);
                    (wrapped, Style::default().fg(Color::Rgb(120, 140, 160)))
                } else {
                    let wrapped = wrap_text(raw_line, wrap_width);
                    (wrapped, Style::default().fg(Color::White))
                };
                for l in styled_lines {
                    lines.push(Line::from(vec![
                        Span::raw("  "),
                        Span::styled(l, line_style),
                    ]));
                }
            }
        }
    }
}

fn push_tool(lines: &mut Vec<Line<'static>>, entry: &ToolHistoryEntry, _width: usize) {
    let pending = entry.ended_at.is_none();
    let (status_icon, status_color) = if pending {
        ("·", Color::Yellow)
    } else if entry.success != Some(false) {
        ("✓", Color::Green)
    } else {
        ("✗", Color::Red)
    };

    let duration = entry.ended_at.map(|end| format_duration(end - entry.started_at));
    let ts = format_timestamp(entry.started_at);
    let detail = format_tool_input(&entry.tool_name, &entry.tool_input);

    let mut spans = vec![
        Span::styled(format!("{} ", ts), Style::default().fg(Color::DarkGray)),
        Span::styled(format!("{} ", status_icon), Style::default().fg(status_color)),
        Span::styled(
            format!("{:<10}", entry.tool_name),
            Style::default().fg(detail.color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" {}", detail.summary), Style::default().fg(Color::DarkGray)),
    ];

    if let Some(removed) = detail.diff_removed {
        if removed > 0 {
            spans.push(Span::styled(
                format!(" -{}", removed),
                Style::default().fg(Color::Red),
            ));
        }
    }
    if let Some(added) = detail.diff_added {
        if added > 0 {
            spans.push(Span::styled(
                format!(" +{}", added),
                Style::default().fg(Color::Green),
            ));
        }
    }
    if let Some(d) = duration {
        spans.push(Span::styled(format!(" {}", d), Style::default().fg(Color::DarkGray)));
    }

    lines.push(Line::from(spans));

    // Diff content
    push_tool_diff(lines, entry);
}

fn push_tool_diff(lines: &mut Vec<Line<'static>>, entry: &ToolHistoryEntry) {
    match entry.tool_name.as_str() {
        "Edit" | "MultiEdit" => {
            push_diff_lines(
                lines,
                entry.tool_input.get("old_string").and_then(|v| v.as_str()).unwrap_or(""),
                "-",
                Color::Red,
            );
            push_diff_lines(
                lines,
                entry.tool_input.get("new_string").and_then(|v| v.as_str()).unwrap_or(""),
                "+",
                Color::Green,
            );
        }
        "Write" => {
            push_diff_lines(
                lines,
                entry.tool_input.get("content").and_then(|v| v.as_str()).unwrap_or(""),
                "+",
                Color::Green,
            );
        }
        _ => {
            if let Some(output) = &entry.tool_output {
                let content = output.trim();
                if !content.is_empty() {
                    push_diff_lines(lines, content, ">", Color::White);
                }
            }
        }
    }
}

fn push_diff_lines(lines: &mut Vec<Line<'static>>, text: &str, prefix: &'static str, color: Color) {
    let text_lines: Vec<&str> = text.lines().collect();
    let shown = &text_lines[..text_lines.len().min(MAX_DIFF_LINES)];
    let extra = text_lines.len().saturating_sub(MAX_DIFF_LINES);

    for line in shown {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{} ", prefix), Style::default().fg(color)),
            Span::styled(line.to_string(), Style::default().fg(color)),
        ]));
    }
    if extra > 0 {
        lines.push(Line::from(Span::styled(
            format!("  … {} more line{}", extra, if extra > 1 { "s" } else { "" }),
            Style::default().fg(Color::DarkGray),
        )));
    }
}

fn visual_row_count(lines: &[Line<'static>], width: usize) -> usize {
    if width == 0 {
        return lines.len();
    }
    lines
        .iter()
        .map(|line| {
            let w: usize = line.spans.iter().map(|s| s.content.chars().count()).sum();
            if w == 0 { 1 } else { w.div_ceil(width) }
        })
        .sum()
}

fn thinking_spinner(tick: u64) -> &'static str {
    const FRAMES: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
    FRAMES[(tick as usize) % FRAMES.len()]
}

fn format_timestamp(ms: i64) -> String {
    let secs = ms / 1000;
    unsafe {
        let t = secs as libc::time_t;
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
    }
}
