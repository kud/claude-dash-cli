use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::utils::{fmt_cost, fmt_delta, fmt_tokens, progress_bar};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    if area.height < 2 {
        return;
    }

    let [header_area, content_area] =
        Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);

    render_header(frame, header_area, app);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let width = content_area.width as usize;

    push_rate_limits(&mut lines, app, width);
    push_divider(&mut lines, width);
    push_usage_stats(&mut lines, app);
    push_divider(&mut lines, width);
    push_daily_chart(&mut lines, app);
    push_divider(&mut lines, width);
    push_token_breakdown(&mut lines, app);

    let models = app
        .usage
        .total
        .as_ref()
        .map(|t| &t.model_breakdowns[..])
        .or_else(|| app.usage.monthly.as_ref().map(|m| &m.model_breakdowns[..]))
        .unwrap_or(&[]);

    if !models.is_empty() {
        push_divider(&mut lines, width);
        lines.push(Line::from(Span::styled(
            "Model Breakdown",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
        )));
        let max_tokens = models.iter().map(|m| m.total_tokens).max().unwrap_or(1);
        let mut sorted = models.to_vec();
        sorted.sort_by(|a, b| b.total_tokens.cmp(&a.total_tokens));
        for m in &sorted {
            let pct = if max_tokens > 0 {
                (m.total_tokens as f64 / max_tokens as f64) * 100.0
            } else {
                0.0
            };
            let bar = progress_bar(pct, 18);
            let short_name = m
                .model_name
                .replace("claude-", "")
                .trim_end_matches(|c: char| c == '-' || c.is_ascii_digit())
                .to_string();
            let short_name = truncate_model_date(&short_name);
            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:<22}", short_name),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(bar, Style::default().fg(Color::Cyan)),
                Span::raw("  "),
                Span::styled(
                    format!("{:>3}%", pct.round() as u32),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("{:>8}", fmt_cost(m.total_cost)),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  {}", fmt_tokens(m.total_tokens)),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    let cache_created = app
        .usage
        .total
        .as_ref()
        .map(|t| t.cache_creation_tokens)
        .unwrap_or(0);
    let cache_hits = app
        .usage
        .total
        .as_ref()
        .map(|t| t.cache_read_tokens)
        .unwrap_or(0);

    if cache_created > 0 || cache_hits > 0 {
        push_divider(&mut lines, width);
        lines.push(Line::from(Span::styled(
            "Cache",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::from(vec![
            Span::styled("created ", Style::default().fg(Color::DarkGray)),
            Span::raw(fmt_tokens(cache_created)),
            Span::styled(" · hits ", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt_tokens(cache_hits), Style::default().fg(Color::Green)),
            Span::styled(" · saved ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("~{}", fmt_cost(cache_hits as f64 * 0.000003)),
                Style::default().fg(Color::Green),
            ),
        ]));
    }

    if let Some(fetched) = &app.usage.last_fetched {
        let secs = fetched.elapsed().as_secs();
        let when = if secs < 60 {
            format!("{}s ago", secs)
        } else {
            format!("{}m ago", secs / 60)
        };
        lines.push(Line::from(Span::styled(
            format!("last updated {}", when),
            Style::default().fg(Color::DarkGray),
        )));
    }

    frame.render_widget(
        Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
        content_area,
    );
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let mut spans = vec![Span::styled(
        "Usage",
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )];
    if let Some(today) = &app.usage.today {
        spans.push(Span::styled(
            format!("  {}", fmt_cost(today.cost)),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(" today", Style::default().fg(Color::DarkGray)));
    }
    if app.usage.loading {
        spans.push(Span::styled(" ↻", Style::default().fg(Color::DarkGray)));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn push_rate_limits(lines: &mut Vec<Line<'static>>, app: &App, _width: usize) {
    if app.usage.limits_loading && app.usage.limits.is_none() {
        lines.push(Line::from(Span::styled(
            "fetching limits…",
            Style::default().fg(Color::DarkGray),
        )));
        return;
    }
    if !app.usage.limits_loading && app.usage.limits.is_none() {
        lines.push(Line::from(Span::styled(
            "limits unavailable — set ANTHROPIC_API_KEY",
            Style::default().fg(Color::DarkGray),
        )));
        return;
    }
    if let Some(limits) = &app.usage.limits {
        push_rate_row(lines, "5-Hour", limits.five_hour.utilization, &limits.five_hour.resets_at);
        push_rate_row(lines, "7-Day", limits.seven_day.utilization, &limits.seven_day.resets_at);
        if let Some(sonnet) = &limits.seven_day_sonnet {
            push_rate_row(lines, "Sonnet", sonnet.utilization, &sonnet.resets_at);
        }
    }
}

fn push_rate_row(lines: &mut Vec<Line<'static>>, label: &str, utilization: f64, resets_at: &str) {
    let bar = progress_bar(utilization, 24);
    let color = if utilization >= 90.0 {
        Color::Red
    } else if utilization >= 70.0 {
        Color::Yellow
    } else {
        Color::Green
    };
    lines.push(Line::from(vec![
        Span::styled(format!("{:<10}", label), Style::default().fg(Color::DarkGray)),
        Span::styled(bar, Style::default().fg(color)),
        Span::raw(" "),
        Span::styled(
            format!("{:>3}%", utilization.round() as u32),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" resets ", Style::default().fg(Color::DarkGray)),
        Span::raw(resets_at.to_string()),
    ]));
}

fn push_divider(lines: &mut Vec<Line<'static>>, width: usize) {
    lines.push(Line::from(Span::styled(
        "─".repeat(width),
        Style::default().fg(Color::DarkGray),
    )));
}

fn push_usage_stats(lines: &mut Vec<Line<'static>>, app: &App) {
    let usage = &app.usage;

    if usage.loading && usage.today.is_none() {
        lines.push(Line::from(Span::styled(
            "loading usage data…",
            Style::default().fg(Color::DarkGray),
        )));
        return;
    }

    let delta = match (&usage.today, &usage.yesterday) {
        (Some(t), Some(y)) => Some(fmt_delta(t.cost, y.cost)),
        _ => None,
    };

    if let Some(today) = &usage.today {
        push_stat_row(lines, "Today", today.cost, today.total_tokens, delta.as_deref(), None);
    }
    if let Some(monthly) = &usage.monthly {
        push_stat_row(lines, "This Month", monthly.total_cost, monthly.total_tokens, None, None);
    }
    if let Some(total) = &usage.total {
        push_stat_row(
            lines,
            "All Time",
            total.total_cost,
            total.total_tokens,
            None,
            Some(format!("{} sessions", total.sessions)),
        );
    }
}

fn push_stat_row(
    lines: &mut Vec<Line<'static>>,
    label: &str,
    cost: f64,
    tokens: i64,
    delta: Option<&str>,
    extra: Option<String>,
) {
    let mut spans = vec![
        Span::styled(format!("{:<14}", label), Style::default().fg(Color::DarkGray)),
        Span::styled(
            fmt_cost(cost),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::default().fg(Color::DarkGray)),
        Span::raw(fmt_tokens(tokens)),
    ];
    if let Some(d) = delta {
        let color = if d.starts_with('+') { Color::Red } else { Color::Green };
        spans.push(Span::styled(
            format!("  vs yesterday: {}", d),
            Style::default().fg(color),
        ));
    }
    if let Some(e) = extra {
        spans.push(Span::styled(format!("  {}", e), Style::default().fg(Color::DarkGray)));
    }
    lines.push(Line::from(spans));
}

fn push_daily_chart(lines: &mut Vec<Line<'static>>, app: &App) {
    let history = &app.usage.daily_history;
    if history.is_empty() {
        lines.push(Line::from(Span::styled(
            "no history available",
            Style::default().fg(Color::DarkGray),
        )));
        return;
    }

    let last7: Vec<_> = history.iter().rev().take(7).collect::<Vec<_>>().into_iter().rev().collect();
    let max_cost = last7.iter().map(|d| d.cost).fold(0.0_f64, f64::max);
    let bar_width: usize = 20;
    let last_idx = last7.len().saturating_sub(1);

    for (i, day) in last7.iter().enumerate() {
        let filled = if max_cost > 0.0 {
            ((day.cost / max_cost) * bar_width as f64).round() as usize
        } else {
            0
        };
        let empty = bar_width.saturating_sub(filled);
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
        let day_label = day_abbrev_from_iso(&day.date);
        let cost_str = fmt_cost(day.cost);
        let marker = if i == last_idx { " ←" } else { "" };
        lines.push(Line::from(vec![
            Span::styled(format!("{:<4}", day_label), Style::default().fg(Color::DarkGray)),
            Span::styled(bar, Style::default().fg(Color::Cyan)),
            Span::raw("  "),
            Span::styled(
                format!("{:>7}{}", cost_str, marker),
                Style::default().fg(Color::White),
            ),
        ]));
    }
}

fn day_abbrev_from_iso(date: &str) -> &'static str {
    if date.len() < 10 {
        return "   ";
    }
    let year: i64 = date[..4].parse().unwrap_or(0);
    let month: i64 = date[5..7].parse().unwrap_or(0);
    let day: i64 = date[8..10].parse().unwrap_or(0);
    let dow = date_to_dow(year, month, day);
    ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][dow % 7]
}

fn date_to_dow(year: i64, month: i64, day: i64) -> usize {
    let (y, m) = if month < 3 { (year - 1, month + 12) } else { (year, month) };
    let k = y % 100;
    let j = y / 100;
    let h = (day + (13 * (m + 1)) / 5 + k + k / 4 + j / 4 - 2 * j).rem_euclid(7);
    ((h + 5) % 7) as usize
}

fn push_token_breakdown(lines: &mut Vec<Line<'static>>, app: &App) {
    let Some(today) = &app.usage.today else {
        return;
    };

    let total = today.total_tokens;
    if total == 0 {
        return;
    }

    let bar_width: usize = 20;

    let push_token_row = |lines: &mut Vec<Line<'static>>, label: &'static str, tokens: i64, color: Color| {
        let pct = (tokens as f64 / total as f64) * 100.0;
        let filled = ((pct / 100.0) * bar_width as f64).round() as usize;
        let empty = bar_width.saturating_sub(filled);
        let bar = format!("{}{}", "█".repeat(filled), "░".repeat(empty));
        lines.push(Line::from(vec![
            Span::styled(format!("{:<14}", label), Style::default().fg(Color::DarkGray)),
            Span::styled(bar, Style::default().fg(color)),
            Span::raw("  "),
            Span::styled(fmt_tokens(tokens), Style::default().fg(Color::White)),
            Span::styled(
                format!("  {:>3}%", pct.round() as u32),
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    };

    push_token_row(lines, "Input", today.input_tokens, Color::Blue);
    push_token_row(lines, "Output", today.output_tokens, Color::Green);
    push_token_row(lines, "Cache Write", today.cache_creation_tokens, Color::Yellow);
    push_token_row(lines, "Cache Read", today.cache_read_tokens, Color::Cyan);
}

fn truncate_model_date(name: &str) -> String {
    // Strip trailing date suffix like "-20240229"
    let re_end = name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '-');
    re_end.trim_end_matches('-').to_string()
}
