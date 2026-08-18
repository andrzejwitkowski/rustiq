use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::app::App;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(2)])
        .split(area);

    let title = Paragraph::new("rustiq — select baseline")
        .alignment(Alignment::Center)
        .style(Style::default().fg(t.fg()).bg(t.bg()).add_modifier(Modifier::BOLD));
    f.render_widget(title, chunks[0]);

    let items: Vec<ListItem> = app
        .baselines
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let label = b.label();
            let style = if i == app.baseline_cursor {
                Style::default().fg(t.fg()).bg(t.selection_bg()).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(t.fg()).bg(t.bg())
            };
            ListItem::new(Line::from(Span::styled(format!(" {label} "), style)))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(t.border()))
                .style(Style::default().bg(t.bg()))
                .title(Span::styled(" Commits ", Style::default().fg(t.comment_fg()))),
        );
    let mut state = ListState::default();
    state.select(Some(app.baseline_cursor));
    f.render_stateful_widget(list, chunks[1], &mut state);

    let help = Paragraph::new("↑/↓ navigate · Enter select · q quit")
        .alignment(Alignment::Center)
        .style(Style::default().fg(t.border()).bg(t.bg()));
    f.render_widget(help, chunks[2]);
}
