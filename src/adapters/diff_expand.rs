use std::collections::HashSet;

use crate::domain::{DiffFile, DiffLine, DiffLineKind, FileStatus, Hunk};

const MAX_SCOPE_LINES: usize = 500;
const CONTEXT_LINES: u32 = 10;

pub fn git_context_lines() -> u32 {
    CONTEXT_LINES
}

/// ponytail: naive `{`/`}` count — strings/comments/macros can skew; upgrade path: tree-sitter
fn brace_delta(line: &str) -> i32 {
    line.chars().filter(|&c| c == '{').count() as i32 - line.chars().filter(|&c| c == '}').count() as i32
}

fn is_scope_keyword(line: &str) -> bool {
    let t = line.trim_start();
    ["fn ", "pub fn ", "impl ", "pub impl ", "struct ", "pub struct ", "enum ", "pub enum ", "trait ", "pub trait "]
        .iter()
        .any(|kw| t.starts_with(kw))
}

fn brace_depths_after_each_line(lines: &[String]) -> Vec<i32> {
    let mut depths = vec![0i32; lines.len()];
    let mut depth = 0i32;
    for (i, line) in lines.iter().enumerate() {
        depth += brace_delta(line);
        depths[i] = depth;
    }
    depths
}

fn depth_before_line(depths: &[i32], lineno: usize) -> i32 {
    if lineno <= 1 { 0 } else { depths[lineno - 2] }
}

/// Walk upward from the change, stopping at the opening `{` or a scope keyword.
fn walk_up_to_scope_start(lines: &[String], depths: &[i32], change_start: usize, depth_before_change: i32) -> usize {
    let mut scope_start = change_start;
    for i in (0..change_start - 1).rev() {
        let depth_before_candidate = depth_before_line(depths, i + 1);
        if depth_before_candidate < depth_before_change {
            scope_start = i + 1;
            break;
        }
        if is_scope_keyword(&lines[i]) {
            scope_start = i + 1;
        }
    }
    scope_start
}

/// Walk downward from the change until the block opened at `scope_start` closes.
fn walk_down_to_scope_end(depths: &[i32], scope_start: usize, change_end: usize, line_count: usize) -> usize {
    let entry_depth = depth_before_line(depths, scope_start);
    let mut scope_end = change_end;
    for i in change_end - 1..line_count {
        scope_end = i + 1;
        if depths[i] <= entry_depth {
            break;
        }
    }
    scope_end
}

fn cap_scope_to_max_lines(scope_start: usize, scope_end: usize, line_count: usize) -> (usize, usize) {
    let mut scope_end = scope_end;
    if scope_end.saturating_sub(scope_start) + 1 > MAX_SCOPE_LINES {
        scope_end = scope_start + MAX_SCOPE_LINES - 1;
    }
    (scope_start, scope_end.min(line_count))
}

/// 1-based inclusive line range containing the innermost `{` block around `start..=end`.
pub fn expand_brace_scope(lines: &[String], start: usize, end: usize) -> (usize, usize) {
    let n = lines.len();
    if n == 0 {
        return (start, end);
    }
    let start = start.clamp(1, n);
    let end = end.clamp(start, n);

    let depths = brace_depths_after_each_line(lines);
    let scope_start = walk_up_to_scope_start(lines, &depths, start, depth_before_line(&depths, start));
    let scope_end = walk_down_to_scope_end(&depths, scope_start, end, n);
    cap_scope_to_max_lines(scope_start, scope_end, n)
}

fn hunk_new_range(hunk: &Hunk) -> Option<(usize, usize)> {
    let mut min = usize::MAX;
    let mut max = 0usize;
    for line in &hunk.lines {
        if let Some(n) = line.new_lineno {
            let n = n as usize;
            min = min.min(n);
            max = max.max(n);
        }
    }
    (min <= max).then_some((min, max))
}

fn merge_overlapping_ranges(ranges: &mut Vec<(usize, usize)>) {
    if ranges.is_empty() {
        return;
    }
    ranges.sort_by_key(|r| r.0);
    let mut merged = vec![ranges[0]];
    for &(start, end) in ranges.iter().skip(1) {
        let last = merged.len() - 1;
        if start <= merged[last].1 + 1 {
            merged[last].1 = merged[last].1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    *ranges = merged;
}

fn context_line(content: &str, lineno: u32) -> DiffLine {
    DiffLine {
        kind: DiffLineKind::Context,
        old_lineno: Some(lineno),
        new_lineno: Some(lineno),
        content: content.to_string(),
    }
}

fn expanded_scope_ranges(hunks: &[Hunk], file_lines: &[String]) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    for hunk in hunks {
        let Some((start, end)) = hunk_new_range(hunk) else { continue };
        ranges.push(expand_brace_scope(file_lines, start, end));
    }
    merge_overlapping_ranges(&mut ranges);
    ranges
}

fn hunk_has_line_in_scope(hunk: &Hunk, scope_start: usize, scope_end: usize) -> bool {
    hunk.lines.iter().any(|line| {
        line.new_lineno
            .map(|n| (scope_start..=scope_end).contains(&(n as usize)))
            .unwrap_or(false)
    })
}

fn collect_diff_lines_from_original_hunks(
    hunks: &[Hunk],
    scope_start: usize,
    scope_end: usize,
    seen: &mut HashSet<u32>,
) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    for hunk in hunks {
        if !hunk_has_line_in_scope(hunk, scope_start, scope_end) {
            continue;
        }
        for line in &hunk.lines {
            if let Some(n) = line.new_lineno {
                if (scope_start..=scope_end).contains(&(n as usize)) && seen.insert(n) {
                    lines.push(line.clone());
                }
            } else if matches!(line.kind, DiffLineKind::Removed) {
                lines.push(line.clone());
            }
        }
    }
    lines
}

fn fill_missing_context_lines(
    file_lines: &[String],
    scope_start: usize,
    scope_end: usize,
    seen: &mut HashSet<u32>,
) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    for lineno in scope_start..=scope_end {
        let n = lineno as u32;
        if seen.insert(n) {
            lines.push(context_line(&file_lines[lineno - 1], n));
        }
    }
    lines
}

fn build_expanded_hunk(
    original_hunks: &[Hunk],
    file_lines: &[String],
    scope_start: usize,
    scope_end: usize,
) -> Option<Hunk> {
    let mut seen = HashSet::new();
    let mut hunk_lines = collect_diff_lines_from_original_hunks(
        original_hunks,
        scope_start,
        scope_end,
        &mut seen,
    );
    hunk_lines.extend(fill_missing_context_lines(file_lines, scope_start, scope_end, &mut seen));
    hunk_lines.sort_by_key(|l| (l.new_lineno.unwrap_or(0), matches!(l.kind, DiffLineKind::Removed)));
    (!hunk_lines.is_empty()).then(|| Hunk { header: String::new(), lines: hunk_lines })
}

/// Expand each hunk to its brace scope and merge overlapping hunks.
pub fn expand_hunks_to_scope(file: &mut DiffFile, file_lines: Option<&[String]>) {
    if matches!(file.status, FileStatus::Deleted) {
        return;
    }
    let Some(lines) = file_lines else { return };
    if lines.is_empty() || file.hunks.is_empty() {
        return;
    }

    let ranges = expanded_scope_ranges(&file.hunks, lines);
    if ranges.is_empty() {
        return;
    }

    let original_hunks = std::mem::take(&mut file.hunks);
    file.hunks = ranges
        .into_iter()
        .filter_map(|(scope_start, scope_end)| build_expanded_hunk(&original_hunks, lines, scope_start, scope_end))
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expand_brace_scope_covers_whole_function() {
        let lines = vec![
            "impl Foo {".into(),
            "    fn bar(&self) {".into(),
            "        let x = 1;".into(),
            "        x".into(),
            "    }".into(),
            "}".into(),
        ];
        let (s, e) = expand_brace_scope(&lines, 3, 3);
        assert_eq!(s, 2);
        assert_eq!(e, 5);
        assert!(lines[s - 1].contains("fn bar"));
        assert_eq!(lines[e - 1].trim(), "}");
    }
}
