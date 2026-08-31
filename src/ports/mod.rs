use std::path::Path;
use anyhow::Result;
use ratatui::style::Style;
use crate::domain::{Baseline, Comment, DiffFile};

pub trait GitRepository {
    fn log(&self) -> Result<Vec<Baseline>>;
    fn diff(&self, baseline: &Baseline) -> Result<Vec<DiffFile>>;
    /// Read raw lines of a file from the working tree
    fn read_lines(&self, path: &Path) -> Result<Vec<String>>;
}

#[derive(Debug, Clone)]
pub struct StyledSpan {
    pub text: String,
    pub style: Style,
}

pub type StyledLine = Vec<StyledSpan>;

pub trait Highlighter {
    /// Returns syntax-highlighted spans per line. Falls back to plain if extension unknown.
    fn highlight(&self, path: &Path, source: &str, dark_theme: bool) -> Vec<StyledLine>;
}

/// Persists review comments across rustiq process sessions.
///
/// Each process is a new session. Session archives are JSON files tagged by
/// session id. `.rustiq/comments.txt` always mirrors the **active** session's
/// export text. The TUI loads comments from every session archive.
pub trait CommentStore {
    /// Load comments from all session archives (and legacy `comments.json` if present).
    fn load_all(&self) -> Result<Vec<Comment>>;
    /// Persist every session's comments to its archive; write `active_export` to comments.txt.
    fn save_all(&self, comments: &[Comment], active_export: &str) -> Result<()>;
}
