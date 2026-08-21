//! Lane assignment: a pure function over a window of commits.
//!
//! No repository, no I/O, no clock. It takes commit ids and parent ids and
//! returns where to draw. That is what makes every shape a repository can have —
//! a merge, a root, a fork, a history whose dates disagree with its topology —
//! assertable from a table of ids rather than from a fixture on disk
//! (`architecture.md` §10, and [ADR-0015](../../../docs/adr/0015-graph-lanes.md)).
//!
//! ## The bound
//!
//! It sees **only the window it is given** — at most [`crate::PAGE`] commits — and
//! keeps one reservation per open lane. Cost is proportional to the rows on
//! screen and the lanes among them, never to the repository behind them.
//!
//! ## The one thing it must never do
//!
//! Draw a line to somewhere that is not there. Two cases arise from reading a
//! window in date order rather than topological order:
//!
//! - a parent that is **below** the window — the line runs off the bottom, which
//!   is true, and the next page continues it;
//! - a parent that is **above** this row, because the dates disagree with the
//!   topology — reported as [`EdgeKind::Reordered`] with no line, because a line
//!   drawn upward into a row already scrolled past is a picture of nothing.

use std::collections::HashSet;

use crate::graph::{Edge, EdgeKind, MAX_LANES};
use crate::history::CommitId;

/// What the layout needs to know about one commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    /// The commit.
    pub id: CommitId,
    /// Its parents, in Git's order. The first is the mainline.
    pub parents: Vec<CommitId>,
}

/// Where one commit sits, and what leaves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    /// The lane the node is drawn in.
    pub lane: u32,
    /// One line per parent.
    pub edges: Vec<Edge>,
    /// Lanes still occupied below this row, so the renderer can draw the
    /// verticals that pass it by without recomputing anything.
    pub continuing: Vec<u32>,
}

/// A laid-out window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    /// One placement per input node, in the same order.
    pub rows: Vec<Placement>,
    /// How many lanes are used, after any collapse.
    pub lanes: u32,
    /// Whether the window needed more than [`MAX_LANES`] and was folded onto it.
    pub collapsed: bool,
}

/// Lay out a window of commits.
///
/// Deterministic: the same window always produces the same picture, which is what
/// stops the graph shifting under a reader who pressed Refresh.
#[must_use]
pub fn layout(nodes: &[Node]) -> Layout {
    // Lane `i` is reserved for the commit id it holds — the next commit expected
    // to appear there. `None` is a free lane.
    let mut lanes: Vec<Option<CommitId>> = Vec::new();
    let mut emitted: HashSet<CommitId> = HashSet::with_capacity(nodes.len());
    let mut rows: Vec<Placement> = Vec::with_capacity(nodes.len());
    let mut widest: usize = 0;

    for node in nodes {
        // The lane already waiting for this commit, or a new one. A commit with
        // no reservation is a tip: nothing in the window has it as a parent.
        let lane = match reserved_for(&lanes, &node.id) {
            Some(found) => found,
            None => reserve(&mut lanes, &node.id),
        };

        // Two children of one parent both reserve it, and it appears once. The
        // duplicate reservations are released here so a lane is never left
        // waiting for a commit that has already gone past.
        for (index, held) in lanes.iter_mut().enumerate() {
            if index != lane && held.as_ref() == Some(&node.id) {
                *held = None;
            }
        }

        emitted.insert(node.id.clone());

        // The lane is free unless a parent claims it below.
        lanes[lane] = None;
        let mut edges = Vec::with_capacity(node.parents.len());

        for (position, parent) in node.parents.iter().enumerate() {
            let first = position == 0;

            if emitted.contains(parent) {
                // Above this row. Reported, not drawn.
                edges.push(Edge {
                    to: lane_index(lane),
                    kind: EdgeKind::Reordered,
                });
                continue;
            }

            if let Some(found) = reserved_for(&lanes, parent) {
                // Another lane is already carrying this parent, so this line
                // bends into it rather than a second lane being opened for the
                // same commit.
                edges.push(Edge {
                    to: lane_index(found),
                    kind: if first {
                        EdgeKind::Join
                    } else {
                        EdgeKind::Merge
                    },
                });
                continue;
            }

            if first {
                // The mainline keeps the lane, which is what makes a long history
                // read as one straight line with side branches beside it.
                lanes[lane] = Some(parent.clone());
                edges.push(Edge {
                    to: lane_index(lane),
                    kind: EdgeKind::Straight,
                });
            } else {
                let slot = reserve(&mut lanes, parent);
                edges.push(Edge {
                    to: lane_index(slot),
                    kind: EdgeKind::Merge,
                });
            }
        }

        // Trailing free lanes are dropped so the picture is only as wide as it
        // needs to be; a branch that ended does not leave a permanent gutter.
        while matches!(lanes.last(), Some(None)) {
            lanes.pop();
        }

        widest = widest.max(lanes.len()).max(lane + 1);
        rows.push(Placement {
            lane: lane_index(lane),
            edges,
            continuing: lanes
                .iter()
                .enumerate()
                .filter_map(|(index, held)| held.as_ref().map(|_| lane_index(index)))
                .collect(),
        });
    }

    let used = u32::try_from(widest).unwrap_or(u32::MAX);
    if used > MAX_LANES {
        collapse(&mut rows);
        return Layout {
            rows,
            lanes: MAX_LANES,
            collapsed: true,
        };
    }

    Layout {
        rows,
        lanes: used,
        collapsed: false,
    }
}

/// Fold everything past [`MAX_LANES`] onto the last drawn lane.
///
/// `design-system.md` §8 caps the picture at eight lanes: past that a graph stops
/// being read and starts being decoded. Folding is honest in a way that dropping
/// rows would not be — every commit is still on screen and still connected; only
/// the horizontal position stops being distinct, and the interface says so.
fn collapse(rows: &mut [Placement]) {
    let last = MAX_LANES - 1;
    for row in rows.iter_mut() {
        row.lane = row.lane.min(last);
        for edge in &mut row.edges {
            edge.to = edge.to.min(last);
        }
        for lane in &mut row.continuing {
            *lane = (*lane).min(last);
        }
        row.continuing.sort_unstable();
        row.continuing.dedup();
    }
}

/// The lane currently reserved for `id`, if any.
fn reserved_for(lanes: &[Option<CommitId>], id: &CommitId) -> Option<usize> {
    lanes.iter().position(|held| held.as_ref() == Some(id))
}

/// Reserve the leftmost free lane for `id`, opening one if none is free.
///
/// Leftmost, always: it is what keeps the mainline at lane 0 and makes two reads
/// of the same window produce the same picture.
fn reserve(lanes: &mut Vec<Option<CommitId>>, id: &CommitId) -> usize {
    match lanes.iter().position(Option::is_none) {
        Some(free) => {
            lanes[free] = Some(id.clone());
            free
        }
        None => {
            lanes.push(Some(id.clone()));
            lanes.len() - 1
        }
    }
}

/// A lane index as the wire carries it.
///
/// Saturating rather than wrapping: a window cannot produce four billion lanes —
/// it holds at most [`crate::PAGE`] commits — so this is a total conversion for an
/// impossible case, not a silent truncation of a real one.
fn lane_index(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::{layout, Node};
    use crate::graph::{EdgeKind, MAX_LANES};
    use crate::history::CommitId;

    /// A distinct commit id per character, so the tables below stay readable.
    ///
    /// Derived from the character's code point rather than repeated, because a
    /// commit id is hexadecimal and `g` is not.
    fn id(letter: char) -> CommitId {
        format!("{:04x}", letter as u32)
            .parse()
            .expect("four hex characters")
    }

    /// `node('c', "ba")` — commit `c` with parents `b` then `a`.
    fn node(commit: char, parents: &str) -> Node {
        Node {
            id: id(commit),
            parents: parents.chars().map(id).collect(),
        }
    }

    fn lanes_of(nodes: &[Node]) -> Vec<u32> {
        layout(nodes).rows.iter().map(|row| row.lane).collect()
    }

    // ── Linear history ───────────────────────────────────────────────────────

    #[test]
    fn a_straight_history_is_one_lane() {
        // c → b → a, newest first. Nothing forks, so nothing moves sideways.
        let laid = layout(&[node('c', "b"), node('b', "a"), node('a', "")]);

        assert_eq!(
            lanes_of(&[node('c', "b"), node('b', "a"), node('a', "")]),
            [0, 0, 0]
        );
        assert_eq!(laid.lanes, 1);
        assert!(!laid.collapsed);
        for row in &laid.rows[..2] {
            assert_eq!(row.edges.len(), 1);
            assert_eq!(row.edges[0].kind, EdgeKind::Straight);
            assert_eq!(row.edges[0].to, 0);
        }
    }

    #[test]
    fn a_root_commit_ends_its_lane() {
        let laid = layout(&[node('b', "a"), node('a', "")]);

        let root = laid.rows.last().expect("a root row");
        assert!(root.edges.is_empty(), "a root commit has nothing below it");
        assert!(
            root.continuing.is_empty(),
            "and leaves no lane running off the bottom"
        );
    }

    #[test]
    fn a_lane_that_reaches_no_parent_in_the_window_runs_off_the_bottom() {
        // `a`'s parent is outside the window: the next page continues it, so the
        // lane stays open rather than being closed as if the history ended.
        let laid = layout(&[node('b', "a"), node('a', "f")]);

        let last = laid.rows.last().expect("a row");
        assert_eq!(last.edges[0].kind, EdgeKind::Straight);
        assert_eq!(
            last.continuing,
            [0],
            "the line leaves the window rather than stopping"
        );
    }

    // ── Branches ─────────────────────────────────────────────────────────────

    #[test]
    fn a_second_tip_opens_a_second_lane() {
        // Two tips, `d` and `c`, both descending from `b`. Neither is reserved
        // when it appears, so each takes a lane of its own.
        let laid = layout(&[
            node('d', "b"),
            node('c', "b"),
            node('b', "a"),
            node('a', ""),
        ]);

        assert_eq!(laid.rows[0].lane, 0);
        assert_eq!(laid.rows[1].lane, 1);
        assert_eq!(laid.lanes, 2);
    }

    #[test]
    fn two_children_of_one_parent_collapse_onto_one_lane() {
        // `d` reserves `b` in lane 0. `c` also has `b` as a parent, so its line
        // bends into lane 0 instead of a second lane being opened for one commit.
        let laid = layout(&[
            node('d', "b"),
            node('c', "b"),
            node('b', "a"),
            node('a', ""),
        ]);

        assert_eq!(laid.rows[1].edges.len(), 1);
        assert_eq!(laid.rows[1].edges[0].kind, EdgeKind::Join);
        assert_eq!(
            laid.rows[1].edges[0].to, 0,
            "into the lane already carrying b"
        );
        assert_eq!(laid.rows[2].lane, 0, "and b appears there");
    }

    #[test]
    fn a_lane_freed_by_a_branch_ending_is_used_again() {
        // `d` is a tip whose parent is already carried, so lane 1 closes at row 1
        // and the next tip reuses it rather than the picture growing for ever.
        let laid = layout(&[
            node('e', "c"),
            node('d', "c"),
            node('c', "b"),
            node('b', "a"),
            node('a', ""),
        ]);

        assert_eq!(laid.lanes, 2, "two lanes are enough for this shape");
    }

    // ── Merges ───────────────────────────────────────────────────────────────

    #[test]
    fn a_merge_opens_a_lane_for_its_second_parent() {
        // `d` merges `c` into `b`. The first parent keeps the lane; the side that
        // was merged in gets one of its own.
        let laid = layout(&[
            node('d', "bc"),
            node('b', "a"),
            node('c', "a"),
            node('a', ""),
        ]);

        let merge = &laid.rows[0];
        assert_eq!(merge.edges.len(), 2);
        assert_eq!(merge.edges[0].kind, EdgeKind::Straight);
        assert_eq!(merge.edges[0].to, 0, "the mainline keeps the lane");
        assert_eq!(merge.edges[1].kind, EdgeKind::Merge);
        assert_eq!(merge.edges[1].to, 1, "the merged side gets its own");

        assert_eq!(laid.rows[1].lane, 0, "b is on the mainline");
        assert_eq!(laid.rows[2].lane, 1, "c is beside it");
    }

    #[test]
    fn the_two_sides_of_a_merge_rejoin_at_their_common_parent() {
        let laid = layout(&[
            node('d', "bc"),
            node('b', "a"),
            node('c', "a"),
            node('a', ""),
        ]);

        // `b` reserves `a` in lane 0; `c` finds it already carried and bends in.
        assert_eq!(laid.rows[1].edges[0].kind, EdgeKind::Straight);
        assert_eq!(laid.rows[2].edges[0].kind, EdgeKind::Join);
        assert_eq!(laid.rows[2].edges[0].to, 0);
        assert_eq!(laid.rows[3].lane, 0, "and a is where both lines lead");
        assert_eq!(laid.lanes, 2);
    }

    #[test]
    fn an_octopus_merge_opens_a_lane_per_extra_parent() {
        // Three parents. Rare, legal, and no reason for the layout to be
        // surprised by it.
        let laid = layout(&[node('e', "bcd")]);

        let merge = &laid.rows[0];
        assert_eq!(merge.edges.len(), 3);
        assert_eq!(merge.edges[0].kind, EdgeKind::Straight);
        assert_eq!(merge.edges[1].kind, EdgeKind::Merge);
        assert_eq!(merge.edges[2].kind, EdgeKind::Merge);
        assert_eq!(
            merge.continuing,
            [0, 1, 2],
            "all three lines leave the window"
        );
    }

    // ── History whose dates disagree with its topology ───────────────────────

    #[test]
    fn a_parent_above_its_child_is_reported_and_not_drawn() {
        // What a repository with skewed commit dates produces: `a` was committed
        // with a later timestamp than its child `b`, so date order puts it first.
        // Drawing a line upward into a row already read would be a picture of
        // something that is not there.
        let laid = layout(&[node('a', ""), node('b', "a")]);

        let child = &laid.rows[1];
        assert_eq!(child.edges.len(), 1);
        assert_eq!(child.edges[0].kind, EdgeKind::Reordered);
        assert!(
            child.continuing.is_empty(),
            "and no lane is left open for a commit that has already gone past"
        );
    }

    #[test]
    fn a_window_of_unrelated_commits_is_laid_out_without_complaint() {
        // Every commit a tip, nothing connected — what a badly broken or heavily
        // filtered history looks like. It must still produce a picture.
        let laid = layout(&[node('a', ""), node('b', ""), node('c', "")]);

        assert_eq!(
            lanes_of(&[node('a', ""), node('b', ""), node('c', "")]),
            [0, 0, 0]
        );
        assert_eq!(laid.lanes, 1, "each lane closes before the next opens");
        assert!(laid.rows.iter().all(|row| row.edges.is_empty()));
    }

    #[test]
    fn a_commit_that_is_its_own_parent_does_not_hang() {
        // Not producible by Git, but the layout is handed ids from a repository
        // and must be total over anything that arrives. It has been emitted by
        // the time its parents are read, so it reads as reordered.
        let laid = layout(&[node('a', "a")]);

        assert_eq!(laid.rows[0].edges[0].kind, EdgeKind::Reordered);
    }

    #[test]
    fn a_repeated_parent_is_only_given_one_lane() {
        let laid = layout(&[node('c', "bb"), node('b', "a")]);

        assert_eq!(laid.rows[0].edges[0].kind, EdgeKind::Straight);
        assert_eq!(
            laid.rows[0].edges[1].kind,
            EdgeKind::Merge,
            "the second mention bends into the lane the first opened"
        );
        assert_eq!(laid.rows[0].edges[1].to, laid.rows[0].edges[0].to);
        assert_eq!(laid.lanes, 1);
    }

    #[test]
    fn an_empty_window_lays_out_to_nothing() {
        let laid = layout(&[]);

        assert!(laid.rows.is_empty());
        assert_eq!(laid.lanes, 0);
        assert!(!laid.collapsed);
    }

    // ── The cap ──────────────────────────────────────────────────────────────

    /// `count` tips, each with a parent of its own, so every one needs a lane
    /// and none is ever shared.
    fn widening(count: u32) -> Vec<Node> {
        (0..count)
            .map(|index| Node {
                id: id(char::from_u32('a' as u32 + index).expect("a character")),
                parents: vec![id(char::from_u32('A' as u32 + index).expect("a character"))],
            })
            .collect()
    }

    #[test]
    fn a_window_wider_than_the_cap_is_folded_onto_the_last_lane() {
        // Ten tips wanting ten lanes; eight are drawn.
        let laid = layout(&widening(10));

        assert!(laid.collapsed, "the picture was folded");
        assert_eq!(laid.lanes, MAX_LANES);
        assert!(
            laid.rows.iter().all(|row| row.lane < MAX_LANES),
            "no node is drawn outside the picture"
        );
        assert!(
            laid.rows
                .iter()
                .all(|row| row.edges.iter().all(|edge| edge.to < MAX_LANES)),
            "and no line points outside it"
        );
        assert!(
            laid.rows
                .iter()
                .all(|row| row.continuing.iter().all(|lane| *lane < MAX_LANES)),
            "nor does any vertical"
        );
    }

    #[test]
    fn folding_never_loses_a_row() {
        let laid = layout(&widening(12));

        assert_eq!(laid.rows.len(), 12, "every commit is still on screen");
        assert!(laid.collapsed);
    }

    #[test]
    fn a_window_exactly_at_the_cap_is_not_folded() {
        // The off-by-one: eight lanes is the picture, not one past it.
        let laid = layout(&widening(MAX_LANES));

        assert_eq!(laid.lanes, MAX_LANES);
        assert!(!laid.collapsed);
    }

    // ── Determinism ──────────────────────────────────────────────────────────

    #[test]
    fn the_same_window_always_draws_the_same_picture() {
        // A graph that shifted between two reads of the same commits would make
        // Refresh feel like something changed when nothing did.
        let window = [
            node('e', "cd"),
            node('c', "b"),
            node('d', "b"),
            node('b', "a"),
            node('a', ""),
        ];

        assert_eq!(layout(&window), layout(&window));
    }

    #[test]
    fn every_row_gets_exactly_one_edge_per_parent() {
        // The invariant the renderer relies on: a line for every relationship,
        // and no line for anything else.
        let window = [
            node('e', "cd"),
            node('c', "b"),
            node('d', "b"),
            node('b', "a"),
            node('a', ""),
        ];

        let laid = layout(&window);
        for (row, source) in laid.rows.iter().zip(window.iter()) {
            assert_eq!(row.edges.len(), source.parents.len());
        }
    }
}
