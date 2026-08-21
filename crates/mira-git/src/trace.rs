//! Tracing one file through a repository's history.
//!
//! **A view, never an edit**, like everything else in this crate.
//!
//! ## The measurement that shapes this file
//!
//! File history is the one read in Mira that is *inherently* O(repository
//! history), and no cleverness removes that. To know whether a commit touched a
//! path you have to look at that commit; to find the commits that touched it you
//! have to look at all of them. `git log -- <path>` does exactly this.
//!
//! Measured before the design was chosen, on a repository where one file changes
//! every fiftieth commit (release build, macOS):
//!
//! | commits | scanned | time | per commit |
//! |---|---|---|---|
//! | 1 000 | 1 000 | 23 ms | 23 µs |
//! | 10 000 | 10 000 | 289 ms | 29 µs |
//! | 40 000 | 40 000 | **2 981 ms** | 75 µs |
//!
//! Three seconds for one file in a middling repository, and worse in a large one.
//! So the walk is **bounded by commits examined** rather than by anything about
//! the file: [`MAX_SCAN`] per request, measured at ~47 ms, with the rest reachable
//! by continuing. A page that stops early says how far it got
//! ([ADR-0017](../../../docs/adr/0017-file-history.md)).
//!
//! ## Renames are cheap; the traversal is not
//!
//! The same measurement answered the second question. A rename-detecting diff of
//! one commit costs **0.03 ms**, and a rename is a rare event rather than a
//! per-commit cost — so following renames adds nothing measurable. What it does
//! *not* do is help with the traversal, which is where the time goes.
//!
//! The walk therefore compares one path's blob id against the first parent's, two
//! tree lookups per commit, and only runs a rename-detecting diff at the commit
//! where the path appears — which is the one place a rename can be hiding.
//!
//! ## No path ever crosses the IPC boundary
//!
//! A file is named by a [`FileSubject`]: a change set Mira produced, an ordinal in
//! it, and whether to take the name from before or after that change. The
//! interface receives one and echoes it back; it never builds one. That is how a
//! walk that follows a file across renames keeps `security-and-privacy.md` §5
//! rule 8 intact.

use std::path::Path;

use git2::{Oid, Repository, Sort};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::diff::{ChangeKind, ChangedFiles, DiffScope};
use crate::history::{CommitId, PAGE};
use crate::libgit2::sentence;
use crate::model::Commit;
use crate::patch;

/// How many commits one request will examine.
///
/// The whole bound, and it is about *commits looked at* rather than anything
/// about the file — because looking is the cost. Measured at ~47 ms for two
/// thousand, against 281 ms for ten thousand: past this a click stops feeling
/// like one. What is not reached in a page is reached by continuing.
pub const MAX_SCAN: usize = 2_000;

/// Which file, said without ever naming a path.
///
/// A change set Mira produced, a position in it, and which side of that change to
/// take the name from. The interface receives one of these and hands it back; it
/// has no way to construct a different one, and no way to describe a file Mira has
/// not already offered it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileSubject {
    /// The change set the file was chosen from.
    pub scope: DiffScope,
    /// Its position in that change set.
    pub at: u32,
    /// Take the name the file had **before** this change rather than after.
    ///
    /// Set when a page ends on a rename, so the next page continues under the old
    /// name — which is how rename following survives pagination without a path
    /// ever being sent.
    pub before: bool,
}

/// Where to continue a trace, and under what name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileCursor {
    /// The file, as of the commit the walk stopped at.
    pub subject: FileSubject,
    /// The commit to resume examining at.
    pub from: CommitId,
}

/// One commit that touched the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileCommit {
    /// The commit, in the same shape every other surface shows one.
    pub commit: Commit,
    /// The name the file had at this commit.
    pub path: String,
    /// What happened to it here.
    pub kind: ChangeKind,
    /// The name it had before this commit renamed it, when this is where the
    /// trace crossed a rename.
    pub renamed_from: Option<String>,
}

/// Why a trace stopped where it did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum ScanStopped {
    /// It reached the end of what this repository holds.
    No,

    /// It ran out of commits to examine before filling a page.
    ///
    /// Says how many were looked at, so "nothing found" is never confused with
    /// "nothing there" — the difference matters most for exactly the files this
    /// bound exists for.
    #[serde(rename_all = "camelCase")]
    Budget {
        /// How many commits were examined.
        scanned: u32,
        /// The ceiling that bit ([`MAX_SCAN`]).
        limit: u32,
    },

    /// A rename was found and could not be followed past it.
    ///
    /// Rare: it needs the renaming commit to have touched more paths than Mira
    /// lists at once. The history continues under a name Mira did not find, and
    /// saying so beats stopping as if the file began there.
    #[serde(rename_all = "camelCase")]
    RenameLost {
        /// The name the trace was following when it lost the thread.
        path: String,
    },
}

/// The commits that touched one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum FileHistory {
    /// The directory has no repository.
    NotARepository,

    /// There is a repository, but this could not be read.
    #[serde(rename_all = "camelCase")]
    Unreadable {
        /// What went wrong, in words a person can act on.
        detail: String,
    },

    /// No such commit, or no such change in it — a stale selection.
    Unknown,

    /// The trace ran. An empty `commits` list with `stopped: No` means the file
    /// genuinely has no history above this point, which is a state and not a
    /// failure.
    #[serde(rename_all = "camelCase")]
    Ready {
        /// The name the file has at the point it was asked about.
        path: String,
        /// The commits that touched it, newest first.
        commits: Vec<FileCommit>,
        /// Where to continue, or `None` at the end of what is here.
        next: Option<FileCursor>,
        /// How many commits were examined to produce this page.
        scanned: u32,
        /// Why it stopped.
        stopped: ScanStopped,
        /// Whether this is a shallow clone, so the end is the end of the *copy*.
        shallow: bool,
    },
}

/// Trace one file back through history.
pub fn file_history(
    repo: &Repository,
    subject: &FileSubject,
    from: Option<&CommitId>,
) -> FileHistory {
    let Some(mut path) = name_of(repo, subject) else {
        return FileHistory::Unknown;
    };
    let asked_about = path.clone();

    let start = match from {
        Some(id) => match resolve(repo, id) {
            Some(oid) => oid,
            None => return FileHistory::Unknown,
        },
        None => match repo.head().ok().and_then(|head| head.target()) {
            Some(oid) => oid,
            // An unborn branch: no history, and nothing is wrong.
            None => {
                return FileHistory::Ready {
                    path: asked_about,
                    commits: Vec::new(),
                    next: None,
                    scanned: 0,
                    stopped: ScanStopped::No,
                    shallow: repo.is_shallow(),
                }
            }
        },
    };

    let mut walk = match start_walk(repo, start) {
        Ok(walk) => walk,
        Err(error) => {
            return FileHistory::Unreadable {
                detail: sentence(&error),
            }
        }
    };

    let mut commits: Vec<FileCommit> = Vec::with_capacity(PAGE);
    let mut scanned: usize = 0;
    let mut stopped = ScanStopped::No;
    let mut last: Option<(Oid, bool)> = None;
    let mut ran_out = false;

    loop {
        if commits.len() == PAGE {
            break;
        }
        if scanned >= MAX_SCAN {
            stopped = ScanStopped::Budget {
                scanned: count(scanned),
                limit: count(MAX_SCAN),
            };
            break;
        }

        let Some(next) = walk.next() else {
            ran_out = true;
            break;
        };
        let Ok(oid) = next else {
            // A history with an object the walk cannot reach ends the trace
            // rather than failing it — what was found is true.
            ran_out = true;
            break;
        };
        scanned += 1;

        let Ok(commit) = repo.find_commit(oid) else {
            ran_out = true;
            break;
        };

        let Some(touch) = touched(&commit, &path) else {
            continue;
        };

        // The path appearing is the one place a rename can hide, so it is the
        // only place a rename-detecting diff is run — 0.03 ms, once per rename
        // rather than once per commit.
        let mut kind = touch;
        let mut renamed_from = None;
        if touch == ChangeKind::Added {
            match came_from(repo, &commit, &path) {
                Origin::Renamed(previous) => {
                    kind = ChangeKind::Renamed;
                    renamed_from = Some(previous);
                }
                Origin::Copied(previous) => {
                    kind = ChangeKind::Copied;
                    renamed_from = Some(previous);
                }
                Origin::New => {}
                Origin::Lost => {
                    stopped = ScanStopped::RenameLost { path: path.clone() };
                }
            }
        }

        commits.push(FileCommit {
            commit: patch::summarise_commit(&commit),
            path: path.clone(),
            kind,
            renamed_from: renamed_from.clone(),
        });
        last = Some((oid, renamed_from.is_some()));

        if let Some(previous) = renamed_from {
            // Everything below this commit knows the file by its old name.
            path = previous;
        }

        if stopped != ScanStopped::No {
            break;
        }
    }

    // The resume point is the next commit the walk *would* have examined, taken
    // without examining it — so continuing neither repeats work nor skips a
    // commit.
    let resume = if ran_out || stopped == (ScanStopped::RenameLost { path: path.clone() }) {
        None
    } else {
        walk.next().and_then(Result::ok)
    };

    let next = resume.and_then(|oid| {
        let subject = match last {
            // Anchored on the last commit that touched the file, because that is
            // a change set Mira produced and can name a position in.
            Some((matched, renamed)) => anchor(repo, matched, &path, renamed)?,
            // Nothing touched the file, so nothing renamed it, so the name the
            // caller gave still applies below.
            None => subject.clone(),
        };

        Some(FileCursor {
            subject,
            from: CommitId::try_from(oid.to_string()).ok()?,
        })
    });

    FileHistory::Ready {
        path: asked_about,
        commits,
        next,
        scanned: count(scanned),
        stopped,
        shallow: repo.is_shallow(),
    }
}

/// The walk this trace runs over.
///
/// `Sort::NONE` for the reason [ADR-0015](../../../docs/adr/0015-graph-lanes.md)
/// gives: a sorted revwalk preprocesses the whole reachable history before
/// yielding anything, which is the opposite of what a bounded scan wants.
fn start_walk(repo: &Repository, start: Oid) -> Result<git2::Revwalk<'_>, git2::Error> {
    let mut walk = repo.revwalk()?;
    walk.set_sorting(Sort::NONE)?;
    walk.push(start)?;
    Ok(walk)
}

/// Whether this commit changed `path`, and how.
///
/// Two tree lookups and an id comparison — no diff, no content. That is what
/// makes a scan of two thousand commits cost forty-seven milliseconds rather than
/// forty-seven seconds.
///
/// A merge is compared against its **first** parent, which is the same choice
/// `diff.rs` makes and for the same reason: against the other parent the answer
/// would differ, and picking one silently would be worse than picking one and
/// saying so.
pub(crate) fn touched(commit: &git2::Commit<'_>, path: &str) -> Option<ChangeKind> {
    let subject = Path::new(path);

    let new = commit
        .tree()
        .ok()
        .and_then(|tree| tree.get_path(subject).ok())
        .map(|entry| entry.id());

    let old = match commit.parent(0) {
        Ok(parent) => parent
            .tree()
            .ok()
            .and_then(|tree| tree.get_path(subject).ok())
            .map(|entry| entry.id()),
        // A root commit is compared against nothing.
        Err(_) => None,
    };

    match (old, new) {
        (None, Some(_)) => Some(ChangeKind::Added),
        (Some(_), None) => Some(ChangeKind::Deleted),
        (Some(before), Some(after)) if before != after => Some(ChangeKind::Modified),
        _ => None,
    }
}

/// Where a path that appears in this commit came from.
pub(crate) enum Origin {
    /// It is genuinely new here.
    New,
    /// It was renamed from another name.
    Renamed(String),
    /// It was copied from another name.
    Copied(String),
    /// This commit touched more paths than Mira lists, so the answer is not
    /// findable without an unbounded read.
    Lost,
}

/// Ask one commit whether it renamed `path` into existence.
///
/// Reuses the bounded change list `diff.rs` already builds, which is what keeps
/// this one call rather than a second Git implementation. Measured at 0.03 ms.
pub(crate) fn came_from(repo: &Repository, commit: &git2::Commit<'_>, path: &str) -> Origin {
    let Ok(id) = CommitId::try_from(commit.id().to_string()) else {
        return Origin::New;
    };

    let ChangedFiles::Ready {
        files, truncated, ..
    } = patch::changed_files(repo, &DiffScope::Commit { commit: id })
    else {
        return Origin::New;
    };

    match files.iter().find(|change| change.path == path) {
        Some(change) => match (change.kind, change.from_path.clone()) {
            (ChangeKind::Renamed, Some(previous)) => Origin::Renamed(previous),
            (ChangeKind::Copied, Some(previous)) => Origin::Copied(previous),
            _ => Origin::New,
        },
        // Not in the list. If the list was complete, the path really is new here
        // — libgit2 simply did not call it a rename. If the list stopped short,
        // the answer is somewhere Mira did not look, and saying so beats
        // pretending the file began here.
        None => match truncated {
            crate::diff::FilesTruncated::No => Origin::New,
            crate::diff::FilesTruncated::Yes { .. } => Origin::Lost,
        },
    }
}

/// Express "this file, at this commit" as a subject the caller can hand back.
///
/// The file is in this commit's change list by construction — it is there because
/// this commit touched it — so its position in that list names it without a path
/// being written down anywhere.
pub(crate) fn anchor(
    repo: &Repository,
    commit: Oid,
    path: &str,
    renamed: bool,
) -> Option<FileSubject> {
    let id = CommitId::try_from(commit.to_string()).ok()?;
    let scope = DiffScope::Commit { commit: id };

    let ChangedFiles::Ready { files, .. } = patch::changed_files(repo, &scope) else {
        return None;
    };

    // After a rename the walk is already following the old name, so the entry to
    // look for is the one that *came from* it.
    let found = files.iter().find(|change| {
        if renamed {
            change.from_path.as_deref() == Some(path)
        } else {
            change.path == path
        }
    })?;

    Some(FileSubject {
        scope,
        at: found.at,
        before: renamed,
    })
}

/// The path a subject names, resolved through the bounded change list.
pub(crate) fn name_of(repo: &Repository, subject: &FileSubject) -> Option<String> {
    let ChangedFiles::Ready { files, .. } = patch::changed_files(repo, &subject.scope) else {
        return None;
    };

    let change = files.iter().find(|change| change.at == subject.at)?;

    if subject.before {
        change
            .from_path
            .clone()
            .or_else(|| Some(change.path.clone()))
    } else {
        Some(change.path.clone())
    }
}

/// The object this id names, if this repository has it.
pub(crate) fn resolve(repo: &Repository, id: &CommitId) -> Option<Oid> {
    let object = repo.revparse_single(id.as_str()).ok()?;
    object.peel_to_commit().ok().map(|commit| commit.id())
}

/// A count as the wire carries it.
fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
