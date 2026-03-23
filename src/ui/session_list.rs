use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::types::{SessionState, SessionStatus};
use crate::utils::{abbreviate_home, format_duration, now_ms, thinking_spinner, tool_summary, trunc_mid};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if area.height < 2 {
        return;
    }

    let [header_area, list_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);

    render_header(frame, header_area, app);

    if app.sessions.is_empty() {
        use ratatui::text::Text;
        frame.render_widget(
            Paragraph::new(Text::from(vec![Line::from(Span::styled(
                "Waiting for Claude sessions…",
                Style::default().fg(Color::DarkGray),
            ))])),
            list_area,
        );
        return;
    }

    render_list(frame, list_area, app);
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let active = app.active_count();
    let total = app.sessions.len();

    let mut spans = vec![Span::styled(
        "Agents",
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )];
    if active > 0 {
        spans.push(Span::styled(
            format!(" · {} active", active),
            Style::default().fg(Color::Green),
        ));
    }
    spans.push(Span::styled(
        format!(" · {} total", total),
        Style::default().fg(Color::DarkGray),
    ));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

// A slot is either a non-selectable section header or a session (by index into app.sessions).
enum Slot {
    Blank,
    Header { label: &'static str, color: Color },
    Session(usize),
}

fn build_slots(sessions: &[SessionState]) -> Vec<Slot> {
    let mut slots: Vec<Slot> = Vec::new();
    let mut current_priority: Option<u8> = None;

    for (i, session) in sessions.iter().enumerate() {
        let priority = session.status.sort_priority();
        if Some(priority) != current_priority {
            if current_priority.is_some() {
                slots.push(Slot::Blank);
            }
            let (label, color) = section_label(&session.status);
            slots.push(Slot::Header { label, color });
            current_priority = Some(priority);
        }
        slots.push(Slot::Session(i));
    }

    slots
}

fn section_label(status: &SessionStatus) -> (&'static str, Color) {
    match status {
        SessionStatus::WaitingForApproval => (" waiting for approval", Color::Rgb(130, 130, 130)),
        SessionStatus::Processing | SessionStatus::RunningTool | SessionStatus::Compacting => {
            ("󰐊 active", Color::Rgb(130, 130, 130))
        }
        SessionStatus::WaitingForInput => ("󰒲 idle", Color::Rgb(130, 130, 130)),
        SessionStatus::Ended => ("󰄬 ended", Color::Rgb(130, 130, 130)),
    }
}

fn render_list(frame: &mut Frame, area: Rect, app: &App) {
    let selected = app.clamped_index();
    let slots = build_slots(&app.sessions);

    let visual_selected = slots
        .iter()
        .position(|s| matches!(s, Slot::Session(i) if *i == selected))
        .unwrap_or(0);


    let items: Vec<ListItem> = slots
        .iter()
        .map(|slot| match slot {
            Slot::Blank => ListItem::new(Line::raw("")),
            Slot::Header { label, color } => section_header_item(label, *color),
            Slot::Session(i) => {
                let session = &app.sessions[*i];
                let has_pending = app
                    .pending_permissions
                    .iter()
                    .any(|p| p.session_id == session.session_id);
                let display_name = app.session_display_name(&session.session_id).to_string();
                session_item(session, *i == selected, has_pending, display_name, app.tick_count, area.width as usize)
            }
        })
        .collect();

    let visible_count = area.height as usize;
    let offset = compute_offset(app.list_offset, visual_selected, items.len(), visible_count);
    let visible: Vec<ListItem> = items.into_iter().skip(offset).take(visible_count).collect();

    frame.render_widget(List::new(visible), area);
}

fn section_header_item(label: &'static str, color: Color) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::styled("  ", Style::default()),
        Span::styled(label.to_uppercase(), Style::default().fg(color)),
    ]))
}

fn compute_offset(current_offset: usize, selected: usize, total: usize, visible: usize) -> usize {
    if total == 0 || visible == 0 {
        return 0;
    }
    let mut offset = current_offset;
    if selected < offset {
        offset = selected;
    }
    if selected >= offset + visible {
        offset = selected.saturating_sub(visible - 1);
    }
    offset.min(total.saturating_sub(visible))
}

fn session_item(session: &SessionState, selected: bool, has_pending_permission: bool, display_name: String, tick: u64, area_width: usize) -> ListItem<'_> {
    let elapsed = format_duration(now_ms() - session.started_at);
    // Reserve space for cursor(2) + icon(2) + name(8) + spaces + status(12) + elapsed(8)
    let cwd_max = area_width.saturating_sub(display_name.len() + 36).max(20);
    let label_max = area_width.saturating_sub(5).max(20);
    let cwd = trunc_mid(&abbreviate_home(&session.cwd), cwd_max);
    let status_color = session.status.color();

    let cursor = if selected {
        Span::styled("▶ ", Style::default().fg(Color::Cyan))
    } else {
        Span::raw("  ")
    };

    let id_style = if selected {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let cwd_style = if selected {
        Style::default().fg(Color::White)
    } else {
        Style::default().fg(Color::Gray)
    };

    let mut lines = vec![Line::from(vec![
        cursor,
        Span::styled(session.status.icon(), Style::default().fg(status_color)),
        Span::raw(" "),
        Span::styled(display_name, id_style),
        Span::styled(" ", Style::default().fg(Color::DarkGray)),
        Span::styled(cwd, cwd_style),
        Span::styled(" ", Style::default().fg(Color::DarkGray)),
        Span::styled(session.status.label(), Style::default().fg(status_color)),
        Span::styled(format!(" {}", elapsed), Style::default().fg(Color::DarkGray)),
    ])];

    use crate::types::SessionStatus;
    let tool_label: Option<(String, ratatui::style::Color)> = match &session.status {
        SessionStatus::RunningTool => {
            session.current_tool.as_deref().map(|t| {
                let input = session
                    .current_tool_input
                    .as_ref()
                    .cloned()
                    .unwrap_or(serde_json::Value::Object(Default::default()));
                (trunc_mid(&tool_summary(t, &input), label_max), Color::Yellow)
            })
        }
        SessionStatus::WaitingForApproval => {
            let label = session
                .current_tool
                .as_deref()
                .map(|t| format!("permission needed: {}", t))
                .unwrap_or_else(|| "permission needed".to_string());
            Some((trunc_mid(&label, label_max), Color::Yellow))
        }
        SessionStatus::Processing | SessionStatus::Compacting => {
            Some((format!("{} thinking…", thinking_spinner(tick)), Color::DarkGray))
        }
        SessionStatus::WaitingForInput => {
            let notification = session.last_notification.as_deref().and_then(|n| {
                let is_permission_msg = n.to_lowercase().contains("permission");
                if is_permission_msg && !has_pending_permission { None } else { Some(n) }
            });
            notification.map(|n| (trunc_mid(n, label_max), Color::DarkGray))
        }
        SessionStatus::Ended => Some(("session ended".to_string(), Color::DarkGray)),
    };

    let second_line = if let Some((label, color)) = tool_label {
        Line::from(vec![
            Span::raw("     "),
            Span::styled(label, Style::default().fg(color)),
        ])
    } else {
        Line::raw("")
    };
    lines.push(second_line);

    ListItem::new(lines)
}
