//! What Mira says about what changed.
//!
//! **A view, never an edit.** Nothing here stages, checks out, applies, reverts,
//! commits, cherry-picks, merges, rebases, resets, or reaches a remote. The
//! absence is structural: a guard test names every libgit2 write API and fails the
//! build if one appears in this crate (`prd.md` FR-3.4,
//! [ADR-0016](../../../docs/adr/0016-bounded-diffs.md)).
//!
//! ## Every read here is bounded, and says when the bound bit
//!
//! A diff is the first read in Mira whose size is set by *the repository's files*
//! rather than by a page of history. A single commit can touch ten thousand
//! paths; a single path can be a forty-megabyte minified bundle on one line. So
//! there are five limits, all constants in this crate, none of them nameable by a
//! caller:
//!
//! | Limit | Constant | Value |
//! |---|---|---|
//! | Files listed per change set | [`MAX_FILES`] | 200 |
//! | Lines rendered per file | [`MAX_LINES`] | 2 000 |
//! | Bytes of patch text per file | [`MAX_BYTES`] | 256 KiB |
//! | Bytes on one line | [`MAX_LINE_BYTES`] | 2 000 |
//! | Blob size Mira will diff at all | [`MAX_FILE_BYTES`] | 2 MiB |
//!
//! **Nothing is truncated silently.** Reaching a limit produces a value —
//! [`FilesTruncated`], [`PatchTruncated`] — carrying what was shown and what the
//! limit was, and the interface renders it as a sentence. A diff that quietly
//! stopped short would be worse than no diff, because it would look complete.
//!
//! A binary file is *identified*, never decoded: [`FileDiff::Binary`] carries the
//! two sizes and no text.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::history::CommitId;

/// How many changed files one change set lists.
///
/// A commit that touches more than this is real — a dependency bump, a formatting
/// pass, a vendored directory — and the list says so rather than scrolling for a
/// minute.
pub const MAX_FILES: usize = 200;

/// How many lines of one file's patch are rendered.
pub const MAX_LINES: usize = 2_000;

/// How many bytes of one file's patch text are returned.
///
/// The line cap alone is not a byte cap: two thousand lines of a minified bundle
/// is still megabytes. Both are checked, and whichever bites first is reported.
pub const MAX_BYTES: usize = 256 * 1024;

/// How much of a single line is kept.
///
/// A generated file can be one line of several megabytes. Cutting the line keeps
/// the diff readable *and* keeps one pathological path from spending the whole
/// byte budget; the interface marks a line that was cut.
pub const MAX_LINE_BYTES: usize = 2_000;

/// The largest blob Mira will diff at all.
///
/// Past this, [`FileDiff::TooLarge`] says so and no content is read. Also handed
/// to libgit2 as its own `max_size`, so a huge blob is never materialised even to
/// be rejected.
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// What happened to a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ChangeKind {
    /// The path is new.
    Added,
    /// The path is gone.
    Deleted,
    /// The path is the same and its contents are not.
    Modified,
    /// The path moved, with or without edits.
    Renamed,
    /// The path was copied from another.
    Copied,
    /// A file became a symlink, or a submodule became a directory.
    TypeChanged,
}

impl ChangeKind {
    /// One letter, as `git status` writes it.
    ///
    /// A companion to the word, never a replacement for it: the interface shows
    /// both, because `C` and `M` are indistinguishable to somebody who has not
    /// memorised them (`design-system.md` §5).
    #[must_use]
    pub const fn letter(self) -> &'static str {
        match self {
            Self::Added => "A",
            Self::Deleted => "D",
            Self::Modified => "M",
            Self::Renamed => "R",
            Self::Copied => "C",
            Self::TypeChanged => "T",
        }
    }

    /// The word the interface reads out.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Added => "Added",
            Self::Deleted => "Deleted",
            Self::Modified => "Modified",
            Self::Renamed => "Renamed",
            Self::Copied => "Copied",
            Self::TypeChanged => "Type changed",
        }
    }
}

/// One changed path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FileChange {
    /// Where this change sits in the change set.
    ///
    /// **This is how a file is asked for.** The interface names an ordinal in a
    /// list Mira produced, never a path of its own — so there is no argument
    /// through which a page could ask Mira to read a file it did not already
    /// decide to offer (`security-and-privacy.md` §5).
    pub at: u32,
    /// What happened to it.
    pub kind: ChangeKind,
    /// The path, relative to the repository root, with `/` separators.
    pub path: String,
    /// Where it came from, for a rename or a copy.
    pub from_path: Option<String>,
    /// Whether Git considers it binary. Binary files are identified, not decoded.
    pub binary: bool,
    /// Lines added, when they were counted.
    ///
    /// `None` for a binary file and for one too large to diff — there is no line
    /// count to give, and inventing a zero would read as "nothing changed".
    pub additions: Option<u32>,
    /// Lines removed, on the same terms.
    pub deletions: Option<u32>,
}

/// Which set of changes is being asked for.
///
/// Two shapes, because there are two questions: *what did this commit change* and
/// *what have I changed since*. They are kept apart deliberately — a working tree
/// is not a commit, and showing them in one list would make it impossible to tell
/// what is recorded from what is merely on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum DiffScope {
    /// What one commit changed.
    #[serde(rename_all = "camelCase")]
    Commit {
        /// Which commit. Validated as it deserialises, like everywhere else.
        commit: CommitId,
    },
    /// What the working tree has that `HEAD` does not, staged or not.
    WorkingTree,
}

/// What a change set was compared against.
///
/// Said out loud, because for a merge the answer would be different against the
/// other parent and a diff that did not say so would be quietly picking a side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum Comparison {
    /// A root commit, compared against nothing: everything in it is new.
    EmptyTree,
    /// The ordinary case: one commit against its parent.
    Parent,
    /// A merge, against its **first** parent.
    #[serde(rename_all = "camelCase")]
    FirstParent {
        /// How many parents it has, so the interface can say which was chosen.
        parents: u32,
    },
    /// The working tree against `HEAD`.
    Head,
}

/// What a change list left out, if anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum FilesTruncated {
    /// Everything that changed is listed.
    No,
    /// More paths changed than Mira lists.
    #[serde(rename_all = "camelCase")]
    Yes {
        /// How many are in the list.
        shown: u32,
        /// How many changed altogether.
        total: u32,
        /// The ceiling that bit ([`MAX_FILES`]).
        limit: u32,
    },
}

/// What a patch left out, if anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum PatchTruncated {
    /// The whole patch is here.
    No,
    /// It has more lines than Mira renders.
    #[serde(rename_all = "camelCase")]
    Lines {
        /// How many lines are here.
        shown: u32,
        /// The ceiling that bit ([`MAX_LINES`]).
        limit: u32,
    },
    /// It is larger than Mira returns.
    #[serde(rename_all = "camelCase")]
    Bytes {
        /// How many bytes are here.
        #[ts(type = "number")]
        shown: u64,
        /// The ceiling that bit ([`MAX_BYTES`]).
        #[ts(type = "number")]
        limit: u64,
    },
}

/// The changed paths of one commit, or of the working tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum ChangedFiles {
    /// The directory has no repository.
    NotARepository,

    /// There is a repository, but this could not be read.
    #[serde(rename_all = "camelCase")]
    Unreadable {
        /// What went wrong, in words a person can act on.
        detail: String,
    },

    /// No commit with that id is here. Ordinary after a rebase, or in a shallow
    /// clone, so it is a state rather than an error.
    Unknown,

    /// The change set was read. An empty `files` list is an empty commit, which
    /// is legal and is a state rather than a failure.
    #[serde(rename_all = "camelCase")]
    Ready {
        /// The paths, in Git's own order.
        files: Vec<FileChange>,
        /// What they were compared against.
        against: Comparison,
        /// Whether the list stops short, and where.
        truncated: FilesTruncated,
    },
}

/// One line of a patch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LineKind {
    /// Unchanged, shown for context.
    Context,
    /// Added by this change.
    Addition,
    /// Removed by this change.
    Deletion,
    /// Git's own note — "\\ No newline at end of file".
    Note,
}

/// One line of a patch, with the numbers that let it be read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiffLine {
    /// Added, removed, context, or a note.
    pub kind: LineKind,
    /// The line number on the old side, where there is one.
    pub old_line: Option<u32>,
    /// The line number on the new side, where there is one.
    pub new_line: Option<u32>,
    /// The text, without its trailing newline.
    ///
    /// Decoded lossily, because a file is bytes and a Latin-1 one must render
    /// rather than panic. A line longer than [`MAX_LINE_BYTES`] is cut, and
    /// `cut` says so.
    pub text: String,
    /// Whether this line was cut at [`MAX_LINE_BYTES`].
    pub cut: bool,
}

/// One run of changed lines, with its surrounding context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Hunk {
    /// Git's own header, `@@ -a,b +c,d @@`, for a boundary a reader recognises.
    pub header: String,
    /// The lines in it.
    pub lines: Vec<DiffLine>,
}

/// What one file's change looks like.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum FileDiff {
    /// The directory has no repository.
    NotARepository,

    /// There is a repository, but this could not be read.
    #[serde(rename_all = "camelCase")]
    Unreadable {
        /// What went wrong, in words a person can act on.
        detail: String,
    },

    /// No such commit, or no such change in it. A stale selection, which is a
    /// state rather than a failure.
    Unknown,

    /// Git considers this file binary. Identified, never decoded.
    #[serde(rename_all = "camelCase")]
    Binary {
        /// Which path, and what happened to it.
        change: FileChange,
        /// Its size before, where there was a before.
        #[ts(type = "number | null")]
        old_bytes: Option<u64>,
        /// Its size after, where there is an after.
        #[ts(type = "number | null")]
        new_bytes: Option<u64>,
    },

    /// Bigger than [`MAX_FILE_BYTES`], so nothing was read.
    #[serde(rename_all = "camelCase")]
    TooLarge {
        /// Which path, and what happened to it.
        change: FileChange,
        /// How big the larger side is.
        #[ts(type = "number")]
        bytes: u64,
        /// The ceiling that bit.
        #[ts(type = "number")]
        limit: u64,
    },

    /// The patch was read. An empty `hunks` list is a change with no textual
    /// difference — a mode change, or a pure rename.
    #[serde(rename_all = "camelCase")]
    Ready {
        /// Which path, and what happened to it.
        change: FileChange,
        /// The patch.
        hunks: Vec<Hunk>,
        /// Whether it stops short, and where.
        truncated: PatchTruncated,
    },
}
