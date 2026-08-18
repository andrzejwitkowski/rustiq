pub mod baseline;
pub mod comment_input;
pub mod diff_view;
pub mod file_list;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::adapters::highlight::SyntectHighlighter;
use crate::app::{App, DiffViewMode, Screen};

pub fn render(f: &mut Frame, app: &App, hl: &SyntectHighlighter) {
    let t = app.theme;

    match app.screen {
        Screen::BaselinePicker => {
            baseline::render(f, app, f.area());
        }
        Screen::Main | Screen::CommentInput => {
            let area = f.area();

            // vertical split: main area + status bar
            let vert = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(1)])
                .split(area);

            // horizontal split: file list (25%) + diff (75%)
            let horiz = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(25), Constraint::Percentage(75)])
                .split(vert[0]);

            file_list::render(f, app, horiz[0]);
            diff_view::render(f, app, horiz[1], hl);

            // status bar
            let mode_label = match app.view_mode {
                DiffViewMode::Stacked => "STACKED",
                DiffViewMode::Split => "SPLIT",
            };
            let comment_count = app.comments.len();
            let stale_count = app.comments.iter().filter(|c| c.stale).count();
            let status_text = if let Some(msg) = &app.status_message {
                msg.clone()
            } else {
                format!(
                    " {} | Theme: {} | {} | 💬 {} comments{}  c add · e edit · d del · C copy · V view · T theme · r refresh · q quit ",
                    mode_label,
                    t.name(),
                    if app.files.is_empty() { "no changes".into() } else { format!("{} files", app.files.len()) },
                    comment_count,
                    if stale_count > 0 { format!(" ({stale_count} stale)") } else { String::new() },
                )
            };
            let status = Paragraph::new(Line::from(vec![
                Span::styled(status_text, Style::default().fg(t.fg()).bg(t.selection_bg())),
            ]))
            .alignment(Alignment::Left)
            .style(Style::default().bg(t.selection_bg()));
            f.render_widget(status, vert[1]);

            if app.screen == Screen::CommentInput {
                comment_input::render(f, app, area);
            }
        }
    }
}
