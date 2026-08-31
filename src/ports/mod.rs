use std::path::Path;
use anyhow::Result;
use ratatui::style::Style;
use crate::domain::{Baseline, DiffFile};

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

/// Persists the active review session's comments.
///
/// Each rustiq process is a new session. Saves dual-write to a session-tagged
/// archive file and to `.rustiq/comments.txt` (always the active session).
pub trait CommentStore {
    fn save(&self, export_text: &str) -> Result<()>;
}
