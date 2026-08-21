//! The libgit2-backed provider.
//!
//! Reads only. Nothing here writes a ref, an object, or an index entry, and
//! nothing opens a network transport — `git2` is built with its HTTPS and SSH
//! features off, so the transports are not compiled in at all (ADR-0009).

use std::path::{Path, PathBuf};

use git2::{BranchType, ErrorCode, Repository, Status, StatusOptions};

use crate::diff::{ChangedFiles, DiffScope, FileDiff};
use crate::graph::CommitGraph;
use crate::history::{CommitId, CommitLookup, CommitPage};
use crate::model::{Commit, GitOverview, Head, Upstream};
use crate::provider::GitProvider;
use crate::{patch, walk};

/// How many hex characters an abbreviated commit id gets.
const SHORT_SHA: usize = 7;

/// Git through libgit2.
#[derive(Debug, Clone, Copy, Default)]
pub struct Libgit2;

impl GitProvider for Libgit2 {
    fn discover(&self, start: &Path) -> Option<PathBuf> {
        // `open_ext` walks up from `start`. Ceiling directories are empty so the
        // walk stops at a filesystem boundary, which is what `git rev-parse
        // --show-toplevel` does (FR-1.3).
        let repo = Repository::open_ext(start, git2::RepositoryOpenFlags::empty(), &[] as &[&Path])
            .ok()?;
        repo.workdir().map(Path::to_path_buf)
    }

    fn overview(&self, root: &Path) -> GitOverview {
        let repo =
            match Repository::open_ext(root, git2::RepositoryOpenFlags::empty(), &[] as &[&Path]) {
                Ok(repo) => repo,
                // libgit2 reports a `.git` it cannot parse the same way it reports no
                // `.git` at all. Telling someone with a visible `.git` directory that
                // this is "not a repository" would read as a bug in Mira, so the two
                // cases are separated here.
                Err(error) if error.code() == ErrorCode::NotFound => {
                    return if root.join(".git").exists() {
                        GitOverview::Unreadable {
                            detail: sentence(&error),
                        }
                    } else {
                        GitOverview::NotARepository
                    }
                }
                Err(error) => {
                    return GitOverview::Unreadable {
                        detail: sentence(&error),
                    }
                }
            };

        match read(&repo) {
            Ok(overview) => overview,
            Err(error) => GitOverview::Unreadable {
                detail: sentence(&error),
            },
        }
    }

    fn history(&self, root: &Path, from: Option<&CommitId>) -> CommitPage {
        match open(root) {
            Ok(repo) => walk::history(&repo, from),
            Err(Absent::NotARepository) => CommitPage::NotARepository,
            Err(Absent::Unreadable(detail)) => CommitPage::Unreadable { detail },
        }
    }

    fn commit(&self, root: &Path, id: &CommitId) -> CommitLookup {
        match open(root) {
            Ok(repo) => walk::commit(&repo, id),
            Err(Absent::NotARepository) => CommitLookup::NotARepository,
            Err(Absent::Unreadable(detail)) => CommitLookup::Unreadable { detail },
        }
    }

    fn graph(&self, root: &Path, from: Option<&CommitId>) -> CommitGraph {
        match open(root) {
            Ok(repo) => walk::graph(&repo, from),
            Err(Absent::NotARepository) => CommitGraph::NotARepository,
            Err(Absent::Unreadable(detail)) => CommitGraph::Unreadable { detail },
        }
    }

    fn changed_files(&self, root: &Path, scope: &DiffScope) -> ChangedFiles {
        match open(root) {
            Ok(repo) => patch::changed_files(&repo, scope),
            Err(Absent::NotARepository) => ChangedFiles::NotARepository,
            Err(Absent::Unreadable(detail)) => ChangedFiles::Unreadable { detail },
        }
    }

    fn file_diff(&self, root: &Path, scope: &DiffScope, at: u32) -> FileDiff {
        match open(root) {
            Ok(repo) => patch::file_diff(&repo, scope, at),
            Err(Absent::NotARepository) => FileDiff::NotARepository,
            Err(Absent::Unreadable(detail)) => FileDiff::Unreadable { detail },
        }
    }
}

/// Why there is no repository to read.
enum Absent {
    /// Nothing is here, which is a neutral state rather than a failure.
    NotARepository,
    /// Something is here and libgit2 could not make sense of it.
    Unreadable(String),
}

/// Open the repository containing `root`, telling the two absences apart.
///
/// libgit2 reports a `.git` it cannot parse the same way it reports no `.git` at
/// all. Telling someone with a visible `.git` directory that this is "not a
/// repository" would read as a bug in Mira, so the two cases are separated here
/// once, for every reader in this file.
fn open(root: &Path) -> Result<Repository, Absent> {
    match Repository::open_ext(root, git2::RepositoryOpenFlags::empty(), &[] as &[&Path]) {
        Ok(repo) => Ok(repo),
        Err(error) if error.code() == ErrorCode::NotFound => {
            if root.join(".git").exists() {
                Err(Absent::Unreadable(sentence(&error)))
            } else {
                Err(Absent::NotARepository)
            }
        }
        Err(error) => Err(Absent::Unreadable(sentence(&error))),
    }
}

fn read(repo: &Repository) -> Result<GitOverview, git2::Error> {
    let head = head(repo)?;
    let last_commit = last_commit(repo)?;
    let (clean, changed) = worktree(repo)?;
    let upstream = upstream(repo);

    Ok(GitOverview::Ready {
        head,
        clean,
        changed,
        last_commit,
        upstream,
    })
}

pub(crate) fn head(repo: &Repository) -> Result<Head, git2::Error> {
    match repo.head() {
        Ok(reference) => {
            if reference.is_branch() {
                if let Ok(name) = reference.shorthand() {
                    return Ok(Head::Branch {
                        name: name.to_owned(),
                    });
                }
            }
            Ok(Head::Detached {
                sha: reference
                    .target()
                    .map(|oid| short(&oid.to_string()))
                    .unwrap_or_default(),
            })
        }
        // An initialised repository with no commits: HEAD names a branch that does
        // not exist yet. That is FR-4.3's "no commits yet", not a failure.
        Err(error) if error.code() == ErrorCode::UnbornBranch => Ok(Head::Unborn),
        Err(error) if error.code() == ErrorCode::NotFound => Ok(Head::Unborn),
        Err(error) => Err(error),
    }
}

fn last_commit(repo: &Repository) -> Result<Option<Commit>, git2::Error> {
    let Some(oid) = repo.head().ok().and_then(|head| head.target()) else {
        return Ok(None);
    };
    let commit = repo.find_commit(oid)?;
    let sha = oid.to_string();
    let author = commit.author();

    Ok(Some(Commit {
        short_sha: short(&sha),
        sha,
        // `_bytes` and a lossy decode, because a commit message is bytes and a
        // Latin-1 one must render, not panic (`prd.md` feature 4 risks).
        subject: String::from_utf8_lossy(commit.summary_bytes().unwrap_or_default())
            .trim()
            .to_owned(),
        author: String::from_utf8_lossy(author.name_bytes()).into_owned(),
        committed_at: commit.time().seconds(),
    }))
}

fn worktree(repo: &Repository) -> Result<(bool, u32), git2::Error> {
    if repo.is_bare() {
        // A bare repository has no working tree, so there is nothing to be dirty.
        return Ok((true, 0));
    }

    let mut options = StatusOptions::new();
    options
        .include_untracked(true)
        // One entry per untracked directory rather than one per file inside it:
        // a fresh `node_modules` should not count as forty thousand changes.
        .recurse_untracked_dirs(false)
        .include_ignored(false)
        // Submodule contents are not recursed in 0.1 (FR-3.7).
        .exclude_submodules(true);

    let statuses = repo.statuses(Some(&mut options))?;
    let changed = statuses
        .iter()
        .filter(|entry| entry.status() != Status::CURRENT)
        .count();

    Ok((changed == 0, u32::try_from(changed).unwrap_or(u32::MAX)))
}

/// The tracked branch and the distance to it, computed locally.
///
/// Mira does not fetch (FR-5.2), so this is the distance as of the user's last
/// fetch and the interface says so. Anything missing — no upstream, a deleted
/// remote ref — is simply no upstream, never an error.
fn upstream(repo: &Repository) -> Option<Upstream> {
    let head = repo.head().ok()?;
    if !head.is_branch() {
        return None;
    }
    let branch = repo
        .find_branch(head.shorthand().ok()?, BranchType::Local)
        .ok()?;

    let tracked = branch.upstream().ok()?;
    let name = tracked.name().ok().flatten()?.to_owned();
    let (ahead, behind) = repo
        .graph_ahead_behind(head.target()?, tracked.get().target()?)
        .ok()?;

    Some(Upstream {
        name,
        ahead: u32::try_from(ahead).unwrap_or(u32::MAX),
        behind: u32::try_from(behind).unwrap_or(u32::MAX),
    })
}

pub(crate) fn short(sha: &str) -> String {
    sha.chars().take(SHORT_SHA).collect()
}

/// libgit2's message as one sentence a person can read.
pub(crate) fn sentence(error: &git2::Error) -> String {
    let message = error.message().trim();
    let mut chars = message.chars();
    let capitalised = match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Git could not read this repository".to_owned(),
    };

    if capitalised.ends_with('.') {
        capitalised
    } else {
        capitalised + "."
    }
}
