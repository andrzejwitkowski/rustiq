use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;

    // Centered 60% wide, 7 lines tall overlay
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(7),
            Constraint::Min(0),
        ])
        .split(area);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(60),
            Constraint::Percentage(20),
        ])
        .split(vert[1]);
    let popup_area = horiz[1];

    f.render_widget(Clear, popup_area);

    let editing = app.comment_editing_id.is_some();
    let title = if editing { " Edit comment " } else { " Add comment " };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(t.comment_fg()))
        .style(Style::default().bg(t.bg()))
        .title(Span::styled(title, Style::default().fg(t.comment_fg()).add_modifier(Modifier::BOLD)));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let text = Paragraph::new(app.comment_input_text.as_str())
        .style(Style::default().fg(t.fg()).bg(t.bg()))
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(text, chunks[0]);

    let help = Paragraph::new(Line::from(vec![
        Span::styled("Enter", Style::default().fg(t.added_fg()).add_modifier(Modifier::BOLD)),
        Span::styled(" save  ", Style::default().fg(t.stale_fg())),
        Span::styled("Esc", Style::default().fg(t.removed_fg()).add_modifier(Modifier::BOLD)),
        Span::styled(" cancel", Style::default().fg(t.stale_fg())),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(help, chunks[1]);

    // cursor blinking position
    let cursor_x = popup_area.x + 1 + (app.comment_input_text.len() % (inner.width as usize)) as u16;
    let cursor_y = popup_area.y + 1 + (app.comment_input_text.len() / inner.width as usize) as u16;
    f.set_cursor_position((cursor_x, cursor_y));
}
