use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use anyhow::Result;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::ports::CommentStore;

pub struct SessionCommentStore {
    session_id: Uuid,
    session_path: PathBuf,
    active_path: PathBuf,
}

impl SessionCommentStore {
    pub fn new(repo_root: &Path) -> Result<Self> {
        let dir = repo_root.join(".rustiq");
        fs::create_dir_all(&dir)?;
        let session_id = Uuid::new_v4();
        let session_path = dir.join(format!("comments-{session_id}.txt"));
        let active_path = dir.join("comments.txt");
        // Active file always mirrors this process's session (empty until first save).
        atomic_write(&active_path, "")?;
        Ok(Self {
            session_id,
            session_path,
            active_path,
        })
    }

    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    pub fn session_path(&self) -> &Path {
        &self.session_path
    }

    pub fn active_path(&self) -> &Path {
        &self.active_path
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

impl CommentStore for SessionCommentStore {
    fn save(&self, export_text: &str) -> Result<()> {
        // Session archive keeps history across restarts; active file is always the current session.
        atomic_write(&self.session_path, export_text)?;
        atomic_write(&self.active_path, export_text)?;
        Ok(())
    }
}

fn atomic_write(path: &Path, contents: &str) -> Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut f = File::create(&tmp)?;
        f.write_all(contents.as_bytes())?;
        f.sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn new_session_starts_empty_active_file() {
        let dir = tempfile_dir();
        let store = SessionCommentStore::new(&dir).unwrap();
        assert!(store.active_path().exists());
        assert_eq!(fs::read_to_string(store.active_path()).unwrap(), "");
        assert!(!store.session_path().exists());
        let name = store.session_path().file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("comments-"));
        assert!(name.ends_with(".txt"));
    }

    #[test]
    fn save_dual_writes_session_and_active() {
        let dir = tempfile_dir();
        let store = SessionCommentStore::new(&dir).unwrap();
        let text = "=== src/main.rs : line 1 ===\n# Comment: hello\n";
        store.save(text).unwrap();
        assert_eq!(fs::read_to_string(store.session_path()).unwrap(), text);
        assert_eq!(fs::read_to_string(store.active_path()).unwrap(), text);
    }

    #[test]
    fn restart_gets_new_session_and_overwrites_active() {
        let dir = tempfile_dir();
        let first = SessionCommentStore::new(&dir).unwrap();
        first.save("session-one\n").unwrap();
        let first_session = first.session_path().to_path_buf();

        let second = SessionCommentStore::new(&dir).unwrap();
        assert_ne!(first.session_id(), second.session_id());
        assert_eq!(fs::read_to_string(second.active_path()).unwrap(), "");
        assert!(first_session.exists());
        assert_eq!(fs::read_to_string(&first_session).unwrap(), "session-one\n");

        second.save("session-two\n").unwrap();
        assert_eq!(fs::read_to_string(second.active_path()).unwrap(), "session-two\n");
        assert_eq!(fs::read_to_string(second.session_path()).unwrap(), "session-two\n");
        assert_eq!(fs::read_to_string(&first_session).unwrap(), "session-one\n");
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rustiq-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
