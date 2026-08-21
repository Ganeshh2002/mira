//! Reading history through libgit2.
//!
//! Bounded by construction. The walk asks for [`PAGE`] + 1 commits and stops: the
//! extra one is not returned, it is only how the walk learns whether there is a
//! next page and what its cursor is. Nothing here can traverse a whole repository,
//! however large, and nothing here holds more than twenty-six commits in memory at
//! a time.
//!
//! Reads only. `git2::Revwalk` and `git2::Commit` cannot write, and no other API
//! is touched (ADR-0009).

use std::collections::{HashMap, HashSet};

use git2::{ErrorCode, Oid, Repository, Sort};

use crate::graph::{CommitGraph, GitRef, GraphRow, RefKind, RowKind, MAX_REFS};
use crate::history::{CommitDetail, CommitId, CommitLookup, CommitPage, PAGE};
use crate::lanes::{self, Node};
use crate::libgit2::{head, sentence, short};
use crate::model::Commit;

/// One page of history, starting at `from` or at `HEAD`.
pub fn history(repo: &Repository, from: Option<&CommitId>) -> CommitPage {
    let head = match head(repo) {
        Ok(head) => head,
        Err(error) => {
            return CommitPage::Unreadable {
                detail: sentence(&error),
            }
        }
    };

    match page(repo, from) {
        Ok((commits, next)) => CommitPage::Ready {
            head,
            commits,
            next,
            shallow: repo.is_shallow(),
        },
        Err(error) => CommitPage::Unreadable {
            detail: sentence(&error),
        },
    }
}

/// The commits on this page, and the cursor for the next one.
fn page(
    repo: &Repository,
    from: Option<&CommitId>,
) -> Result<(Vec<Commit>, Option<CommitId>), git2::Error> {
    let start = match from {
        Some(id) => match resolve(repo, id) {
            Some(oid) => Some(oid),
            // A cursor naming a commit that is not here — after a rebase, or in a
            // shallow clone that was deepened away — is the end of what can be
            // shown, not a failure to show anything.
            None => return Ok((Vec::new(), None)),
        },
        None => match repo.head().ok().and_then(|head| head.target()) {
            Some(oid) => Some(oid),
            // An unborn branch. There is nothing to walk and nothing is wrong.
            None => return Ok((Vec::new(), None)),
        },
    };

    let mut walk = repo.revwalk()?;
    // `NONE` is not "unsorted": it is libgit2's default, which is the same
    // reverse-chronological order `git log` prints, produced **lazily** one commit
    // at a time.
    //
    // The alternative was `Sort::TIME`, and measuring it is what settled the
    // choice. Asking libgit2 to sort makes the walk preprocess the entire
    // reachable history before yielding anything, so the first page cost 2.5 ms on
    // a hundred commits and 272 ms on ten thousand — an unbounded traversal
    // wearing a bounded interface. With the default it is 0.9 ms at every size
    // (`tests/performance.rs`). Same order, same page, and the promise that a page
    // costs a page becomes true rather than merely intended.
    //
    // `TOPOLOGICAL` is not wanted at all here: 5a's History is a linear list, and
    // the lane graph that needs topology is 5b.
    walk.set_sorting(Sort::NONE)?;

    if let Some(oid) = start {
        walk.push(oid)?;
    }

    // One more than a page: the extra id is the next cursor, and is never shown.
    let mut commits = Vec::with_capacity(PAGE);
    let mut next = None;

    for found in walk.take(PAGE + 1) {
        let oid = match found {
            Ok(oid) => oid,
            // A history with an object the walk cannot reach — a broken clone, a
            // pruned shallow boundary — ends the page rather than failing it. The
            // commits already read are true, and showing them beats showing none.
            Err(_) => break,
        };

        if commits.len() == PAGE {
            next = CommitId::try_from(oid.to_string()).ok();
            break;
        }

        match repo.find_commit(oid) {
            Ok(commit) => commits.push(summarise(&commit)),
            Err(_) => break,
        }
    }

    Ok((commits, next))
}

/// One commit, in the detail its own view shows.
pub fn commit(repo: &Repository, id: &CommitId) -> CommitLookup {
    let Some(oid) = resolve(repo, id) else {
        return CommitLookup::Unknown;
    };

    let found = match repo.find_commit(oid) {
        Ok(found) => found,
        Err(error) if error.code() == ErrorCode::NotFound => return CommitLookup::Unknown,
        Err(error) => {
            return CommitLookup::Unreadable {
                detail: sentence(&error),
            }
        }
    };

    let parents = u32::try_from(found.parent_count()).unwrap_or(u32::MAX);
    let author_email = String::from_utf8_lossy(found.author().email_bytes()).into_owned();

    CommitLookup::Ready {
        commit: CommitDetail {
            commit: summarise(&found),
            body: body(&found),
            author_email,
            parents,
            changed_files: changed_files(repo, &found),
        },
    }
}

/// The object this id names, if this repository has it.
///
/// `revparse` is deliberately **not** used: it accepts `HEAD`, `main@{2}`,
/// `v1.0^{tree}` and a great deal else, and the whole point of [`CommitId`] is
/// that the interface names an object id and nothing else. This resolves an
/// abbreviation to a full id and refuses everything that is not one.
fn resolve(repo: &Repository, id: &CommitId) -> Option<Oid> {
    let object = repo.revparse_single(id.as_str()).ok()?;
    // `revparse_single` on a hex string can only produce an object id, and
    // `peel_to_commit` is what turns an annotated tag into the commit it names.
    // Anything that is not a commit — a tree, a blob — is not a commit id.
    object.peel_to_commit().ok().map(|commit| commit.id())
}

/// The parts of a commit every surface shows.
pub(crate) fn summarise(commit: &git2::Commit<'_>) -> Commit {
    let sha = commit.id().to_string();
    let author = commit.author();

    Commit {
        short_sha: short(&sha),
        sha,
        // `_bytes` and a lossy decode, because a commit message is bytes and a
        // Latin-1 one must render, not panic (`prd.md` feature 4 risks).
        subject: String::from_utf8_lossy(commit.summary_bytes().unwrap_or_default())
            .trim()
            .to_owned(),
        author: String::from_utf8_lossy(author.name_bytes()).into_owned(),
        committed_at: commit.time().seconds(),
    }
}

/// Everything after the subject line, when there is anything.
fn body(commit: &git2::Commit<'_>) -> Option<String> {
    let body = String::from_utf8_lossy(commit.body_bytes().unwrap_or_default())
        .trim()
        .to_owned();

    (!body.is_empty()).then_some(body)
}

/// How many paths this commit changed, against its first parent.
///
/// A tree-to-tree comparison: file names and modes, never file contents, so the
/// cost is proportional to the changed part of the tree rather than to the size of
/// the repository. `None` for a merge — see [`CommitDetail::changed_files`] — and
/// `None` whenever libgit2 declines, because a detail panel missing one number is
/// better than a detail panel that failed.
fn changed_files(repo: &Repository, commit: &git2::Commit<'_>) -> Option<u32> {
    if commit.parent_count() > 1 {
        return None;
    }

    let new = commit.tree().ok()?;
    let old = match commit.parent(0) {
        Ok(parent) => Some(parent.tree().ok()?),
        // A root commit has no parent, so everything in it is new.
        Err(_) => None,
    };

    let diff = repo
        .diff_tree_to_tree(old.as_ref(), Some(&new), None)
        .ok()?;

    u32::try_from(diff.deltas().len()).ok()
}

// ── The graph ────────────────────────────────────────────────────────────────

/// One page of history, with the shape of it.
///
/// Reads **the same page** as [`history`], through the same bounded walk, and
/// adds two things: each commit's parent ids, which are already in the commit
/// object and cost nothing extra, and the references that point into the window.
/// There is no second traversal and no second history — the graph is the history,
/// with the relationships kept.
pub fn graph(repo: &Repository, from: Option<&CommitId>) -> CommitGraph {
    let head = match head(repo) {
        Ok(head) => head,
        Err(error) => {
            return CommitGraph::Unreadable {
                detail: sentence(&error),
            }
        }
    };

    let (commits, next) = match page(repo, from) {
        Ok(page) => page,
        Err(error) => {
            return CommitGraph::Unreadable {
                detail: sentence(&error),
            }
        }
    };

    // Parent ids come from the commit objects the walk already loaded. A commit
    // whose parents cannot be read is kept with none rather than dropped: a row
    // missing a line is better than a history missing a commit.
    let nodes: Vec<Node> = commits
        .iter()
        .map(|commit| Node {
            id: parse(&commit.sha),
            parents: parents_of(repo, &commit.sha),
        })
        .collect();

    let laid = lanes::layout(&nodes);
    let window: HashSet<String> = commits.iter().map(|commit| commit.sha.clone()).collect();
    let (mut labels, refs_truncated) = refs_in(repo, &window);

    let rows = commits
        .into_iter()
        .zip(nodes)
        .zip(laid.rows)
        .map(|((commit, node), placement)| GraphRow {
            kind: RowKind::of(node.parents.len()),
            refs: labels.remove(&commit.sha).unwrap_or_default(),
            parents: node.parents,
            lane: placement.lane,
            edges: placement.edges,
            continuing: placement.continuing,
            commit,
        })
        .collect();

    CommitGraph::Ready {
        head,
        rows,
        next,
        shallow: repo.is_shallow(),
        lanes: laid.lanes,
        collapsed: laid.collapsed,
        refs_truncated,
    }
}

/// A commit's parent ids, in Git's order.
///
/// An id that will not parse is dropped rather than propagated: every id libgit2
/// hands back is forty hex characters, so this cannot happen — and if it somehow
/// did, one missing line is a better answer than a failed page.
fn parents_of(repo: &Repository, sha: &str) -> Vec<CommitId> {
    let Ok(oid) = Oid::from_str(sha) else {
        return Vec::new();
    };
    let Ok(commit) = repo.find_commit(oid) else {
        return Vec::new();
    };

    commit
        .parent_ids()
        .filter_map(|parent| CommitId::try_from(parent.to_string()).ok())
        .collect()
}

/// A forty-character id libgit2 produced, as a [`CommitId`].
///
/// Infallible in practice for the same reason as above; a malformed id would
/// simply fail to match anything and its row would be laid out as a tip.
fn parse(sha: &str) -> CommitId {
    CommitId::try_from(sha.to_owned()).unwrap_or_else(|_| {
        CommitId::try_from("0".repeat(40)).expect("forty zeroes is a well-formed id")
    })
}

/// The branch and tag labels that point at commits in `window`.
///
/// Bounded by [`MAX_REFS`], and the flag says when that ceiling was reached. A
/// reference scan is bounded by how many refs a repository has rather than by how
/// long its history is — but a repository with tens of thousands of tags exists,
/// and "not unbounded by history" is not the same as "small".
///
/// Reads references. Never writes one: there is no branch created, moved, or
/// deleted anywhere in this crate, and a guard test fails the build if a libgit2
/// write API appears in its sources.
fn refs_in(repo: &Repository, window: &HashSet<String>) -> (HashMap<String, Vec<GitRef>>, bool) {
    let mut labels: HashMap<String, Vec<GitRef>> = HashMap::new();

    let Ok(references) = repo.references() else {
        // A repository whose refs cannot be listed still has a readable history,
        // and an unlabelled graph beats no graph.
        return (labels, false);
    };

    let mut truncated = false;

    for (seen, reference) in references.flatten().enumerate() {
        if seen >= MAX_REFS {
            // More references exist than Mira looks at. Said out loud rather
            // than swallowed: a row that has a label may be shown without one.
            truncated = true;
            break;
        }

        let kind = if reference.is_tag() {
            RefKind::Tag
        } else if reference.is_remote() {
            RefKind::Remote
        } else if reference.is_branch() {
            RefKind::Branch
        } else {
            // HEAD, notes, stash, and anything else a repository keeps. Only
            // branches, remotes and tags label a row.
            continue;
        };

        // Peeled, so an annotated tag lands on the commit it names rather than on
        // the tag object, which is in no history.
        let Ok(commit) = reference.peel_to_commit() else {
            continue;
        };
        let sha = commit.id().to_string();
        if !window.contains(&sha) {
            continue;
        }

        let Ok(name) = reference.shorthand() else {
            continue;
        };

        labels.entry(sha).or_default().push(GitRef {
            kind,
            name: name.to_owned(),
        });
    }

    // Where HEAD is, which is the marker that makes a detached HEAD visible.
    if let Some(sha) = repo
        .head()
        .ok()
        .and_then(|head| head.target())
        .map(|oid| oid.to_string())
    {
        if window.contains(&sha) {
            labels.entry(sha).or_default().push(GitRef {
                kind: RefKind::Head,
                name: "HEAD".to_owned(),
            });
        }
    }

    // Sorted so two reads of one repository label a row in the same order.
    for row in labels.values_mut() {
        row.sort();
        row.dedup();
    }

    (labels, truncated)
}
