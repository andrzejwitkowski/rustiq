use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Frame,
};

use crate::app::App;
use crate::domain::FileStatus;

pub fn render(f: &mut Frame, app: &App, area: Rect) {
    let t = app.theme;

    let items: Vec<ListItem> = app
        .files
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let badge_color = match file.status {
                FileStatus::Added => t.added_fg(),
                FileStatus::Deleted => t.removed_fg(),
                FileStatus::Modified => Color::Rgb(137, 180, 250),
                FileStatus::Renamed => Color::Rgb(203, 166, 247),
            };
            let badge = file.status.badge();
            let path = file.path.display().to_string();
            let comment_count = app.comments.iter().filter(|c| c.file == file.path).count();
            let bg = if i == app.file_cursor { t.selection_bg() } else { t.bg() };
            let name_style = Style::default().fg(t.fg()).bg(bg);
            let badge_style = Style::default().fg(badge_color).bg(bg).add_modifier(Modifier::BOLD);

            let mut spans = vec![
                Span::styled(format!(" {badge:<2} "), badge_style),
                Span::styled(path, name_style),
            ];
            if comment_count > 0 {
                spans.push(Span::styled(
                    format!("   💬 {:>2}", comment_count),
                    Style::default().fg(t.comment_fg()).bg(bg).add_modifier(Modifier::BOLD),
                ));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(t.border()))
            .style(Style::default().bg(t.bg()))
            .title(Span::styled(
                format!(" Files ({}) ", app.files.len()),
                Style::default().fg(t.comment_fg()),
            )),
    );

    let mut state = ListState::default();
    state.select(Some(app.file_cursor));
    f.render_stateful_widget(list, area, &mut state);
}
