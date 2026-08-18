use std::path::PathBuf;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
}

impl FileStatus {
    pub fn badge(&self) -> &'static str {
        match self {
            Self::Modified => "M",
            Self::Added => "A",
            Self::Deleted => "D",
            Self::Renamed => "MV",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLineKind {
    Added,
    Removed,
    Context,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct Hunk {
    pub header: String,
    pub lines: Vec<DiffLine>,
}

#[derive(Debug, Clone)]
pub struct DiffFile {
    pub path: PathBuf,
    pub old_path: Option<PathBuf>,
    pub status: FileStatus,
    pub hunks: Vec<Hunk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Baseline {
    WorkingTree,
    Commit { oid: String, summary: String },
}

impl Baseline {
    pub fn label(&self) -> String {
        match self {
            Self::WorkingTree => "Working Tree (staged + unstaged)".into(),
            Self::Commit { summary, oid } => format!("{} {}", &oid[..7], summary),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    pub id: Uuid,
    pub file: PathBuf,
    /// 1-based line number in the new file at time of creation
    pub line_no: usize,
    /// SHA-256 hex of ±10 lines of context at save time
    pub anchor_hash: String,
    pub text: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default)]
    pub stale: bool,
}

impl Comment {
    pub fn new(file: PathBuf, line_no: usize, anchor_hash: String, text: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            file,
            line_no,
            anchor_hash,
            text,
            created_at: now,
            updated_at: now,
            stale: false,
        }
    }
}
