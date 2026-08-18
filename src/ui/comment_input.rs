use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
    Frame,
};

use crate::app::App;

const POPUP_HEIGHT: u16 = 7;
const POPUP_WIDTH_PERCENT: u16 = 60;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;

    // Centered 60% wide, 7 lines tall overlay
    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Length(POPUP_HEIGHT),
            Constraint::Min(0),
        ])
        .split(area);
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - POPUP_WIDTH_PERCENT) / 2),
            Constraint::Percentage(POPUP_WIDTH_PERCENT),
            Constraint::Percentage((100 - POPUP_WIDTH_PERCENT) / 2),
        ])
        .split(vert[1]);
    let popup_area = horiz[1];

    f.render_widget(Clear, popup_area);

    let editing = app.comment_editing_id.is_some();
    let title = if editing { " Edit comment " } else { " Add comment " };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(t.comment_border()))
        .style(Style::default().bg(t.comment_bg()))
        .title(Span::styled(
            title,
            Style::default()
                .fg(t.comment_border())
                .bg(t.comment_bg())
                .add_modifier(Modifier::BOLD),
        ));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let text = Paragraph::new(app.comment_input_text.as_str())
        .style(Style::default().fg(t.comment_text_fg()).bg(t.comment_bg()))
        .wrap(ratatui::widgets::Wrap { trim: false });
    f.render_widget(text, chunks[0]);

    let help = Paragraph::new(Line::from(vec![
        Span::styled("Enter", Style::default().fg(t.added_fg()).add_modifier(Modifier::BOLD)),
        Span::styled(" save  ", Style::default().fg(t.comment_text_fg()).bg(t.comment_bg())),
        Span::styled("Esc", Style::default().fg(t.removed_fg()).add_modifier(Modifier::BOLD)),
        Span::styled(" cancel", Style::default().fg(t.comment_text_fg()).bg(t.comment_bg())),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(help, chunks[1]);

    // cursor blinking position — display columns, clamped to the input pane
    let input_width = usize::from(chunks[0].width.max(1));
    let input_height = usize::from(chunks[0].height.max(1));
    let cols = app.comment_input_text.chars().count();
    let mut row = cols / input_width;
    if row >= input_height {
        row = input_height - 1;
    }
    let col = cols % input_width;
    f.set_cursor_position((chunks[0].x + col as u16, chunks[0].y + row as u16));
}
