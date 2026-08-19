use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};

use crate::app::App;

const POPUP_HEIGHT_PERCENT: u16 = 70;
const POPUP_WIDTH_PERCENT: u16 = 80;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - POPUP_HEIGHT_PERCENT) / 2),
            Constraint::Percentage(POPUP_HEIGHT_PERCENT),
            Constraint::Percentage((100 - POPUP_HEIGHT_PERCENT) / 2),
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

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(t.comment_border()))
        .style(Style::default().bg(t.comment_bg()))
        .title(Span::styled(
            " Export comments ",
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

    let text = app.comment_export_text.as_str();

    let content = Paragraph::new(text)
        .style(Style::default().fg(t.comment_text_fg()).bg(t.comment_bg()))
        .wrap(Wrap { trim: false })
        .scroll((app.comment_export_scroll, 0));
    f.render_widget(content, chunks[0]);

    let line_count = app.comment_export_line_count;
    let viewport = chunks[0].height;
    if line_count > viewport {
        let mut state = ScrollbarState::new(line_count.saturating_sub(viewport) as usize)
            .position(app.comment_export_scroll as usize);
        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight),
            chunks[0],
            &mut state,
        );
    }

    let help = Paragraph::new(Line::from(vec![
        Span::styled("↑↓ / j k", Style::default().fg(t.added_fg()).add_modifier(Modifier::BOLD)),
        Span::styled(" scroll  ", Style::default().fg(t.comment_text_fg()).bg(t.comment_bg())),
        Span::styled("Esc / q", Style::default().fg(t.removed_fg()).add_modifier(Modifier::BOLD)),
        Span::styled(" close", Style::default().fg(t.comment_text_fg()).bg(t.comment_bg())),
    ]))
    .alignment(Alignment::Center);
    f.render_widget(help, chunks[1]);
}
