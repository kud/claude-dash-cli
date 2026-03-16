use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::types::SessionStatus;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let has_pending = !app.pending_permissions.is_empty();
    let has_sessions = !app.sessions.is_empty();

    let dim = Style::default().fg(Color::DarkGray);
    let key = Style::default().fg(Color::White);
    let warn = Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD);

    let mut left_spans: Vec<Span> = vec![
        Span::styled("[q]", key),
        Span::styled(" quit  ", dim),
    ];

    if has_sessions {
        left_spans.extend([Span::styled("[↑↓]", key), Span::styled(" select  ", dim)]);
    }

    let has_selected_pending = app.selected_pending_permission().is_some();
    if has_selected_pending {
        left_spans.extend([
            Span::styled("[a]", warn),
            Span::styled(" allow  ", dim),
            Span::styled("[s]", Style::default().fg(Color::Cyan)),
            Span::styled(" allow session  ", dim),
            Span::styled("[d]", warn),
            Span::styled(" deny  ", dim),
        ]);
    }

    if has_sessions && !app.show_input {
        let selected_waiting = app.selected_session()
            .map(|s| s.status == SessionStatus::WaitingForInput)
            .unwrap_or(false);
        if selected_waiting {
            left_spans.extend([
                Span::styled("[i]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(" reply  ", Style::default().fg(Color::Yellow)),
            ]);
        } else {
            left_spans.extend([Span::styled("[i]", key), Span::styled(" send  ", dim)]);
        }
    }
    if has_sessions && !app.show_rename {
        left_spans.extend([Span::styled("[e]", key), Span::styled(" rename  ", dim)]);
    }

    if !app.show_new_session {
        left_spans.extend([Span::styled("[n]", key), Span::styled(" new  ", dim)]);
    }

    left_spans.extend([Span::styled("[r]", key), Span::styled(" refresh", dim)]);

    if has_sessions {
        left_spans.push(Span::styled("  scroll↕", dim));
    }

    let right_text = if has_pending {
        Line::from(vec![Span::styled(
            format!("⚠ {} pending  ", app.pending_permissions.len()),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )])
    } else if app.connected {
        Line::from(vec![Span::styled(
            "● connected  ",
            Style::default().fg(Color::DarkGray),
        )])
    } else {
        Line::from(vec![Span::styled(
            "⊘ disconnected  ",
            Style::default().fg(Color::Red),
        )])
    };

    let [left_area, right_area] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(20),
    ])
    .areas(area);

    frame.render_widget(Paragraph::new(Line::from(left_spans)), left_area);
    frame.render_widget(
        Paragraph::new(right_text).alignment(Alignment::Right),
        right_area,
    );
}
