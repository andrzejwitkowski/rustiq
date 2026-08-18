use std::path::PathBuf;
use chrono::Utc;
use uuid::Uuid;

use crate::adapters::comments::JsonCommentStore;
use crate::domain::{Baseline, Comment, DiffFile};
use crate::ports::{CommentStore, GitRepository};
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffViewMode {
    Stacked,
    Split,
}

impl DiffViewMode {
    pub fn toggle(self) -> Self {
        match self {
            Self::Stacked => Self::Split,
            Self::Split => Self::Stacked,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Screen {
    BaselinePicker,
    Main,
    CommentInput,
}

pub struct App {
    // git
    pub repo: Box<dyn GitRepository>,
    // state
    pub screen: Screen,
    pub theme: Theme,
    pub view_mode: DiffViewMode,
    // baseline picker
    pub baselines: Vec<Baseline>,
    pub baseline_cursor: usize,
    // file list
    pub files: Vec<DiffFile>,
    pub file_cursor: usize,
    // diff scroll
    pub diff_scroll: u16,
    // diff line cursor (for comments)
    pub diff_line_cursor: usize,
    // comments
    pub comments: Vec<Comment>,
    pub comment_store: JsonCommentStore,
    // comment input overlay
    pub comment_input_text: String,
    pub comment_editing_id: Option<Uuid>,
    pub status_message: Option<String>,
}

impl App {
    pub fn new(repo: Box<dyn GitRepository>, comment_store: JsonCommentStore) -> anyhow::Result<Self> {
        let baselines = repo.log()?;
        let comments = comment_store.load().unwrap_or_default();
        // stale detection deferred to after first diff load (need file content)
        Ok(Self {
            repo,
            screen: Screen::BaselinePicker,
            theme: Theme::DefaultDark,
            view_mode: DiffViewMode::Stacked,
            baselines,
            baseline_cursor: 0,
            files: vec![],
            file_cursor: 0,
            diff_scroll: 0,
            diff_line_cursor: 0,
            comments,
            comment_store,
            comment_input_text: String::new(),
            comment_editing_id: None,
            status_message: None,
        })
    }

    pub fn select_baseline(&mut self) -> anyhow::Result<()> {
        let baseline = self.baselines[self.baseline_cursor].clone();
        self.files = self.repo.diff(&baseline)?;
        self.file_cursor = 0;
        self.diff_scroll = 0;
        self.diff_line_cursor = 0;
        self.screen = Screen::Main;
        self.check_stale_comments();
        Ok(())
    }

    pub fn current_file(&self) -> Option<&DiffFile> {
        self.files.get(self.file_cursor)
    }

    pub fn all_diff_lines(&self) -> Vec<&crate::domain::DiffLine> {
        self.current_file()
            .map(|f| f.hunks.iter().flat_map(|h| h.lines.iter()).collect())
            .unwrap_or_default()
    }

    pub fn current_line_no(&self) -> Option<usize> {
        self.all_diff_lines()
            .get(self.diff_line_cursor)
            .and_then(|l| l.new_lineno.map(|n| n as usize))
    }

    pub fn current_file_path(&self) -> Option<PathBuf> {
        self.current_file().map(|f| f.path.clone())
    }

    // ── comment operations ──────────────────────────────────────────────────

    pub fn open_comment_input(&mut self) {
        let existing = self.comment_for_current_line().map(|c| (c.id, c.text.clone()));
        if let Some((id, text)) = existing {
            self.comment_editing_id = Some(id);
            self.comment_input_text = text;
        } else {
            self.comment_editing_id = None;
            self.comment_input_text = String::new();
        }
        self.screen = Screen::CommentInput;
    }

    pub fn save_comment(&mut self) {
        let text = self.comment_input_text.trim().to_string();
        if text.is_empty() {
            self.screen = Screen::Main;
            return;
        }
        let Some(file) = self.current_file_path() else { self.screen = Screen::Main; return; };
        let Some(line_no) = self.current_line_no() else { self.screen = Screen::Main; return; };
        let anchor_hash = self.compute_anchor_hash(&file, line_no);

        if let Some(edit_id) = self.comment_editing_id.take() {
            if let Some(c) = self.comments.iter_mut().find(|c| c.id == edit_id) {
                c.text = text;
                c.anchor_hash = anchor_hash;
                c.updated_at = Utc::now();
                c.stale = false;
            }
        } else {
            self.comments.push(Comment::new(file, line_no, anchor_hash, text));
        }
        let _ = self.comment_store.save(&self.comments);
        self.screen = Screen::Main;
    }

    pub fn delete_comment_on_current_line(&mut self) {
        let Some(file) = self.current_file_path() else { return; };
        let Some(line_no) = self.current_line_no() else { return; };
        self.comments.retain(|c| !(c.file == file && c.line_no == line_no));
        let _ = self.comment_store.save(&self.comments);
        self.status_message = Some("Comment deleted.".into());
    }

    pub fn comment_for_current_line(&self) -> Option<&Comment> {
        let file = self.current_file_path()?;
        let line_no = self.current_line_no()?;
        self.comments.iter().find(|c| c.file == file && c.line_no == line_no)
    }

    pub fn comment_for_line(&self, file: &PathBuf, line_no: usize) -> Option<&Comment> {
        self.comments.iter().find(|c| &c.file == file && c.line_no == line_no)
    }

    fn compute_anchor_hash(&self, file: &PathBuf, line_no: usize) -> String {
        let lines = self.repo.read_lines(file).unwrap_or_default();
        JsonCommentStore::anchor_hash(&lines, line_no)
    }

    fn check_stale_comments(&mut self) {
        for comment in self.comments.iter_mut() {
            let lines = match std::fs::read_to_string(&comment.file) {
                Ok(s) => s.lines().map(str::to_string).collect::<Vec<_>>(),
                Err(_) => { comment.stale = true; continue; }
            };
            let current_hash = JsonCommentStore::anchor_hash(&lines, comment.line_no);
            comment.stale = current_hash != comment.anchor_hash;
        }
    }

    // ── clipboard export ─────────────────────────────────────────────────────

    pub fn export_comments_to_clipboard(&mut self) {
        if self.comments.is_empty() {
            self.status_message = Some("No comments to export.".into());
            return;
        }
        let text = self.format_comments_for_export();
        match arboard::Clipboard::new().and_then(|mut cb| cb.set_text(text.clone())) {
            Ok(_) => self.status_message = Some(format!("Copied {} comment(s) to clipboard.", self.comments.len())),
            Err(_) => {
                // fallback: dump to a temp file
                let path = std::env::temp_dir().join("rustiq_comments.txt");
                let _ = std::fs::write(&path, &text);
                self.status_message = Some(format!("Clipboard unavailable. Written to {}", path.display()));
            }
        }
    }

    fn format_comments_for_export(&self) -> String {
        let mut out = String::new();
        for c in &self.comments {
            out.push_str(&format!("=== {} : line {} ===\n", c.file.display(), c.line_no));
            if c.stale { out.push_str("[STALE]\n"); }
            // context lines from working tree
            let lines = std::fs::read_to_string(&c.file)
                .map(|s| s.lines().map(str::to_string).collect::<Vec<_>>())
                .unwrap_or_default();
            let start = c.line_no.saturating_sub(11);
            let end = (c.line_no + 9).min(lines.len());
            out.push_str(&format!("--- a/{}\n+++ b/{}\n@@ -{},{} +{},{} @@\n",
                c.file.display(), c.file.display(),
                start + 1, end - start, start + 1, end - start));
            for (i, line) in lines[start..end].iter().enumerate() {
                let lineno = start + i + 1;
                let prefix = if lineno == c.line_no { ">" } else { " " };
                out.push_str(&format!("{prefix} {line}\n"));
            }
            out.push_str(&format!("# Comment: {}\n\n", c.text));
        }
        out
    }

    // ── navigation ───────────────────────────────────────────────────────────

    pub fn file_up(&mut self) {
        if self.file_cursor > 0 {
            self.file_cursor -= 1;
            self.diff_scroll = 0;
            self.diff_line_cursor = 0;
        }
    }

    pub fn file_down(&mut self) {
        if self.file_cursor + 1 < self.files.len() {
            self.file_cursor += 1;
            self.diff_scroll = 0;
            self.diff_line_cursor = 0;
        }
    }

    pub fn diff_line_up(&mut self) {
        if self.diff_line_cursor > 0 {
            self.diff_line_cursor -= 1;
            if self.diff_line_cursor < self.diff_scroll as usize {
                self.diff_scroll = self.diff_line_cursor as u16;
            }
        }
    }

    pub fn diff_line_down(&mut self) {
        let total = self.all_diff_lines().len();
        if self.diff_line_cursor + 1 < total {
            self.diff_line_cursor += 1;
        }
    }
}
