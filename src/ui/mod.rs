mod footer;
mod header;
mod overlays;
mod session_list;
mod usage_panel;

use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Style};
use ratatui::widgets::Block;
use ratatui::Frame;

use crate::app::App;

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let vertical = Layout::vertical([
        Constraint::Length(1),
        Constraint::Fill(1),
        Constraint::Length(1),
    ]);
    let [header_area, content_area, footer_area] = vertical.areas(area);

    header::render(frame, header_area, app);
    footer::render(frame, footer_area, app);

    let panel_block = Block::bordered().border_style(Style::default().fg(Color::DarkGray));
    let panel_inner = panel_block.inner(content_area);
    frame.render_widget(panel_block, content_area);

    let inner_height = panel_inner.height;
    let agents_height = (inner_height as f32 * 0.65) as u16;
    let sep_height = 1u16;
    let usage_height = inner_height.saturating_sub(agents_height + sep_height);

    let [agents_area, sep_area, usage_area] = Layout::vertical([
        Constraint::Length(agents_height),
        Constraint::Length(sep_height),
        Constraint::Length(usage_height),
    ]).areas(panel_inner);

    session_list::render(frame, agents_area, app);
    render_separator(frame, sep_area);
    usage_panel::render(frame, usage_area, app);

    if app.selected_pending_permission().is_some() && !app.show_new_session {
        overlays::render_permission(frame, area, app);
    }
    if app.show_new_session {
        overlays::render_new_session(frame, area, app);
    }
    if app.show_rename {
        overlays::render_rename(frame, area, app);
    }
}

fn render_separator(frame: &mut Frame, area: ratatui::layout::Rect) {
    use ratatui::style::Stylize;
    use ratatui::text::Line;
    use ratatui::widgets::Paragraph;
    let sep = "─".repeat(area.width as usize);
    frame.render_widget(
        Paragraph::new(Line::from(sep).dark_gray()),
        area,
    );
}
