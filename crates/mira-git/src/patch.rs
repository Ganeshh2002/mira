//! Reading diffs through libgit2, inside the limits `diff.rs` declares.
//!
//! Bounded at four points, in this order, so that nothing large is ever read even
//! to be rejected:
//!
//! 1. libgit2 is given [`MAX_FILE_BYTES`] as its own `max_size`, so a blob past it
//!    is treated as binary and never materialised.
//! 2. Rename and copy detection is capped at [`MAX_FILES`] candidates.
//! 3. The delta list is cut at [`MAX_FILES`], and the total is reported.
//! 4. Line counting for the list spends at most [`MAX_STATS_BYTES`] of blob;
//!    files past that are listed without counts rather than the list stopping.
//!
//! Then, per file: [`MAX_LINES`], [`MAX_BYTES`] and [`MAX_LINE_BYTES`].
//!
//! Reads only. `git2::Diff` and `git2::Patch` cannot write, and no other API is
//! touched (ADR-0009, ADR-0016).

use git2::{Delta, Diff, DiffFindOptions, DiffOptions, Oid, Repository, Tree};

use crate::diff::{
    ChangeKind, ChangedFiles, Comparison, DiffLine, DiffScope, FileChange, FileDiff,
    FilesTruncated, Hunk, LineKind, PatchTruncated, MAX_BYTES, MAX_FILES, MAX_FILE_BYTES,
    MAX_LINES, MAX_LINE_BYTES,
};
use crate::history::CommitId;
use crate::libgit2::sentence;

/// How much blob Mira will diff purely to count a change list's lines.
///
/// The per-file ceiling bounds one file; this bounds a *list* of them, so a
/// commit touching two hundred large files cannot turn a listing into four
/// hundred megabytes of work. Files past it are listed with no counts, which is a
/// state [`FileChange::additions`] already has a word for.
pub const MAX_STATS_BYTES: u64 = 8 * 1024 * 1024;

/// A prepared comparison, or the reason there is not one.
enum Prepared<'repo> {
    Ready(Diff<'repo>, Comparison),
    Unknown,
    Unreadable(String),
}

/// The changed paths of a commit, or of the working tree.
pub fn changed_files(repo: &Repository, scope: &DiffScope) -> ChangedFiles {
    let (diff, against) = match prepare(repo, scope) {
        Prepared::Ready(diff, against) => (diff, against),
        Prepared::Unknown => return ChangedFiles::Unknown,
        Prepared::Unreadable(detail) => return ChangedFiles::Unreadable { detail },
    };

    let total = diff.deltas().len();
    let mut files = Vec::with_capacity(total.min(MAX_FILES));
    let mut spent: u64 = 0;

    for (at, delta) in diff.deltas().enumerate().take(MAX_FILES) {
        let mut change = describe(at, &delta);

        // Both "is it binary" and "how many lines" cost a generated patch —
        // libgit2 knows neither until it has loaded the contents — so both are
        // budgeted together. A file that is oversized or past the budget is
        // listed without counts, which is what `None` means; opening it is the
        // authoritative answer and reads the size and the binary flag properly.
        let bytes = largest(repo, &delta);
        if bytes <= MAX_FILE_BYTES && spent < MAX_STATS_BYTES {
            spent = spent.saturating_add(bytes);
            if let Ok(Some(patch)) = git2::Patch::from_diff(&diff, at) {
                change.binary = patch.delta().flags().is_binary();
                if !change.binary {
                    if let Ok((_, additions, deletions)) = patch.line_stats() {
                        change.additions = u32::try_from(additions).ok();
                        change.deletions = u32::try_from(deletions).ok();
                    }
                }
            }
        }

        files.push(change);
    }

    let truncated = if total > files.len() {
        FilesTruncated::Yes {
            shown: count(files.len()),
            total: count(total),
            limit: count(MAX_FILES),
        }
    } else {
        FilesTruncated::No
    };

    ChangedFiles::Ready {
        files,
        against,
        truncated,
    }
}

/// One file's patch, by its position in the change list.
///
/// `at` is an ordinal in the list [`changed_files`] returned, not a path. The same
/// comparison is rebuilt with the same options, so the same ordinal names the same
/// file — and an ordinal past the end is a stale selection, which is
/// [`FileDiff::Unknown`] rather than a failure.
pub fn file_diff(repo: &Repository, scope: &DiffScope, at: u32) -> FileDiff {
    let (diff, _) = match prepare(repo, scope) {
        Prepared::Ready(diff, against) => (diff, against),
        Prepared::Unknown => return FileDiff::Unknown,
        Prepared::Unreadable(detail) => return FileDiff::Unreadable { detail },
    };

    let index = at as usize;
    // Past the list Mira offers is past what Mira will read, whatever the
    // repository holds. The interface can only have got this ordinal from a list
    // it was given, so this is a stale selection rather than an attempt.
    if index >= MAX_FILES {
        return FileDiff::Unknown;
    }

    let Some(delta) = diff.deltas().nth(index) else {
        return FileDiff::Unknown;
    };
    let mut change = describe(index, &delta);

    // The size gate comes first, and it reads object *headers* rather than
    // objects — so a file past the ceiling is refused without ever having been
    // in memory, which is the difference between a limit and a cleanup.
    let old_bytes = size_of(repo, &delta.old_file());
    let new_bytes = size_of(repo, &delta.new_file());
    let bytes = old_bytes.max(new_bytes);
    if bytes > MAX_FILE_BYTES {
        return FileDiff::TooLarge {
            change,
            bytes,
            limit: MAX_FILE_BYTES,
        };
    }

    let generated = match git2::Patch::from_diff(&diff, index) {
        Ok(generated) => generated,
        Err(error) => {
            return FileDiff::Unreadable {
                detail: sentence(&error),
            }
        }
    };

    let Some(patch) = generated else {
        // No patch at all: a pure rename, or a mode change. The change is real
        // and there is nothing to show for it, which is a state and not a gap.
        return FileDiff::Ready {
            change,
            hunks: Vec::new(),
            truncated: PatchTruncated::No,
        };
    };

    // Binariness is libgit2's judgement and it only makes it once it has looked.
    // Identified, never decoded: the sizes go out, the contents do not.
    if patch.delta().flags().is_binary() {
        change.binary = true;
        return FileDiff::Binary {
            change,
            old_bytes: delta.old_file().exists().then_some(old_bytes),
            new_bytes: delta.new_file().exists().then_some(new_bytes),
        };
    }

    match render(&patch, change) {
        Ok(rendered) => rendered,
        Err(error) => FileDiff::Unreadable {
            detail: sentence(&error),
        },
    }
}

/// Walk one file's patch, stopping at whichever limit bites first.
fn render(patch: &git2::Patch<'_>, change: FileChange) -> Result<FileDiff, git2::Error> {
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut shown_lines: usize = 0;
    let mut shown_bytes: usize = 0;
    let mut truncated = PatchTruncated::No;

    for index in 0..patch.num_hunks() {
        let (hunk, _) = patch.hunk(index)?;
        let mut lines = Vec::new();

        for line in 0..patch.num_lines_in_hunk(index)? {
            if shown_lines >= MAX_LINES {
                truncated = PatchTruncated::Lines {
                    shown: count(shown_lines),
                    limit: count(MAX_LINES),
                };
                break;
            }
            if shown_bytes >= MAX_BYTES {
                truncated = PatchTruncated::Bytes {
                    shown: shown_bytes as u64,
                    limit: MAX_BYTES as u64,
                };
                break;
            }

            let found = patch.line_in_hunk(index, line)?;
            let (text, cut) = readable(found.content());
            shown_lines += 1;
            shown_bytes += text.len();

            lines.push(DiffLine {
                kind: origin(found.origin()),
                old_line: found.old_lineno(),
                new_line: found.new_lineno(),
                text,
                cut,
            });
        }

        if !lines.is_empty() {
            hunks.push(Hunk {
                header: String::from_utf8_lossy(hunk.header()).trim_end().to_owned(),
                lines,
            });
        }

        if truncated != PatchTruncated::No {
            break;
        }
    }

    Ok(FileDiff::Ready {
        change,
        hunks,
        truncated,
    })
}

/// Build the comparison this scope asks for.
fn prepare<'repo>(repo: &'repo Repository, scope: &DiffScope) -> Prepared<'repo> {
    let mut options = options();

    let (built, against) = match scope {
        DiffScope::Commit { commit } => {
            let Some(oid) = resolve(repo, commit) else {
                return Prepared::Unknown;
            };
            let found = match repo.find_commit(oid) {
                Ok(found) => found,
                Err(_) => return Prepared::Unknown,
            };

            let new = match found.tree() {
                Ok(tree) => tree,
                Err(error) => return Prepared::Unreadable(sentence(&error)),
            };

            // A merge is compared against its **first** parent, and the answer
            // says so: against the second parent the same commit changed
            // different things, and a diff that did not say which side it picked
            // would be quietly choosing one (ADR-0016).
            let parents = found.parent_count();
            // A root commit has no parent, and a commit whose parent's tree
            // cannot be read is compared against nothing rather than failing —
            // showing "everything is new" beats showing an error for a commit
            // that is plainly there.
            let old: Option<Tree<'repo>> = found.parent(0).and_then(|parent| parent.tree()).ok();
            let against = match (parents, old.is_some()) {
                (0, _) | (_, false) => Comparison::EmptyTree,
                (1, _) => Comparison::Parent,
                (more, _) => Comparison::FirstParent {
                    parents: count(more),
                },
            };

            (
                repo.diff_tree_to_tree(old.as_ref(), Some(&new), Some(&mut options)),
                against,
            )
        }

        DiffScope::WorkingTree => {
            // Untracked files are included, as one entry per untracked directory
            // rather than one per file inside it — the same rule the overview's
            // changed count uses, so the two never disagree.
            //
            // `show_untracked_content` is what makes a new file readable rather
            // than a row reading "+0 −0": without it libgit2 reports that the
            // path is there and declines to say what is in it, which is the less
            // useful half of the answer. It costs nothing extra to bound — an
            // untracked file goes through the same size gate as any other.
            options
                .include_untracked(true)
                .recurse_untracked_dirs(false)
                .show_untracked_content(true);

            let head = repo
                .head()
                .ok()
                .and_then(|head| head.target())
                .and_then(|oid| repo.find_commit(oid).ok())
                .and_then(|commit| commit.tree().ok());

            (
                repo.diff_tree_to_workdir_with_index(head.as_ref(), Some(&mut options)),
                Comparison::Head,
            )
        }
    };

    let mut diff = match built {
        Ok(diff) => diff,
        Err(error) => return Prepared::Unreadable(sentence(&error)),
    };

    // Renames and copies, with the candidate set capped: similarity detection is
    // quadratic in the files considered, and an uncapped one on a commit that
    // moved a vendored directory is the slowest thing a diff can do.
    let mut finding = DiffFindOptions::new();
    finding
        .renames(true)
        .copies(true)
        .rename_limit(MAX_FILES)
        .break_rewrites(false);
    if let Err(error) = diff.find_similar(Some(&mut finding)) {
        return Prepared::Unreadable(sentence(&error));
    }

    Prepared::Ready(diff, against)
}

/// The options every comparison is built with.
///
/// `max_size` is the load-bearing one: it is libgit2's own ceiling, so a blob past
/// [`MAX_FILE_BYTES`] is marked binary and its contents are never read — the bound
/// applies before anything large exists in memory, not after.
fn options() -> DiffOptions {
    let mut options = DiffOptions::new();
    options
        .context_lines(3)
        .include_typechange(true)
        .ignore_submodules(true)
        .max_size(i64::try_from(MAX_FILE_BYTES).unwrap_or(i64::MAX));
    options
}

/// What one delta is, as the interface reads it.
fn describe(at: usize, delta: &git2::DiffDelta<'_>) -> FileChange {
    let kind = match delta.status() {
        Delta::Added | Delta::Untracked => ChangeKind::Added,
        Delta::Deleted => ChangeKind::Deleted,
        Delta::Renamed => ChangeKind::Renamed,
        Delta::Copied => ChangeKind::Copied,
        Delta::Typechange => ChangeKind::TypeChanged,
        // Modified, and everything else Git can report — conflicted, ignored,
        // unreadable — reads as "this path is not what it was", which is what a
        // person wants to know and is true of all of them.
        _ => ChangeKind::Modified,
    };

    let new = path_of(&delta.new_file());
    let old = path_of(&delta.old_file());

    // A deletion has no new path; a rename has both and shows where it came from.
    let path = match (&new, &old) {
        (Some(new), _) => new.clone(),
        (None, Some(old)) => old.clone(),
        (None, None) => String::new(),
    };
    let from_path = match kind {
        ChangeKind::Renamed | ChangeKind::Copied => old.filter(|from| *from != path),
        _ => None,
    };

    FileChange {
        at: count(at),
        kind,
        path,
        from_path,
        binary: delta.flags().is_binary(),
        additions: None,
        deletions: None,
    }
}

/// A delta's path, with `/` separators on every platform.
fn path_of(file: &git2::DiffFile<'_>) -> Option<String> {
    let path = file.path()?;
    let written: Vec<String> = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect();

    (!written.is_empty()).then(|| written.join("/"))
}

/// The larger of a delta's two sides, which is what a size limit has to test.
fn largest(repo: &Repository, delta: &git2::DiffDelta<'_>) -> u64 {
    size_of(repo, &delta.old_file()).max(size_of(repo, &delta.new_file()))
}

/// One side's size, **without reading its contents**.
///
/// This is what makes the size limit a limit rather than a cleanup. libgit2 fills
/// a delta's `size` only once it has loaded the blob, so asking it directly would
/// mean reading the very file the ceiling exists to refuse. A working-tree side
/// has a size from `stat`; a tree side has one in its object header, and reading
/// a header is not reading an object.
fn size_of(repo: &Repository, file: &git2::DiffFile<'_>) -> u64 {
    if !file.exists() {
        return 0;
    }

    let stated = file.size();
    if stated > 0 {
        return stated;
    }

    let id = file.id();
    if id.is_zero() {
        return 0;
    }

    repo.odb()
        .ok()
        .and_then(|odb| odb.read_header(id).ok())
        .map_or(0, |(size, _)| size as u64)
}

/// One patch line as text, cut at [`MAX_LINE_BYTES`] if it runs long.
///
/// Lossy, because a file is bytes and a Latin-1 one must render rather than
/// panic. Cut on a character boundary, because cutting inside one would produce
/// the replacement character at the end of every long line.
fn readable(content: &[u8]) -> (String, bool) {
    let text = String::from_utf8_lossy(content);
    let trimmed = text.trim_end_matches(['\n', '\r']);

    if trimmed.len() <= MAX_LINE_BYTES {
        return (trimmed.to_owned(), false);
    }

    let boundary = trimmed
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= MAX_LINE_BYTES)
        .last()
        .unwrap_or(0);

    (trimmed[..boundary].to_owned(), true)
}

/// What libgit2's origin character means.
const fn origin(marker: char) -> LineKind {
    match marker {
        '+' => LineKind::Addition,
        '-' => LineKind::Deletion,
        ' ' => LineKind::Context,
        // `=`, `>`, `<` are Git's end-of-file notes, and `B` is a binary marker
        // that `line_in_hunk` does not produce. All of them are things Git is
        // saying about the patch rather than lines of the file.
        _ => LineKind::Note,
    }
}

/// The object this id names, if this repository has it.
///
/// The same deliberate narrowness as the history walk: `revparse_single` on a
/// validated hexadecimal id can only produce an object id, and peeling is what
/// turns an annotated tag into the commit it names.
fn resolve(repo: &Repository, id: &CommitId) -> Option<Oid> {
    let object = repo.revparse_single(id.as_str()).ok()?;
    object.peel_to_commit().ok().map(|commit| commit.id())
}

/// A count as the wire carries it.
///
/// Saturating rather than wrapping: every count here is bounded by a constant in
/// this crate, so this is a total conversion for an impossible case rather than a
/// silent truncation of a real one.
fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
