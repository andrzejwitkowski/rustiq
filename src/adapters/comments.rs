use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::Comment;
use crate::ports::CommentStore;

pub struct SessionCommentStore {
    dir: PathBuf,
    session_id: Uuid,
    session_path: PathBuf,
    active_path: PathBuf,
}

impl SessionCommentStore {
    pub fn new(repo_root: &Path) -> Result<Self> {
        let dir = repo_root.join(".rustiq");
        fs::create_dir_all(&dir)?;
        let session_id = Uuid::new_v4();
        let session_path = dir.join(format!("comments-{session_id}.json"));
        let active_path = dir.join("comments.txt");
        // Active export always mirrors this process's session (empty until first save).
        atomic_write(&active_path, "")?;
        Ok(Self {
            dir,
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

    fn session_file(&self, session_id: Uuid) -> PathBuf {
        self.dir.join(format!("comments-{session_id}.json"))
    }

    fn read_session_file(path: &Path, fallback_session: Uuid) -> Result<Vec<Comment>> {
        if !path.exists() {
            return Ok(vec![]);
        }
        let data = fs::read_to_string(path)
            .with_context(|| format!("read {}", path.display()))?;
        if data.trim().is_empty() {
            return Ok(vec![]);
        }
        let mut comments: Vec<Comment> = serde_json::from_str(&data)
            .with_context(|| format!("parse {}", path.display()))?;
        for c in &mut comments {
            // Legacy rows / filename ownership win if session_id was missing in older shapes —
            // serde requires the field now, but keep ownership consistent with the file name.
            if c.session_id.is_nil() {
                c.session_id = fallback_session;
            }
        }
        Ok(comments)
    }

    fn parse_session_id_from_name(name: &str) -> Option<Uuid> {
        let id = name.strip_prefix("comments-")?.strip_suffix(".json")?;
        Uuid::parse_str(id).ok()
    }

    fn existing_session_files(&self) -> Result<Vec<(Uuid, PathBuf)>> {
        let mut out = Vec::new();
        if !self.dir.exists() {
            return Ok(out);
        }
        for entry in fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if let Some(id) = Self::parse_session_id_from_name(name) {
                out.push((id, path));
            }
        }
        Ok(out)
    }
}

impl CommentStore for SessionCommentStore {
    fn load_all(&self) -> Result<Vec<Comment>> {
        let mut all = Vec::new();
        let mut seen_ids = HashSet::new();

        for (session_id, path) in self.existing_session_files()? {
            for c in Self::read_session_file(&path, session_id)? {
                if seen_ids.insert(c.id) {
                    all.push(c);
                }
            }
        }

        // One-shot migration from pre-session `comments.json`.
        let legacy = self.dir.join("comments.json");
        if legacy.exists() {
            let legacy_session = Uuid::nil();
            for mut c in Self::read_session_file(&legacy, legacy_session)? {
                if c.session_id.is_nil() {
                    c.session_id = legacy_session;
                }
                if seen_ids.insert(c.id) {
                    all.push(c);
                }
            }
        }

        Ok(all)
    }

    fn save_all(&self, comments: &[Comment], active_export: &str) -> Result<()> {
        let mut by_session: HashMap<Uuid, Vec<&Comment>> = HashMap::new();
        for c in comments {
            by_session.entry(c.session_id).or_default().push(c);
        }
        // Ensure the active session archive exists even when it has no comments yet.
        by_session.entry(self.session_id).or_default();

        let mut written = HashSet::new();
        for (session_id, list) in &by_session {
            let owned: Vec<Comment> = list.iter().map(|c| (*c).clone()).collect();
            let path = self.session_file(*session_id);
            let data = serde_json::to_string_pretty(&owned)?;
            atomic_write(&path, &data)?;
            written.insert(*session_id);
        }

        // Drop emptied past-session archives (keep active session file always).
        for (session_id, path) in self.existing_session_files()? {
            if session_id != self.session_id && !written.contains(&session_id) {
                let _ = fs::remove_file(path);
            }
        }

        atomic_write(&self.active_path, active_export)?;
        // Touch/create active session path for consistency when empty.
        if !self.session_path.exists() {
            atomic_write(&self.session_path, "[]")?;
        }
        Ok(())
    }
}

fn atomic_write(path: &Path, contents: &str) -> Result<()> {
    let tmp = path.with_extension(format!(
        "{}.tmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("dat")
    ));
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
    use crate::ports::CommentStore;
    use std::path::PathBuf;

    #[test]
    fn new_session_starts_empty_active_file() {
        let dir = tempfile_dir();
        let store = SessionCommentStore::new(&dir).unwrap();
        assert!(store.active_path().exists());
        assert_eq!(fs::read_to_string(store.active_path()).unwrap(), "");
        let name = store.session_path().file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("comments-"));
        assert!(name.ends_with(".json"));
    }

    #[test]
    fn load_all_includes_past_sessions() {
        let dir = tempfile_dir();
        let first = SessionCommentStore::new(&dir).unwrap();
        let c = Comment::new(
            first.session_id(),
            PathBuf::from("src/main.rs"),
            10,
            "abc".into(),
            "fix me".into(),
        );
        first
            .save_all(std::slice::from_ref(&c), "active-one\n")
            .unwrap();

        let second = SessionCommentStore::new(&dir).unwrap();
        assert_ne!(first.session_id(), second.session_id());
        assert_eq!(fs::read_to_string(second.active_path()).unwrap(), "");

        let loaded = second.load_all().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].text, "fix me");
        assert_eq!(loaded[0].session_id, first.session_id());
    }

    #[test]
    fn save_all_dual_writes_active_export_only_for_current_session_file_content() {
        let dir = tempfile_dir();
        let store = SessionCommentStore::new(&dir).unwrap();
        let past_id = Uuid::new_v4();
        let past = Comment::new(past_id, PathBuf::from("a.rs"), 1, "h".into(), "old".into());
        let cur = Comment::new(
            store.session_id(),
            PathBuf::from("b.rs"),
            2,
            "h".into(),
            "new".into(),
        );
        store
            .save_all(&[past.clone(), cur.clone()], "=== active only ===\n")
            .unwrap();

        assert_eq!(
            fs::read_to_string(store.active_path()).unwrap(),
            "=== active only ===\n"
        );
        let past_path = dir.join(".rustiq").join(format!("comments-{past_id}.json"));
        let past_loaded: Vec<Comment> =
            serde_json::from_str(&fs::read_to_string(past_path).unwrap()).unwrap();
        assert_eq!(past_loaded.len(), 1);
        assert_eq!(past_loaded[0].text, "old");

        let cur_loaded: Vec<Comment> =
            serde_json::from_str(&fs::read_to_string(store.session_path()).unwrap()).unwrap();
        assert_eq!(cur_loaded.len(), 1);
        assert_eq!(cur_loaded[0].text, "new");
    }

    #[test]
    fn restart_overwrites_active_but_keeps_archives() {
        let dir = tempfile_dir();
        let first = SessionCommentStore::new(&dir).unwrap();
        let c = Comment::new(
            first.session_id(),
            PathBuf::from("x.rs"),
            1,
            "h".into(),
            "one".into(),
        );
        first.save_all(std::slice::from_ref(&c), "session-one\n").unwrap();
        let first_session = first.session_path().to_path_buf();

        let second = SessionCommentStore::new(&dir).unwrap();
        assert_eq!(fs::read_to_string(second.active_path()).unwrap(), "");
        let loaded = second.load_all().unwrap();
        assert_eq!(loaded.len(), 1);
        // App always persists the full loaded set (past + current).
        second.save_all(&loaded, "session-two\n").unwrap();
        assert_eq!(fs::read_to_string(second.active_path()).unwrap(), "session-two\n");
        assert!(first_session.exists());
        let still: Vec<Comment> =
            serde_json::from_str(&fs::read_to_string(&first_session).unwrap()).unwrap();
        assert_eq!(still[0].text, "one");
    }

    fn tempfile_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rustiq-test-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
