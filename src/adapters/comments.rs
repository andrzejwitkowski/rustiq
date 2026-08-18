use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use anyhow::Result;
use sha2::{Digest, Sha256};

use crate::domain::Comment;
use crate::ports::CommentStore;

pub struct JsonCommentStore {
    path: PathBuf,
}

impl JsonCommentStore {
    pub fn new(repo_root: &Path) -> Result<Self> {
        let dir = repo_root.join(".rustiq");
        fs::create_dir_all(&dir)?;
        Ok(Self { path: dir.join("comments.json") })
    }

    /// SHA-256 of ±10 lines of context around `line_no` (1-based).
    pub fn anchor_hash(lines: &[String], line_no: usize) -> String {
        let start = line_no.saturating_sub(10).saturating_sub(1);
        let end = (line_no + 9).min(lines.len());
        let context = lines[start..end].join("\n");
        let hash = Sha256::digest(context.as_bytes());
        format!("{hash:x}")
    }
}

impl CommentStore for JsonCommentStore {
    fn load(&self) -> Result<Vec<Comment>> {
        if !self.path.exists() {
            return Ok(vec![]);
        }
        let data = fs::read_to_string(&self.path)?;
        Ok(serde_json::from_str(&data)?)
    }

    fn save(&self, comments: &[Comment]) -> Result<()> {
        let data = serde_json::to_string_pretty(comments)?;
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut f = File::create(&tmp)?;
            f.write_all(data.as_bytes())?;
            f.sync_all()?;
        }
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}
