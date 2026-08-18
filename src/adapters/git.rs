use std::cell::RefCell;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use git2::{Delta, DiffOptions, Repository, Sort};
use similar::{ChangeTag, TextDiff};

use crate::domain::{Baseline, DiffFile, DiffLine, DiffLineKind, FileStatus, Hunk};
use crate::ports::GitRepository;

pub struct Git2Repository {
    repo: Repository,
}

impl Git2Repository {
    pub fn open(path: &Path) -> Result<Self> {
        let repo = Repository::discover(path).context("not a git repository")?;
        Ok(Self { repo })
    }
}

impl GitRepository for Git2Repository {
    fn log(&self) -> Result<Vec<Baseline>> {
        let mut walk = self.repo.revwalk()?;
        walk.push_head()?;
        walk.set_sorting(Sort::TIME)?;

        let mut baselines = vec![Baseline::WorkingTree];
        for oid in walk.take(200) {
            let oid = oid?;
            let commit = self.repo.find_commit(oid)?;
            let summary = commit.summary().unwrap_or("").to_string();
            baselines.push(Baseline::Commit { oid: oid.to_string(), summary });
        }
        Ok(baselines)
    }

    fn diff(&self, baseline: &Baseline) -> Result<Vec<DiffFile>> {
        match baseline {
            Baseline::WorkingTree => self.diff_working_tree(),
            Baseline::Commit { oid, .. } => self.diff_commit(oid),
        }
    }

    fn read_lines(&self, path: &PathBuf) -> Result<Vec<String>> {
        let workdir = self.repo.workdir().context("bare repo")?;
        let full = workdir.join(path);
        let content = std::fs::read_to_string(&full)
            .with_context(|| format!("reading {}", full.display()))?;
        Ok(content.lines().map(str::to_string).collect())
    }
}

impl Git2Repository {
    fn diff_working_tree(&self) -> Result<Vec<DiffFile>> {
        let mut opts = DiffOptions::new();
        opts.include_untracked(true).recurse_untracked_dirs(true);

        let head_tree = self.repo.head().ok().and_then(|h| h.peel_to_tree().ok());
        let staged = self.repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts))?;
        let unstaged = self.repo.diff_index_to_workdir(None, Some(&mut opts))?;

        let mut files = parse_git2_diff(&staged)?;
        let unstaged_files = parse_git2_diff(&unstaged)?;
        for uf in unstaged_files {
            if !files.iter().any(|f| f.path == uf.path) {
                files.push(uf);
            }
        }
        Ok(files)
    }

    fn diff_commit(&self, oid_str: &str) -> Result<Vec<DiffFile>> {
        let oid = git2::Oid::from_str(oid_str)?;
        let commit = self.repo.find_commit(oid)?;
        let tree = commit.tree()?;
        let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());

        let diff = self.repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
        parse_git2_diff(&diff)
    }
}

fn delta_to_status(delta: Delta) -> FileStatus {
    match delta {
        Delta::Added | Delta::Untracked => FileStatus::Added,
        Delta::Deleted => FileStatus::Deleted,
        Delta::Renamed | Delta::Copied => FileStatus::Renamed,
        _ => FileStatus::Modified,
    }
}

fn parse_git2_diff(diff: &git2::Diff) -> Result<Vec<DiffFile>> {
    // Use RefCell to work around the multiple-closure borrow limitation of foreach
    let files: RefCell<Vec<DiffFile>> = RefCell::new(Vec::new());

    diff.foreach(
        &mut |delta, _| {
            let status = delta_to_status(delta.status());
            let path = delta.new_file().path().unwrap_or(Path::new("")).to_path_buf();
            let old_path = if matches!(delta.status(), Delta::Renamed | Delta::Copied) {
                delta.old_file().path().map(Path::to_path_buf)
            } else {
                None
            };
            files.borrow_mut().push(DiffFile { path, old_path, status, hunks: vec![] });
            true
        },
        None,
        Some(&mut |delta, hunk| {
            let path = delta.new_file().path().unwrap_or(Path::new("")).to_path_buf();
            let mut fs = files.borrow_mut();
            if let Some(f) = fs.iter_mut().find(|f| f.path == path) {
                f.hunks.push(Hunk {
                    header: String::from_utf8_lossy(hunk.header()).to_string(),
                    lines: vec![],
                });
            }
            true
        }),
        Some(&mut |delta, _hunk, line| {
            let path = delta.new_file().path().unwrap_or(Path::new("")).to_path_buf();
            let kind = match line.origin() {
                '+' => DiffLineKind::Added,
                '-' => DiffLineKind::Removed,
                _ => DiffLineKind::Context,
            };
            let content = String::from_utf8_lossy(line.content()).trim_end_matches('\n').to_string();
            let mut fs = files.borrow_mut();
            if let Some(f) = fs.iter_mut().find(|f| f.path == path) {
                // auto-create a hunk if diff has lines without a hunk header (e.g. untracked files)
                if f.hunks.is_empty() {
                    f.hunks.push(Hunk { header: String::new(), lines: vec![] });
                }
                if let Some(h) = f.hunks.last_mut() {
                    h.lines.push(DiffLine {
                        kind,
                        old_lineno: line.old_lineno(),
                        new_lineno: line.new_lineno(),
                        content,
                    });
                }
            }
            true
        }),
    )?;

    Ok(files.into_inner())
}

/// Diff two strings line-by-line using `similar`. Used for ad-hoc comparisons.
pub fn file_diff_similar(old: &str, new: &str) -> Vec<Hunk> {
    let diff = TextDiff::from_lines(old, new);
    diff.grouped_ops(3)
        .iter()
        .map(|group| {
            let lines = group.iter().flat_map(|op| {
                diff.iter_changes(op).map(|change| DiffLine {
                    kind: match change.tag() {
                        ChangeTag::Insert => DiffLineKind::Added,
                        ChangeTag::Delete => DiffLineKind::Removed,
                        ChangeTag::Equal => DiffLineKind::Context,
                    },
                    old_lineno: change.old_index().map(|i| i as u32 + 1),
                    new_lineno: change.new_index().map(|i| i as u32 + 1),
                    content: change.value().trim_end_matches('\n').to_string(),
                })
            }).collect();
            Hunk { header: String::new(), lines }
        })
        .collect()
}
