//! Filtering history: by branch, author, subject text, or file.
//!
//! **A search, never an edit.** Nothing here checks out, fetches, merges, resets
//! or writes anything, and ADR-0015's guard — every libgit2 write API absent from
//! this crate — covers it unchanged.
//!
//! ## What the measurement said
//!
//! Every filter costs the same, because none of them is the cost. Measured on
//! repositories of one and ten thousand commits (release build, macOS):
//!
//! | filter | 1 000 | 10 000 |
//! |---|---|---|
//! | none — the walk alone | 21.9 µs/commit | 22.9 µs/commit |
//! | author | 19.2 | 22.1 |
//! | subject text | 19.1 | 25.0 |
//! | file touched | 19.2 | 23.3 |
//! | all three at once | 19.1 | 22.6 |
//!
//! Loading the commit object dominates, and the walk does that anyway. Author and
//! subject are then fields already in memory; a file test is two tree lookups
//! against a warm cache. So there is **one budget for every filter**
//! ([`MAX_FILTER_SCAN`]) rather than a table of them, and no cheapest-first
//! ordering — measured, the combination costs what the walk costs
//! ([ADR-0018](../../../docs/adr/0018-history-filters.md)).
//!
//! The same measurement settled a correctness question. A byte-wise ASCII
//! subject matcher was 5 % faster than `to_lowercase`, and is wrong for every
//! language that is not English. Five per cent does not buy that.
//!
//! ## Nothing here becomes a Git argument
//!
//! - **Branch** is a [`CommitId`] — a tip Mira listed, validated as hexadecimal
//!   like every other commit the interface may name. The walk starts there. There
//!   is no ref *string* on the wire at all.
//! - **Author** and **subject** are values that are only ever *compared*, in Rust,
//!   against fields of a commit already in memory. Neither reaches libgit2.
//! - **File** is a [`FileSubject`] — 5d's identity model, a change set Mira
//!   produced and a position in it. No path, no pathspec, no glob.
//!
//! Mira links libgit2 and builds no command line anywhere (ADR-0009). These types
//! are the second wall.

use git2::{Repository, Sort};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use ts_rs::TS;

use crate::diff::ChangeKind;
use crate::graph::{GitRef, RefKind, MAX_REFS};
use crate::history::{CommitId, PAGE};
use crate::libgit2::sentence;
use crate::model::{Commit, Head};
use crate::trace::{self, FileSubject, ScanStopped};
use crate::walk;

/// How many commits one filtered request will examine.
///
/// The same shape of bound as a file trace, and for the same reason: a search
/// over history is proportional to the history, and the only honest way to keep a
/// click feeling like one is to stop looking and say so. Measured at ~45 ms for
/// two thousand commits, whichever filters are on.
pub const MAX_FILTER_SCAN: usize = 2_000;

/// How many distinct authors are offered for choosing from.
pub const MAX_AUTHORS: usize = 100;

/// The longest a filter value may be.
///
/// An author name and a search phrase are both short by nature. The cap is here
/// so that "these are values, not payloads" is enforced rather than assumed.
pub const LONGEST_TERM: usize = 200;

/// A short piece of text a filter compares against.
///
/// Validated as it deserialises, like [`CommitId`]: bounded, single-line, not
/// empty. It is never handed to Git — it is compared in Rust against a field of a
/// commit already in memory — but the type is what makes that a property of the
/// boundary rather than of the code behind it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, TS)]
#[ts(export)]
pub struct Term(#[ts(type = "string")] String);

impl Term {
    /// The text as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The text, lower-cased once for repeated comparison.
    #[must_use]
    pub fn folded(&self) -> String {
        self.0.to_lowercase()
    }
}

impl std::fmt::Display for Term {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for Term {
    type Error = MalformedTerm;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        let trimmed = raw.trim();

        if trimmed.is_empty()
            || trimmed.chars().count() > LONGEST_TERM
            || trimmed.chars().any(char::is_control)
        {
            return Err(MalformedTerm);
        }

        Ok(Self(trimmed.to_owned()))
    }
}

impl std::str::FromStr for Term {
    type Err = MalformedTerm;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

impl Serialize for Term {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Term {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// What arrived was not a filter term.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MalformedTerm;

impl std::fmt::Display for MalformedTerm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "a filter is one line of 1 to {LONGEST_TERM} characters")
    }
}

impl std::error::Error for MalformedTerm {}

/// What to narrow the history to.
///
/// Composable: every field that is set has to match. All four unset is plain
/// history, which the default view reads through `history` instead — this is the
/// narrowed question, and it costs more to ask.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryFilter {
    /// Start the walk at this tip instead of `HEAD`.
    ///
    /// A commit id rather than a ref name, so there is no string here that could
    /// ever be read as a revision expression. The interface picks a ref from
    /// [`known_refs`] and sends the tip it was given.
    pub branch: Option<CommitId>,

    /// Keep commits whose author's name contains this, ignoring case.
    pub author: Option<Term>,

    /// Keep commits whose **subject line** contains this, ignoring case.
    ///
    /// Substring, not a pattern: no globbing, no regular expressions, no word
    /// boundaries. The comparison is Unicode-lowercase on both sides, the query
    /// is trimmed, and the body of the message is not searched.
    pub subject: Option<Term>,

    /// Keep commits that touched this file.
    ///
    /// 5d's identity model, unchanged: a change set Mira produced and a position
    /// in it. Renames are followed, exactly as they are in a file trace.
    pub file: Option<FileSubject>,
}

impl HistoryFilter {
    /// Whether this narrows anything at all.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.branch.is_none()
            && self.author.is_none()
            && self.subject.is_none()
            && self.file.is_none()
    }
}

/// Where to continue a filtered search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FilterCursor {
    /// The commit to resume examining at.
    pub from: CommitId,
    /// The file, as of the resume point, when a file filter is on.
    ///
    /// Carried for the same reason a file trace carries it: a rename crossed
    /// mid-search changes the name below it, and the new name is expressed as a
    /// position in a change set rather than written down.
    pub file: Option<FileSubject>,
}

/// One page of a narrowed history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum FilteredHistory {
    /// The directory has no repository.
    NotARepository,

    /// There is a repository, but this could not be read.
    #[serde(rename_all = "camelCase")]
    Unreadable {
        /// What went wrong, in words a person can act on.
        detail: String,
    },

    /// A commit or a change named by the filter is not here — a stale selection.
    Unknown,

    /// The search ran. An empty `commits` list with `stopped: No` means nothing
    /// in this history matches, which is a different answer from a budget that
    /// ran out, and is said differently.
    #[serde(rename_all = "camelCase")]
    Ready {
        /// Where `HEAD` is, for the surface's own header.
        head: Head,
        /// The matches, newest first.
        commits: Vec<Commit>,
        /// Where to continue, or `None` when the search reached the end.
        next: Option<FilterCursor>,
        /// How many commits were examined to produce this page.
        scanned: u32,
        /// Why it stopped.
        stopped: ScanStopped,
        /// Whether this is a shallow clone.
        shallow: bool,
    },
}

/// A branch or tag the interface may filter by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RefTip {
    /// What kind of reference it is.
    pub kind: RefKind,
    /// The short name, as Git would print it.
    pub name: String,
    /// The commit it points at — which is what a filter actually carries.
    pub tip: CommitId,
}

/// The references a filter may start from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum KnownRefs {
    /// The directory has no repository.
    NotARepository,

    /// There is a repository, but its references could not be listed.
    #[serde(rename_all = "camelCase")]
    Unreadable {
        /// What went wrong, in words a person can act on.
        detail: String,
    },

    /// The references were read.
    #[serde(rename_all = "camelCase")]
    Ready {
        /// Branches, remotes and tags, in reading order.
        refs: Vec<RefTip>,
        /// Whether there were more than [`MAX_REFS`] to look at.
        truncated: bool,
    },
}

/// One author, and how often they appear in what was examined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AuthorCount {
    /// The name as recorded in the commits.
    pub name: Term,
    /// How many of the examined commits they wrote.
    pub commits: u32,
}

/// The authors a filter may choose from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum KnownAuthors {
    /// The directory has no repository.
    NotARepository,

    /// There is a repository, but it could not be read.
    #[serde(rename_all = "camelCase")]
    Unreadable {
        /// What went wrong, in words a person can act on.
        detail: String,
    },

    /// The authors of what was examined.
    ///
    /// **Of what was examined**, not of the repository: this is a bounded scan
    /// like every other, so a name missing from the list may still be in the
    /// history further back. `scanned` and `stopped` say so.
    #[serde(rename_all = "camelCase")]
    Ready {
        /// Most frequent first, then alphabetical.
        authors: Vec<AuthorCount>,
        /// How many commits were examined.
        scanned: u32,
        /// Why the scan stopped.
        stopped: ScanStopped,
    },
}

/// Search history, narrowed by `filter`.
pub fn filtered(
    repo: &Repository,
    filter: &HistoryFilter,
    from: Option<&CommitId>,
) -> FilteredHistory {
    let head = match crate::libgit2::head(repo) {
        Ok(head) => head,
        Err(error) => {
            return FilteredHistory::Unreadable {
                detail: sentence(&error),
            }
        }
    };

    // The file filter's name is resolved once, through the bounded change list
    // 5c built and 5d already reads. It is the only filter that needs anything
    // looked up before the walk starts.
    let mut path = match &filter.file {
        Some(subject) => match trace::name_of(repo, subject) {
            Some(path) => Some(path),
            None => return FilteredHistory::Unknown,
        },
        None => None,
    };

    // Where the walk starts: a chosen branch's tip, the resume point, or HEAD.
    let start = match from.or(filter.branch.as_ref()) {
        Some(id) => match trace::resolve(repo, id) {
            Some(oid) => Some(oid),
            None => return FilteredHistory::Unknown,
        },
        None => repo.head().ok().and_then(|reference| reference.target()),
    };

    let Some(start) = start else {
        // An unborn branch: nothing to search, and nothing is wrong.
        return FilteredHistory::Ready {
            head,
            commits: Vec::new(),
            next: None,
            scanned: 0,
            stopped: ScanStopped::No,
            shallow: repo.is_shallow(),
        };
    };

    let mut walk = match repo.revwalk() {
        Ok(walk) => walk,
        Err(error) => {
            return FilteredHistory::Unreadable {
                detail: sentence(&error),
            }
        }
    };
    // Unsorted for ADR-0015's reason: a sorted revwalk reads the whole reachable
    // history before yielding anything, which is the opposite of a bounded scan.
    if let Err(error) = walk.set_sorting(Sort::NONE).and_then(|()| walk.push(start)) {
        return FilteredHistory::Unreadable {
            detail: sentence(&error),
        };
    }

    let author = filter.author.as_ref().map(Term::folded);
    let subject = filter.subject.as_ref().map(Term::folded);

    let mut commits: Vec<Commit> = Vec::with_capacity(PAGE);
    let mut scanned: usize = 0;
    let mut stopped = ScanStopped::No;
    let mut last: Option<(git2::Oid, bool)> = None;
    let mut ran_out = false;

    loop {
        if commits.len() == PAGE {
            break;
        }
        if scanned >= MAX_FILTER_SCAN {
            stopped = ScanStopped::Budget {
                scanned: count(scanned),
                limit: count(MAX_FILTER_SCAN),
            };
            break;
        }

        let Some(Ok(oid)) = walk.next() else {
            ran_out = true;
            break;
        };
        scanned += 1;

        let Ok(commit) = repo.find_commit(oid) else {
            ran_out = true;
            break;
        };

        // Ordering is not an optimisation here: measured, every predicate costs
        // what the walk costs, because loading the commit is the expense and the
        // walk has already paid it. They are written in the order a person would
        // read them.
        if let Some(wanted) = &author {
            let name = String::from_utf8_lossy(commit.author().name_bytes()).to_lowercase();
            if !name.contains(wanted.as_str()) {
                continue;
            }
        }

        if let Some(wanted) = &subject {
            let line =
                String::from_utf8_lossy(commit.summary_bytes().unwrap_or_default()).to_lowercase();
            if !line.contains(wanted.as_str()) {
                continue;
            }
        }

        let mut renamed = false;
        if let Some(following) = &path {
            let Some(touch) = trace::touched(&commit, following) else {
                continue;
            };

            // A path appearing is the one place a rename can hide, so it is the
            // only place a rename-detecting diff runs — the same rule, and the
            // same helper, as a file trace.
            if touch == ChangeKind::Added {
                match trace::came_from(repo, &commit, following) {
                    trace::Origin::Renamed(previous) | trace::Origin::Copied(previous) => {
                        path = Some(previous);
                        renamed = true;
                    }
                    trace::Origin::New => {}
                    trace::Origin::Lost => {
                        stopped = ScanStopped::RenameLost {
                            path: following.clone(),
                        };
                    }
                }
            }
        }

        commits.push(walk::summarise(&commit));
        last = Some((oid, renamed));

        if stopped != ScanStopped::No {
            break;
        }
    }

    let resume = if ran_out || matches!(stopped, ScanStopped::RenameLost { .. }) {
        None
    } else {
        walk.next().and_then(Result::ok)
    };

    let next = resume.and_then(|oid| {
        // Only a file filter needs its identity carried forward; the others match
        // against the commit itself and mean the same thing on every page.
        let file = match (&filter.file, &path, last) {
            (Some(_), Some(following), Some((matched, renamed))) => {
                Some(trace::anchor(repo, matched, following, renamed)?)
            }
            (Some(subject), _, None) => Some(subject.clone()),
            _ => None,
        };

        Some(FilterCursor {
            from: CommitId::try_from(oid.to_string()).ok()?,
            file,
        })
    });

    FilteredHistory::Ready {
        head,
        commits,
        next,
        scanned: count(scanned),
        stopped,
        shallow: repo.is_shallow(),
    }
}

/// The branches and tags a filter may start from.
///
/// Bounded by [`MAX_REFS`], like the graph's labels — a reference scan is bounded
/// by how many refs a repository has rather than by how long its history is, but
/// that is not the same as small.
pub fn known_refs(repo: &Repository) -> KnownRefs {
    let Ok(references) = repo.references() else {
        return KnownRefs::Ready {
            refs: Vec::new(),
            truncated: false,
        };
    };

    let mut refs: Vec<RefTip> = Vec::new();
    let mut truncated = false;

    for (seen, reference) in references.flatten().enumerate() {
        if seen >= MAX_REFS {
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
            continue;
        };

        // Peeled, so an annotated tag names the commit it points at rather than
        // the tag object — which is in no history and could start no walk.
        let Ok(commit) = reference.peel_to_commit() else {
            continue;
        };
        let Ok(name) = reference.shorthand() else {
            continue;
        };
        let Ok(tip) = CommitId::try_from(commit.id().to_string()) else {
            continue;
        };

        refs.push(RefTip {
            kind,
            name: name.to_owned(),
            tip,
        });
    }

    // Sorted so two reads of one repository offer the same list in the same
    // order: where you are, then branches, then remotes, then tags.
    refs.sort_by(|left, right| {
        GitRef {
            kind: left.kind,
            name: left.name.clone(),
        }
        .cmp(&GitRef {
            kind: right.kind,
            name: right.name.clone(),
        })
    });

    KnownRefs::Ready { refs, truncated }
}

/// The authors of the commits within one scan budget.
pub fn known_authors(repo: &Repository, from: Option<&CommitId>) -> KnownAuthors {
    let start = match from {
        Some(id) => trace::resolve(repo, id),
        None => repo.head().ok().and_then(|reference| reference.target()),
    };

    let Some(start) = start else {
        return KnownAuthors::Ready {
            authors: Vec::new(),
            scanned: 0,
            stopped: ScanStopped::No,
        };
    };

    let mut walk = match repo.revwalk() {
        Ok(walk) => walk,
        Err(error) => {
            return KnownAuthors::Unreadable {
                detail: sentence(&error),
            }
        }
    };
    if let Err(error) = walk.set_sorting(Sort::NONE).and_then(|()| walk.push(start)) {
        return KnownAuthors::Unreadable {
            detail: sentence(&error),
        };
    }

    let mut tally: Vec<(String, u32)> = Vec::new();
    let mut scanned: usize = 0;
    let mut stopped = ScanStopped::No;

    for oid in walk {
        if scanned >= MAX_FILTER_SCAN {
            stopped = ScanStopped::Budget {
                scanned: count(scanned),
                limit: count(MAX_FILTER_SCAN),
            };
            break;
        }
        let Ok(oid) = oid else { break };
        scanned += 1;

        let Ok(commit) = repo.find_commit(oid) else {
            break;
        };
        let name = String::from_utf8_lossy(commit.author().name_bytes()).into_owned();

        match tally.iter_mut().find(|(known, _)| *known == name) {
            Some((_, seen)) => *seen += 1,
            None => {
                // Past the cap, further names are not tallied. The list is for
                // choosing from, and a hundred names is already more than a menu.
                if tally.len() < MAX_AUTHORS {
                    tally.push((name, 1));
                }
            }
        }
    }

    // Most frequent first, then alphabetical — the order a person scans a menu.
    tally.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    let authors = tally
        .into_iter()
        .filter_map(|(name, commits)| {
            Some(AuthorCount {
                name: Term::try_from(name).ok()?,
                commits,
            })
        })
        .collect();

    KnownAuthors::Ready {
        authors,
        scanned: count(scanned),
        stopped,
    }
}

/// A count as the wire carries it.
fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::{Term, LONGEST_TERM};

    #[test]
    fn a_term_is_one_short_line() {
        assert_eq!(
            "Grace Hopper".parse::<Term>().expect("valid").as_str(),
            "Grace Hopper"
        );
        assert_eq!(
            "  padded  ".parse::<Term>().expect("valid").as_str(),
            "padded",
            "trimmed, so a stray space does not silently match nothing"
        );
    }

    #[test]
    fn nothing_that_is_not_a_term_parses() {
        for refused in ["", "   ", "two\nlines", "carriage\rreturn", "null\u{0}byte"] {
            assert!(
                refused.parse::<Term>().is_err(),
                "{refused:?} must not parse as a filter term"
            );
        }
        assert!("x".repeat(LONGEST_TERM + 1).parse::<Term>().is_err());
        assert!("x".repeat(LONGEST_TERM).parse::<Term>().is_ok());
    }

    #[test]
    fn matching_folds_case_the_way_a_person_would_expect() {
        // The documented semantics: case-insensitive substring, Unicode-aware.
        // A byte-wise ASCII fold measured 5 % faster and gets every non-English
        // name wrong, which five per cent does not buy.
        assert_eq!("WIDGET".parse::<Term>().expect("valid").folded(), "widget");
        assert_eq!(
            "Ångström".parse::<Term>().expect("valid").folded(),
            "ångström"
        );
        assert_eq!(
            "İstanbul"
                .parse::<Term>()
                .expect("valid")
                .folded()
                .chars()
                .next(),
            Some('i')
        );
    }
}
