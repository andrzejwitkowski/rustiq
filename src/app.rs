use std::path::{Path, PathBuf};
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
    CommentExport,
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
    pub comment_export_scroll: u16,
    pub comment_export_text: String,
    pub comment_export_line_count: u16,
    pub status_message: Option<String>,
    pub diff_viewport_height: u16,
}

impl App {
    pub fn new(repo: Box<dyn GitRepository>, comment_store: JsonCommentStore) -> anyhow::Result<Self> {
        let baselines = repo.log()?;
        let comments = comment_store.load()?;
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
            comment_export_scroll: 0,
            comment_export_text: String::new(),
            comment_export_line_count: 0,
            status_message: None,
            diff_viewport_height: 1,
        })
    }

    pub fn select_baseline(&mut self) -> anyhow::Result<()> {
        let Some(baseline) = self.baselines.get(self.baseline_cursor).cloned() else {
            self.status_message = Some("No baseline is available.".into());
            return Ok(());
        };
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

    pub fn open_comment_export(&mut self) {
        if self.comments.is_empty() {
            self.status_message = Some("No comments to view.".into());
            return;
        }
        self.comment_export_text = self.format_comments_for_export();
        self.comment_export_line_count = self.comment_export_text.lines().count() as u16;
        self.comment_export_scroll = 0;
        self.screen = Screen::CommentExport;
    }

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

        let mut next = self.comments.clone();
        if let Some(edit_id) = self.comment_editing_id {
            if let Some(c) = next.iter_mut().find(|c| c.id == edit_id) {
                c.text = text;
                c.anchor_hash = anchor_hash;
                c.updated_at = Utc::now();
                c.stale = false;
            }
        } else {
            next.push(Comment::new(file, line_no, anchor_hash, text));
        }
        if self.persist_comments(&next, "Comment saved.") {
            self.comments = next;
            self.comment_editing_id = None;
            self.screen = Screen::Main;
        }
    }

    pub fn delete_comment_on_current_line(&mut self) {
        let Some(file) = self.current_file_path() else { return; };
        let Some(line_no) = self.current_line_no() else { return; };
        let next: Vec<_> = self.comments.iter().filter(|c| !(c.file == file && c.line_no == line_no)).cloned().collect();
        if self.persist_comments(&next, "Comment deleted.") {
            self.comments = next;
        }
    }

    pub fn comment_for_current_line(&self) -> Option<&Comment> {
        let file = self.current_file_path()?;
        let line_no = self.current_line_no()?;
        self.comments.iter().find(|c| c.file == file && c.line_no == line_no)
    }

    pub fn comment_for_line(&self, file: &Path, line_no: usize) -> Option<&Comment> {
        self.comments.iter().find(|c| c.file == file && c.line_no == line_no)
    }

    fn compute_anchor_hash(&self, file: &Path, line_no: usize) -> String {
        let lines = self.repo.read_lines(file).unwrap_or_default();
        JsonCommentStore::anchor_hash(&lines, line_no)
    }

    fn check_stale_comments(&mut self) {
        for i in 0..self.comments.len() {
            let file = self.comments[i].file.clone();
            let line_no = self.comments[i].line_no;
            let anchor_hash = self.comments[i].anchor_hash.clone();
            let lines = self.repo.read_lines(&file).unwrap_or_default();
            let current_hash = JsonCommentStore::anchor_hash(&lines, line_no);
            self.comments[i].stale = current_hash != anchor_hash;
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
            Err(_) => match write_private_export(&text) {
                Ok(path) => self.status_message = Some(format!("Clipboard unavailable. Written to {}", path.display())),
                Err(e) => self.status_message = Some(format!("Clipboard export failed: {e}")),
            },
        }
    }

    pub fn format_comments_for_export(&self) -> String {
        let mut out = String::new();
        for c in &self.comments {
            out.push_str(&format!("=== {} : line {} ===\n", c.file.display(), c.line_no));
            if c.stale { out.push_str("[STALE]\n"); }
            // context lines from working tree
            let lines = self.repo.read_lines(&c.file).unwrap_or_default();
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
            let visible = self.diff_viewport_height.max(1) as usize;
            if self.diff_line_cursor >= self.diff_scroll as usize + visible {
                self.diff_scroll = (self.diff_line_cursor + 1 - visible) as u16;
            }
        }
    }

    fn persist_comments(&mut self, comments: &[Comment], ok: &str) -> bool {
        match self.comment_store.save(comments) {
            Ok(()) => {
                self.status_message = Some(ok.into());
                true
            }
            Err(e) => {
                self.status_message = Some(format!("Save failed: {e}"));
                false
            }
        }
    }
}

fn write_private_export(text: &str) -> std::io::Result<PathBuf> {
    let path = std::env::temp_dir().join(format!("rustiq-comments-{}.txt", Uuid::new_v4()));
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)?;
        f.write_all(text.as_bytes())?;
        Ok(path)
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&path, text)?;
        Ok(path)
    }
}
