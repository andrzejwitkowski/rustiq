use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

use crate::app::{App, DiffViewMode};
use crate::domain::{DiffFile, DiffLine, DiffLineKind};
use crate::ports::{Highlighter, StyledLine};
use crate::theme::Theme;

const GUTTER_WIDTH: u16 = 17;
const GUTTER_SPACER: &str = "              ";

pub fn render(f: &mut Frame, app: &App, area: Rect, hl: &dyn Highlighter) {
    let t = app.theme;

    let Some(file) = app.current_file() else {
        let empty = Paragraph::new("No file selected")
            .style(t.base_style())
            .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded)
                .border_style(Style::default().fg(t.border())).style(Style::default().bg(t.bg())));
        f.render_widget(empty, area);
        return;
    };

    let rename_hint = file
        .old_path
        .as_ref()
        .map(|old| format!(" (from {})", old.display()))
        .unwrap_or_default();
    let has_hunk_headers = file.hunks.iter().any(|h| !h.header.is_empty());
    let title = if has_hunk_headers {
        format!(" {}{} · hunks:{} ", file.path.display(), rename_hint, file.hunks.len())
    } else {
        format!(" {}{} ", file.path.display(), rename_hint)
    };
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

fn render_stacked(f: &mut Frame, app: &App, file: &DiffFile, area: Rect, t: Theme, hl: &dyn Highlighter) {
    let all_lines: Vec<&DiffLine> = file.hunks.iter().flat_map(|h| h.lines.iter()).collect();
    let source = all_lines.iter().map(|l| l.content.as_str()).collect::<Vec<_>>().join("\n");
    let highlighted = hl.highlight(&file.path, &source, t.is_dark());
    let content_width = area.width.saturating_sub(GUTTER_WIDTH).max(12) as usize;
    let lines = render_lines_with_comments(
        &all_lines,
        &highlighted,
        RenderCtx {
            app,
            file,
            theme: t,
            cursor: app.diff_line_cursor,
            content_width,
        },
        |_line| true,
    );

    let para = Paragraph::new(lines)
        .style(t.base_style())
        .scroll((app.diff_scroll, 0));
    f.render_widget(para, area);
}

fn render_split(f: &mut Frame, app: &App, file: &DiffFile, area: Rect, t: Theme, hl: &dyn Highlighter) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let all_lines: Vec<&DiffLine> = file.hunks.iter().flat_map(|h| h.lines.iter()).collect();
    let source = all_lines.iter().map(|l| l.content.as_str()).collect::<Vec<_>>().join("\n");
    let highlighted = hl.highlight(&file.path, &source, t.is_dark());
    let pane_content_width = chunks[1].width.saturating_sub(GUTTER_WIDTH).max(12) as usize;

    // left: old (context + removed)
    let left_lines = render_lines_with_comments(
        &all_lines,
        &highlighted,
        RenderCtx {
            app,
            file,
            theme: t,
            cursor: app.diff_line_cursor,
            content_width: pane_content_width,
        },
        |line| !matches!(line.kind, DiffLineKind::Added),
    );

    // right: new (context + added)
    let right_lines = render_lines_with_comments(
        &all_lines,
        &highlighted,
        RenderCtx {
            app,
            file,
            theme: t,
            cursor: app.diff_line_cursor,
            content_width: pane_content_width,
        },
        |line| !matches!(line.kind, DiffLineKind::Removed),
    );

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
    line_idx: usize,
    cursor: usize,
    has_comment: bool,
) -> Line<'a> {
    let (prefix, line_bg, line_fg) = match dl.kind {
        DiffLineKind::Added => ("+", t.added_bg(), t.added_fg()),
        DiffLineKind::Removed => ("-", t.removed_bg(), t.removed_fg()),
        DiffLineKind::Context => (" ", t.bg(), t.fg()),
    };

    let is_cursor = line_idx == cursor;
    let bg = if is_cursor {
        t.selection_bg()
    } else if has_comment {
        t.commented_line_bg()
    } else {
        line_bg
    };

    // gutter: line numbers (comment block is rendered below, without extra icon)
    let old_no = dl.old_lineno.map(|n| format!("{n:4}")).unwrap_or_else(|| "    ".into());
    let new_no = dl.new_lineno.map(|n| format!("{n:4}")).unwrap_or_else(|| "    ".into());

    let gutter_style = Style::default()
        .fg(if has_comment { t.comment_border() } else { t.border() })
        .bg(bg)
        .add_modifier(if has_comment { Modifier::BOLD } else { Modifier::empty() });

    let prefix_style = Style::default().fg(line_fg).bg(bg).add_modifier(Modifier::BOLD);

    let comment_anchor = if has_comment { ">>" } else { "  " };
    let mut spans: Vec<Span<'static>> = vec![
        Span::styled(format!("{old_no} {new_no} {comment_anchor} {prefix}"), gutter_style),
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

struct RenderCtx<'a> {
    app: &'a App,
    file: &'a DiffFile,
    theme: Theme,
    cursor: usize,
    content_width: usize,
}

fn render_lines_with_comments<'a, F>(
    all_lines: &[&DiffLine],
    highlighted: &[StyledLine],
    ctx: RenderCtx<'_>,
    include_line: F,
) -> Vec<Line<'a>>
where
    F: Fn(&DiffLine) -> bool,
{
    let mut lines = Vec::new();
    for (i, dl) in all_lines.iter().enumerate() {
        if !include_line(dl) {
            continue;
        }
        let hl_spans = highlighted.get(i).cloned().unwrap_or_default();
        let comment = dl
            .new_lineno
            .and_then(|new_lineno| ctx.app.comment_for_line(&ctx.file.path, new_lineno as usize));
        lines.push(diff_line_to_ratatui(
            dl,
            hl_spans,
            ctx.theme,
            i,
            ctx.cursor,
            comment.is_some(),
        ));
        if let Some(comment) = comment {
            lines.extend(render_inline_comment_rows(
                comment.text.as_str(),
                comment.stale,
                ctx.theme,
                ctx.content_width,
            ));
        }
    }
    lines
}

fn render_inline_comment_rows<'a>(
    text: &str,
    stale: bool,
    t: Theme,
    content_width: usize,
) -> Vec<Line<'a>> {
    let tag = if stale { "STALE COMMENT" } else { "COMMENT" };
    let tag_style = Style::default()
        .fg(t.comment_border())
        .bg(t.comment_bg())
        .add_modifier(Modifier::BOLD);
    let body_style = Style::default().fg(t.comment_text_fg()).bg(t.comment_bg());
    let edge_style = Style::default().fg(t.comment_border()).bg(t.comment_bg());
    let spacer_style = Style::default().fg(t.border()).bg(t.bg());
    let wrapped = wrap_comment_text(text, content_width.saturating_sub(2).max(8));

    let mut rows = Vec::new();
    rows.push(Line::from(vec![
        Span::styled(GUTTER_SPACER, spacer_style),
        Span::styled("┏━", edge_style),
        Span::styled(format!(" {tag} "), tag_style),
    ]));

    for chunk in wrapped {
        rows.push(Line::from(vec![
            Span::styled(GUTTER_SPACER, spacer_style),
            Span::styled("┃ ", edge_style),
            Span::styled(chunk, body_style),
        ]));
    }

    rows.push(Line::from(vec![
        Span::styled(GUTTER_SPACER, spacer_style),
        Span::styled("┗", edge_style),
    ]));
    rows
}

fn wrap_comment_text(text: &str, width: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut out = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut chunk = String::new();
        let mut chunk_len = 0usize;
        for ch in line.chars() {
            if chunk_len >= width {
                out.push(std::mem::take(&mut chunk));
                chunk_len = 0;
            }
            chunk.push(ch);
            chunk_len += 1;
        }
        out.push(chunk);
    }
    out
}
