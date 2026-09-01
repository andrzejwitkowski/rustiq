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

enum DiffRow<'a> {
    Line(&'a DiffLine, usize),
    Separator(usize),
}

fn build_display_rows(file: &DiffFile) -> Vec<DiffRow<'_>> {
    let mut rows = Vec::new();
    let mut prev_new: Option<u32> = None;
    let mut line_idx = 0usize;
    for hunk in &file.hunks {
        for line in &hunk.lines {
            if let Some(prev) = prev_new {
                if let Some(new) = line.new_lineno {
                    if new > prev + 1 {
                        rows.push(DiffRow::Separator((new - prev - 1) as usize));
                    }
                }
            }
            rows.push(DiffRow::Line(line, line_idx));
            line_idx += 1;
            if let Some(new) = line.new_lineno {
                prev_new = Some(new);
            }
        }
    }
    rows
}

fn separator_line(omitted: usize, t: Theme) -> Line<'static> {
    let msg = format!("··· {omitted} lines omitted ···");
    Line::from(vec![
        Span::styled(GUTTER_SPACER, Style::default().fg(t.border()).bg(t.bg())),
        Span::styled(msg, Style::default().fg(t.stale_fg()).bg(t.bg()).add_modifier(Modifier::ITALIC)),
    ])
}

pub fn render(f: &mut Frame, app: &mut App, area: Rect, hl: &dyn Highlighter) {
    let t = app.theme;

    let Some(file) = app.current_file().cloned() else {
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
        DiffViewMode::Stacked => render_stacked(f, app, &file, inner, t, hl),
        DiffViewMode::Split => render_split(f, app, &file, inner, t, hl),
    }
}

fn render_stacked(f: &mut Frame, app: &mut App, file: &DiffFile, area: Rect, t: Theme, hl: &dyn Highlighter) {
    let all_lines: Vec<&DiffLine> = file.hunks.iter().flat_map(|h| h.lines.iter()).collect();
    let display_rows = build_display_rows(file);
    let source = all_lines.iter().map(|l| l.content.as_str()).collect::<Vec<_>>().join("\n");
    let highlighted = hl.highlight(&file.path, &source, t.is_dark());
    let content_width = area.width.saturating_sub(GUTTER_WIDTH).max(12) as usize;
    let (lines, cursor_y) = render_stacked_rows(
        &display_rows,
        &highlighted,
        RenderCtx {
            app,
            file,
            theme: t,
            cursor: app.diff_line_cursor,
            content_width,
        },
    );

    sync_diff_scroll(app, lines.len(), cursor_y, area.height.max(1));

    let para = Paragraph::new(lines)
        .style(t.base_style())
        .scroll((app.diff_scroll, 0));
    f.render_widget(para, area);
}

fn render_split(f: &mut Frame, app: &mut App, file: &DiffFile, area: Rect, t: Theme, hl: &dyn Highlighter) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let all_lines: Vec<&DiffLine> = file.hunks.iter().flat_map(|h| h.lines.iter()).collect();
    let display_rows = build_display_rows(file);
    let source = all_lines.iter().map(|l| l.content.as_str()).collect::<Vec<_>>().join("\n");
    let highlighted = hl.highlight(&file.path, &source, t.is_dark());
    let pane_content_width = chunks[1].width.saturating_sub(GUTTER_WIDTH).max(12) as usize;

    let ctx = RenderCtx {
        app,
        file,
        theme: t,
        cursor: app.diff_line_cursor,
        content_width: pane_content_width,
    };
    let (left_lines, right_lines, cursor_y) = render_split_rows(&display_rows, &highlighted, ctx);

    let left_block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(t.border()))
        .style(Style::default().bg(t.bg()))
        .title(Span::styled(" old ", Style::default().fg(t.stale_fg())));
    let right_block = Block::default()
        .style(Style::default().bg(t.bg()))
        .title(Span::styled(" new ", Style::default().fg(t.added_fg())));

    // Viewport is the paragraph inner area (block borders/titles reduce usable height).
    let viewport = right_block.inner(chunks[1]).height.max(1);
    sync_diff_scroll(app, right_lines.len(), cursor_y, viewport);

    f.render_widget(
        Paragraph::new(left_lines).style(t.base_style()).scroll((app.diff_scroll, 0)).block(left_block),
        chunks[0],
    );
    f.render_widget(
        Paragraph::new(right_lines).style(t.base_style()).scroll((app.diff_scroll, 0)).block(right_block),
        chunks[1],
    );
}

/// Keep the cursor row visible when following, and always clamp so the last lines are reachable.
fn sync_diff_scroll(app: &mut App, rendered_len: usize, cursor_y: u16, viewport: u16) {
    let visible = viewport.max(1) as usize;
    app.diff_viewport_height = viewport.max(1);
    app.diff_rendered_len = rendered_len;

    if app.diff_follow_cursor {
        let cy = cursor_y as usize;
        if cy < app.diff_scroll as usize {
            app.diff_scroll = cursor_y;
        } else if cy >= app.diff_scroll as usize + visible {
            app.diff_scroll = (cy + 1 - visible) as u16;
        }
    }

    let max_scroll = rendered_len.saturating_sub(visible) as u16;
    if app.diff_scroll > max_scroll {
        app.diff_scroll = max_scroll;
    }
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

    if hl_spans.is_empty() {
        spans.push(Span::styled(dl.content.clone(), Style::default().fg(line_fg).bg(bg)));
    } else {
        // Keep syntax colors on context/add/remove; only the line background carries diff tint.
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

fn render_stacked_rows<'a>(
    display_rows: &[DiffRow<'_>],
    highlighted: &[StyledLine],
    ctx: RenderCtx<'_>,
) -> (Vec<Line<'a>>, u16) {
    let blank = Line::from(Span::styled(" ", Style::default().bg(ctx.theme.bg())));
    let mut lines = Vec::new();
    let mut cursor_y = 0u16;
    for row in display_rows {
        match row {
            DiffRow::Separator(n) => {
                lines.push(separator_line(*n, ctx.theme));
                lines.push(blank.clone());
            }
            DiffRow::Line(dl, idx) => {
                if *idx == ctx.cursor {
                    cursor_y = lines.len() as u16;
                }
                let hl_spans = highlighted.get(*idx).cloned().unwrap_or_default();
                let comment = dl
                    .new_lineno
                    .and_then(|n| ctx.app.comment_for_line(&ctx.file.path, n as usize));
                lines.push(diff_line_to_ratatui(dl, hl_spans, ctx.theme, *idx, ctx.cursor, comment.is_some()));
                if let Some(comment) = comment {
                    lines.extend(render_inline_comment_rows(
                        comment.text.as_str(),
                        comment.stale,
                        ctx.theme,
                        ctx.content_width,
                    ));
                }
            }
        }
    }
    (lines, cursor_y)
}

fn render_split_rows<'a>(
    display_rows: &[DiffRow<'_>],
    highlighted: &[StyledLine],
    ctx: RenderCtx<'_>,
) -> (Vec<Line<'a>>, Vec<Line<'a>>, u16) {
    let blank = Line::from(Span::styled(" ", Style::default().bg(ctx.theme.bg())));
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut cursor_y = 0u16;
    for row in display_rows {
        match row {
            DiffRow::Separator(n) => {
                let sep = separator_line(*n, ctx.theme);
                left.push(sep.clone());
                right.push(sep);
                left.push(blank.clone());
                right.push(blank.clone());
            }
            DiffRow::Line(dl, idx) => {
                if *idx == ctx.cursor {
                    cursor_y = right.len() as u16;
                }
                let hl_spans = highlighted.get(*idx).cloned().unwrap_or_default();
                let comment = dl
                    .new_lineno
                    .and_then(|n| ctx.app.comment_for_line(&ctx.file.path, n as usize));
                let code = diff_line_to_ratatui(
                    dl,
                    hl_spans,
                    ctx.theme,
                    *idx,
                    ctx.cursor,
                    comment.is_some() && !matches!(dl.kind, DiffLineKind::Removed),
                );
                match dl.kind {
                    DiffLineKind::Added => {
                        left.push(blank.clone());
                        right.push(code);
                    }
                    DiffLineKind::Removed => {
                        left.push(code);
                        right.push(blank.clone());
                    }
                    DiffLineKind::Context => {
                        left.push(code.clone());
                        right.push(code);
                    }
                }
                if let Some(comment) = comment {
                    if !matches!(dl.kind, DiffLineKind::Removed) {
                        let rows = render_inline_comment_rows(
                            comment.text.as_str(),
                            comment.stale,
                            ctx.theme,
                            ctx.content_width,
                        );
                        let n = rows.len();
                        right.extend(rows);
                        left.extend((0..n).map(|_| blank.clone()));
                    }
                }
            }
        }
    }
    (left, right, cursor_y)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::DiffLineKind;
    use crate::ports::StyledSpan;
    use ratatui::style::Color;

    #[test]
    fn added_lines_keep_syntax_spans() {
        let dl = DiffLine {
            kind: DiffLineKind::Added,
            old_lineno: None,
            new_lineno: Some(1),
            content: "let x = 1;".into(),
        };
        let hl = vec![
            StyledSpan {
                text: "let".into(),
                style: Style::default().fg(Color::Rgb(1, 2, 3)),
            },
            StyledSpan {
                text: " x = 1;".into(),
                style: Style::default().fg(Color::Rgb(4, 5, 6)),
            },
        ];
        let line = diff_line_to_ratatui(&dl, hl, Theme::DefaultDark, 0, 0, false);
        // gutter + spacer + 2 syntax spans
        assert!(line.spans.len() >= 4);
        assert_eq!(line.spans[2].content.as_ref(), "let");
        assert_eq!(line.spans[2].style.fg, Some(Color::Rgb(1, 2, 3)));
    }

    #[test]
    fn scroll_clamps_so_last_rows_are_reachable() {
        // Simulate free PageDown past the end: 100 rendered rows, 20 visible.
        let mut scroll = 999u16;
        let follow = false;
        let cursor_y = 0u16;
        let rendered_len = 100usize;
        let viewport = 20u16;
        let visible = viewport.max(1) as usize;
        if follow {
            let cy = cursor_y as usize;
            if cy < scroll as usize {
                scroll = cursor_y;
            } else if cy >= scroll as usize + visible {
                scroll = (cy + 1 - visible) as u16;
            }
        }
        let max_scroll = rendered_len.saturating_sub(visible) as u16;
        if scroll > max_scroll {
            scroll = max_scroll;
        }
        assert_eq!(scroll, 80);
    }

    #[test]
    fn follow_cursor_uses_rendered_y_not_diff_index() {
        // Cursor on rendered row 50 with comments above; viewport 10.
        let mut scroll = 0u16;
        let follow = true;
        let cursor_y = 50u16;
        let rendered_len = 60usize;
        let viewport = 10u16;
        let visible = viewport.max(1) as usize;
        if follow {
            let cy = cursor_y as usize;
            if cy < scroll as usize {
                scroll = cursor_y;
            } else if cy >= scroll as usize + visible {
                scroll = (cy + 1 - visible) as u16;
            }
        }
        let max_scroll = rendered_len.saturating_sub(visible) as u16;
        if scroll > max_scroll {
            scroll = max_scroll;
        }
        assert_eq!(scroll, 41); // 50 visible at bottom of 10-row viewport
    }

    #[test]
    fn split_title_reduces_inner_height() {
        let block = Block::default().title(" new ");
        let area = Rect::new(0, 0, 40, 20);
        assert_eq!(block.inner(area).height, 19);
    }
}

