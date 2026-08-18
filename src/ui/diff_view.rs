use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::adapters::highlight::SyntectHighlighter;
use crate::app::{App, DiffViewMode};
use crate::domain::{DiffFile, DiffLine, DiffLineKind};
use crate::ports::StyledLine;
use crate::theme::Theme;

pub fn render(f: &mut Frame, app: &App, area: Rect, hl: &SyntectHighlighter) {
    let t = app.theme;

    let Some(file) = app.current_file() else {
        let empty = Paragraph::new("No file selected")
            .style(t.base_style())
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
                .border_style(Style::default().fg(t.border())).style(Style::default().bg(t.bg())));
        f.render_widget(empty, area);
        return;
    };

    let title = format!(" {} ", file.path.display());
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(t.border()))
        .style(Style::default().bg(t.bg()))
        .title(Span::styled(title, Style::default().fg(t.comment_fg())));

    let inner = block.inner(area);
    f.render_widget(block, area);

    match app.view_mode {
        DiffViewMode::Stacked => render_stacked(f, app, file, inner, t, hl),
        DiffViewMode::Split => render_split(f, app, file, inner, t, hl),
    }
}

fn render_stacked(f: &mut Frame, app: &App, file: &DiffFile, area: Rect, t: Theme, hl: &SyntectHighlighter) {
    let all_lines: Vec<&DiffLine> = file.hunks.iter().flat_map(|h| h.lines.iter()).collect();
    let source = all_lines.iter().map(|l| l.content.as_str()).collect::<Vec<_>>().join("\n");
    let highlighted = hl.highlight_with_theme(&file.path, &source, t.is_dark());

    let lines: Vec<Line> = all_lines
        .iter()
        .enumerate()
        .map(|(i, dl)| {
            let global_i = i;
            let hl_spans = highlighted.get(i).cloned().unwrap_or_default();
            diff_line_to_ratatui(dl, hl_spans, t, app, file, global_i, app.diff_line_cursor)
        })
        .collect();

    let para = Paragraph::new(lines)
        .style(t.base_style())
        .scroll((app.diff_scroll, 0));
    f.render_widget(para, area);
}

fn render_split(f: &mut Frame, app: &App, file: &DiffFile, area: Rect, t: Theme, hl: &SyntectHighlighter) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let all_lines: Vec<&DiffLine> = file.hunks.iter().flat_map(|h| h.lines.iter()).collect();
    let source = all_lines.iter().map(|l| l.content.as_str()).collect::<Vec<_>>().join("\n");
    let highlighted = hl.highlight_with_theme(&file.path, &source, t.is_dark());

    // left: old (context + removed)
    let left_lines: Vec<Line> = all_lines
        .iter()
        .enumerate()
        .filter(|(_, l)| !matches!(l.kind, DiffLineKind::Added))
        .map(|(i, dl)| {
            let hl_spans = highlighted.get(i).cloned().unwrap_or_default();
            diff_line_to_ratatui(dl, hl_spans, t, app, file, i, app.diff_line_cursor)
        })
        .collect();

    // right: new (context + added)
    let right_lines: Vec<Line> = all_lines
        .iter()
        .enumerate()
        .filter(|(_, l)| !matches!(l.kind, DiffLineKind::Removed))
        .map(|(i, dl)| {
            let hl_spans = highlighted.get(i).cloned().unwrap_or_default();
            diff_line_to_ratatui(dl, hl_spans, t, app, file, i, app.diff_line_cursor)
        })
        .collect();

    let left_block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(t.border()))
        .style(Style::default().bg(t.bg()))
        .title(Span::styled(" old ", Style::default().fg(t.stale_fg())));
    let right_block = Block::default()
        .style(Style::default().bg(t.bg()))
        .title(Span::styled(" new ", Style::default().fg(t.added_fg())));

    f.render_widget(
        Paragraph::new(left_lines).style(t.base_style()).scroll((app.diff_scroll, 0)).block(left_block),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new(right_lines).style(t.base_style()).scroll((app.diff_scroll, 0)).block(right_block),
        chunks[1],
    );
}

fn diff_line_to_ratatui<'a>(
    dl: &DiffLine,
    hl_spans: StyledLine,
    t: Theme,
    app: &App,
    file: &DiffFile,
    line_idx: usize,
    cursor: usize,
) -> Line<'a> {
    let (prefix, line_bg, line_fg) = match dl.kind {
        DiffLineKind::Added => ("+", t.added_bg(), t.added_fg()),
        DiffLineKind::Removed => ("-", t.removed_bg(), t.removed_fg()),
        DiffLineKind::Context => (" ", t.bg(), t.fg()),
    };

    let is_cursor = line_idx == cursor;
    let bg = if is_cursor { t.selection_bg() } else { line_bg };

    // gutter: line numbers + comment marker
    let old_no = dl.old_lineno.map(|n| format!("{n:4}")).unwrap_or_else(|| "    ".into());
    let new_no = dl.new_lineno.map(|n| format!("{n:4}")).unwrap_or_else(|| "    ".into());

    let comment_marker = dl.new_lineno.and_then(|n| app.comment_for_line(&file.path, n as usize));
    let gutter_suffix = match comment_marker {
        Some(c) if c.stale => " [S]",
        Some(_) => " 💬 ",
        None => "    ",
    };

    let gutter_style = Style::default()
        .fg(t.border())
        .bg(bg);

    let prefix_style = Style::default().fg(line_fg).bg(bg).add_modifier(Modifier::BOLD);

    let mut spans: Vec<Span<'static>> = vec![
        Span::styled(format!("{old_no} {new_no}{gutter_suffix}{prefix}"), gutter_style),
        Span::styled(" ".to_string(), prefix_style),
    ];

    if hl_spans.is_empty() || matches!(dl.kind, DiffLineKind::Added | DiffLineKind::Removed) {
        // override with uniform color for add/remove lines (keep readability)
        spans.push(Span::styled(dl.content.clone(), Style::default().fg(line_fg).bg(bg)));
    } else {
        for s in hl_spans {
            spans.push(Span::styled(s.text, s.style.bg(bg)));
        }
    }

    Line::from(spans)
}
