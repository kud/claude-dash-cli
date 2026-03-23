use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::utils::{fmt_cost, fmt_tokens};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let active = app.active_count();
    let total = app.sessions.len();

    let mut spans = vec![
        Span::styled("◆ claude-dash", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
    ];

    if active > 0 {
        spans.push(Span::styled(
            format!("  {} active", active),
            Style::default().fg(Color::Green),
        ));
    } else {
        spans.push(Span::styled(
            format!("  {} sessions", total),
            Style::default().fg(Color::DarkGray),
        ));
    }

    if !app.pending_permissions.is_empty() {
        spans.push(Span::styled(
            format!("  ⚠ {} pending", app.pending_permissions.len()),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ));
    }

    if !app.connected {
        spans.push(Span::styled("  ⊘ disconnected", Style::default().fg(Color::Red)));
    }

    if let Some(today) = &app.usage.today {
        spans.push(Span::styled("  │ today ", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(fmt_cost(today.cost), Style::default().fg(Color::Green)));
        spans.push(Span::styled(
            format!(" · {}", fmt_tokens(today.total_tokens)),
            Style::default().fg(Color::DarkGray),
        ));
    }

    if let Some(monthly) = &app.usage.monthly {
        spans.push(Span::styled("  month ", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            fmt_cost(monthly.total_cost),
            Style::default().fg(Color::Cyan),
        ));
    }

    if let Some(limits) = &app.usage.limits {
        let u = limits.five_hour.utilization;
        let color = if u >= 90.0 {
            Color::Red
        } else if u >= 70.0 {
            Color::Yellow
        } else {
            Color::Green
        };
        spans.push(Span::styled("  5hr ", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            format!("{}%", u.round() as u32),
            Style::default().fg(color),
        ));
    }

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}
