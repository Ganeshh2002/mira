//! What Mira says about a repository's past.
//!
//! Read-only, like everything else in this crate: there is no method here that
//! checks anything out, moves a ref, or rewrites a commit, and the absence is
//! structural rather than a matter of discipline (`prd.md` FR-3.4).
//!
//! Two ideas shape these types.
//!
//! **History is paged, never whole.** There is no way to ask for a repository's
//! entire log: [`PAGE`] is a constant in this crate, and the caller's only lever is
//! a cursor saying where to continue from. A repository with a million commits
//! costs the same as one with thirty (`architecture.md` §5 rule 3).
//!
//! **Everything here is provider-neutral.** No `git2` type appears in a public
//! signature, so moving to `gitoxide` stays a change inside this crate
//! ([ADR-0009](../../../docs/adr/0009-git-via-libgit2.md)).

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ts_rs::TS;

use crate::model::{Commit, Head};

/// How many commits one page holds.
///
/// A constant here rather than an argument from the caller. The interface asks
/// for *the next page*; it cannot ask for ten thousand commits, because there is
/// no parameter through which it could say so — which is what makes "Mira never
/// walks a whole repository" a property of the boundary rather than a promise
/// about how it is called.
pub const PAGE: usize = 25;

/// A commit id, in the form the interface is allowed to name one.
///
/// A newtype with a validating conversion rather than a `String`, because this is
/// the one Git value that crosses the IPC boundary *inbound*. Anything that is not
/// hexadecimal and of a plausible length fails to deserialise, so a command taking
/// one of these can never be handed a path, a flag, or a refspec — the shapes an
/// attacker would try if Mira ran `git` (it does not; see ADR-0009).
///
/// Abbreviations are allowed, from four characters up, because that is what a
/// person copies and what Mira itself shows.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, TS)]
#[ts(export)]
// Written by hand rather than derived, because the validation *is* the point:
// deserialising is the moment a value from a webview becomes a commit id, and
// anything that is not one has to fail there rather than further in.
pub struct CommitId(#[ts(type = "string")] String);

impl Serialize for CommitId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CommitId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// The shortest abbreviation Git itself will resolve.
const SHORTEST_ID: usize = 4;

/// A full SHA-1 object id, in hex.
const LONGEST_ID: usize = 40;

impl CommitId {
    /// The id as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CommitId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<CommitId> for String {
    fn from(id: CommitId) -> Self {
        id.0
    }
}

impl TryFrom<String> for CommitId {
    type Error = MalformedCommitId;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        if raw.len() < SHORTEST_ID
            || raw.len() > LONGEST_ID
            || !raw.chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(MalformedCommitId);
        }

        // Lower-cased so one commit has one spelling, whichever case it arrived in.
        Ok(Self(raw.to_ascii_lowercase()))
    }
}

impl std::str::FromStr for CommitId {
    type Err = MalformedCommitId;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

/// What arrived was not a commit id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MalformedCommitId;

impl std::fmt::Display for MalformedCommitId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "a commit id is {SHORTEST_ID} to {LONGEST_ID} hexadecimal characters"
        )
    }
}

impl std::error::Error for MalformedCommitId {}

/// One page of a repository's history.
///
/// The same three-state shape as [`crate::GitOverview`]: "not a repository" and
/// "cannot be read" are answers the interface renders, not failures a caller has
/// to translate (`prd.md` FR-3.6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum CommitPage {
    /// The directory has no repository.
    NotARepository,

    /// There is a repository, but this page could not be read.
    #[serde(rename_all = "camelCase")]
    Unreadable {
        /// What went wrong, in words a person can act on.
        detail: String,
    },

    /// The page was read. An empty `commits` list is a repository with no
    /// commits yet, which is a state and not a failure.
    #[serde(rename_all = "camelCase")]
    Ready {
        /// Which branch, or which commit, this history is being read from.
        head: Head,
        /// The commits on this page, newest first.
        commits: Vec<Commit>,
        /// Where to continue from, or `None` at the end of what is here.
        next: Option<CommitId>,
        /// Whether this is a shallow clone, so "the end" is the end of the
        /// *copy* rather than the end of the history.
        shallow: bool,
    },
}

/// One commit, in the detail its own view shows.
///
/// No diff. Slice 5a shows what a commit *is*; what it *changed* is 5b's read-only
/// diff view, and building half of one here would be the speculative structure the
/// roadmap's rule 8 exists to prevent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CommitDetail {
    /// Everything the history row already showed.
    pub commit: Commit,
    /// The rest of the message, when there is more than a subject line.
    pub body: Option<String>,
    /// The author's email as recorded in the commit.
    pub author_email: String,
    /// How many parents it has. Two or more is a merge.
    pub parents: u32,
    /// How many paths it changed against its first parent.
    ///
    /// `None` for a merge, and honestly so: a merge's changed-file count depends
    /// on which parent you compare against, so there is no single true number to
    /// show. It is also `None` if the comparison could not be made.
    pub changed_files: Option<u32>,
}

/// What Mira found when asked for one commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum CommitLookup {
    /// The directory has no repository.
    NotARepository,

    /// There is a repository, but it could not be read.
    #[serde(rename_all = "camelCase")]
    Unreadable {
        /// What went wrong, in words a person can act on.
        detail: String,
    },

    /// No commit with that id is in this repository. Ordinary after a rebase, or
    /// in a shallow clone, so it is a state rather than an error.
    Unknown,

    /// The commit was read.
    #[serde(rename_all = "camelCase")]
    Ready {
        /// What it says about itself.
        commit: CommitDetail,
    },
}

#[cfg(test)]
mod tests {
    use super::CommitId;

    #[test]
    fn a_commit_id_is_hexadecimal_and_bounded() {
        assert!("ad50fc7".parse::<CommitId>().is_ok());
        assert!("ad50fc71f70b122469a9772cb772dc86b2c83521"
            .parse::<CommitId>()
            .is_ok());
        assert!("AD50FC7".parse::<CommitId>().is_ok(), "case is normalised");
    }

    #[test]
    fn one_commit_has_one_spelling() {
        assert_eq!(
            "AD50FC7".parse::<CommitId>().expect("valid"),
            "ad50fc7".parse::<CommitId>().expect("valid")
        );
    }

    #[test]
    fn nothing_that_is_not_a_commit_id_parses() {
        // Revisions, paths, flags and metacharacters — the four shapes anything
        // that took a Git argument would have to defend against. None of them is
        // a commit id, so none survives the wire. The full adversarial battery
        // lives in `src-tauri/tests/wire.rs`, next to the boundary it defends.
        for refused in [
            "",
            "ad5",
            "HEAD",
            "main",
            "HEAD~1",
            "--upload-pack=id",
            "../../../etc/passwd",
            "ad50fc7; rm -rf ~",
            "ad50fc7 --exec=id",
            "$(id)",
            "ad50fc71f70b122469a9772cb772dc86b2c835211",
            "refs/heads/main",
            "zzzzzzz",
        ] {
            assert!(
                refused.parse::<CommitId>().is_err(),
                "{refused} must not parse as a commit id"
            );
        }
    }
}
