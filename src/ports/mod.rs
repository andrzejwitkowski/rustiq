use std::path::PathBuf;
use anyhow::Result;
use ratatui::style::Style;
use crate::domain::{Baseline, Comment, DiffFile};

pub trait GitRepository {
    fn log(&self) -> Result<Vec<Baseline>>;
    fn diff(&self, baseline: &Baseline) -> Result<Vec<DiffFile>>;
    /// Read raw lines of a file from the working tree
    fn read_lines(&self, path: &PathBuf) -> Result<Vec<String>>;
}

#[derive(Debug, Clone)]
pub struct StyledSpan {
    pub text: String,
    pub style: Style,
}

pub type StyledLine = Vec<StyledSpan>;

pub trait Highlighter {
    /// Returns syntax-highlighted spans per line. Falls back to plain if extension unknown.
    fn highlight(&self, path: &PathBuf, source: &str) -> Vec<StyledLine>;
}

pub trait CommentStore {
    fn load(&self) -> Result<Vec<Comment>>;
    fn save(&self, comments: &[Comment]) -> Result<()>;
}
