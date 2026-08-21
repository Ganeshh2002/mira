//! What Mira says about how commits relate to each other.
//!
//! **This is a picture, not a Git client.** Everything here describes where to
//! draw a line. There is no method that checks anything out, merges, rebases,
//! resets, cherry-picks, creates a commit, stages a path, or talks to a remote,
//! and the absence is structural — a guard test fails the build if a libgit2 write
//! API appears anywhere in this crate's sources (`prd.md` FR-3.4).
//!
//! ## The bound
//!
//! The graph is computed over **the page that is on screen and nothing else**.
//! Lane assignment is a pure function of a window of at most [`crate::PAGE`]
//! commits and their parent ids; it never looks at a commit the reader cannot see.
//!
//! That is the whole reason topological ordering is not used. libgit2's sorted
//! revwalks — `Sort::TOPOLOGICAL` and `Sort::TIME` alike — preprocess the entire
//! reachable history before yielding a single commit, which turns a bounded
//! interface into an unbounded traversal. Measured on this design's own fixtures,
//! that is the difference between a first page costing the same at every
//! repository size and one costing proportionally more the longer the history is
//! (`tests/performance.rs`, and [ADR-0015](../../../docs/adr/0015-graph-lanes.md)).
//!
//! The cost of the bound, stated plainly: rows arrive in the same order the
//! history list uses — the order `git log` prints — so a repository with skewed
//! commit dates can place a parent *above* its child. That relationship is
//! reported as [`EdgeKind::Reordered`] and no line is drawn for it, rather than a
//! line being drawn upwards into a row that is already gone.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::history::CommitId;
use crate::model::{Commit, Head};

/// How many lanes are drawn before the picture collapses.
///
/// `design-system.md` §8: lanes are 1px strokes, at most eight, coloured by index
/// from a muted ramp. Past eight a graph stops being read and starts being
/// decoded, so lanes beyond the eighth are folded onto the last one and the
/// interface says it has done so rather than drawing a thicket.
pub const MAX_LANES: u32 = 8;

/// How many references are examined when labelling a page.
///
/// A reference scan is bounded by how many refs a repository has, not by how long
/// its history is — but "not unbounded by history" is not the same as "small", and
/// a repository with tens of thousands of tags exists. This is the ceiling, and
/// [`CommitGraph::Ready::refs_truncated`] says when it was reached.
pub const MAX_REFS: usize = 500;

/// What a reference is.
///
/// Declared in the order labels are read, because that is the order `Ord` gives
/// and the order a row shows them in: where you are, then the branches, then what
/// a remote had, then tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RefKind {
    /// Where `HEAD` is, which is the marker that makes a detached HEAD visible.
    Head,
    /// A local branch.
    Branch,
    /// A branch on a remote, as of the last fetch. Mira does not fetch.
    Remote,
    /// A tag, lightweight or annotated.
    Tag,
}

/// One label on a row.
///
/// Ordered by kind and then by name, so two reads of one repository label a row
/// the same way round.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GitRef {
    /// What kind of reference it is. First, so ordering is by kind then name.
    pub kind: RefKind,
    /// The short name, as Git would print it: `main`, `origin/main`, `v1.0`.
    pub name: String,
}

/// What one commit is, in the shape a graph reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RowKind {
    /// No parents: the beginning of the history.
    Root,
    /// One parent. The ordinary case.
    Normal,
    /// Two or more parents.
    Merge,
}

impl RowKind {
    /// What a commit with this many parents is.
    #[must_use]
    pub const fn of(parents: usize) -> Self {
        match parents {
            0 => Self::Root,
            1 => Self::Normal,
            _ => Self::Merge,
        }
    }

    /// A word for it, so a row never depends on a shape or a colour alone
    /// (`design-system.md` §5).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Root => "First commit",
            Self::Normal => "Commit",
            Self::Merge => "Merge",
        }
    }
}

/// What a line leaving a commit means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum EdgeKind {
    /// The first parent, continuing straight down this commit's own lane.
    Straight,
    /// The first parent is already carried by another lane, so this one ends
    /// here and bends into it.
    Join,
    /// A second or later parent — the side that was merged in.
    Merge,
    /// The parent sits *above* this row rather than below it.
    ///
    /// Ordinary in a repository with skewed commit dates, and the honest answer
    /// to it: the relationship is reported and no line is drawn, because a line
    /// running upward off a row that has already been read would be a picture of
    /// something that is not there.
    Reordered,
}

/// One line from a commit to a parent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Edge {
    /// The lane the parent occupies.
    pub to: u32,
    /// What the line means.
    pub kind: EdgeKind,
}

/// One row of the graph: a commit, where it sits, and what it connects to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GraphRow {
    /// Everything the history list already showed. The same [`Commit`] type,
    /// read by the same walk — there is no second history here.
    pub commit: Commit,
    /// This commit's parents, in Git's order. The first is the mainline.
    pub parents: Vec<CommitId>,
    /// Which lane the node sits in, counting from the left.
    pub lane: u32,
    /// Root, ordinary, or merge.
    pub kind: RowKind,
    /// Branch and tag labels that point at this commit.
    pub refs: Vec<GitRef>,
    /// The lines leaving this commit, one per parent.
    pub edges: Vec<Edge>,
    /// Lanes still occupied below this row, so the renderer can draw the
    /// verticals that pass it by without recomputing the layout.
    pub continuing: Vec<u32>,
}

/// One page of a repository's history, with the shape of it.
///
/// The same three-state shape as [`crate::CommitPage`], because it answers the
/// same question with more detail: "not a repository" and "cannot be read" are
/// answers the interface renders, not failures a caller has to translate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum CommitGraph {
    /// The directory has no repository.
    NotARepository,

    /// There is a repository, but this page could not be read.
    #[serde(rename_all = "camelCase")]
    Unreadable {
        /// What went wrong, in words a person can act on.
        detail: String,
    },

    /// The page was read. An empty `rows` list is a repository with no commits
    /// yet, which is a state and not a failure.
    #[serde(rename_all = "camelCase")]
    Ready {
        /// Which branch, or which commit, this page is being read from.
        head: Head,
        /// The commits on this page, newest first, in the same order the history
        /// list uses — so the graph sits beside it row for row.
        rows: Vec<GraphRow>,
        /// Where to continue from, or `None` at the end of what is here.
        next: Option<CommitId>,
        /// Whether this is a shallow clone, so the oldest row is where the
        /// *copy* stops rather than where the history does.
        shallow: bool,
        /// How many lanes this page uses, after any collapse.
        lanes: u32,
        /// Whether the page needed more than [`MAX_LANES`] and was folded.
        collapsed: bool,
        /// Whether there were more references than [`MAX_REFS`] to look at, so a
        /// label may be missing from a row that has one.
        refs_truncated: bool,
    },
}
