//! `git.*` — what happened in a project's repository.
//!
//! Thin by rule (`architecture.md` §5): resolve a project row to a repository,
//! call the provider, map the result. The reading itself is `mira-git`'s, and
//! everything here is about the boundary.
//!
//! Three things are true of every command in this file.
//!
//! **The caller names a project, never a directory.** The repository is resolved
//! in Rust from a row that could only have been created by a native picker
//! (`security-and-privacy.md` §5 rule 8).
//!
//! **The caller cannot say how much to read.** There is no `limit`, no `count`,
//! no `all`. A page is [`mira_git::PAGE`] commits, decided in `mira-git`, and the
//! only lever the interface has is a cursor saying *continue from here*. That is
//! what makes "Mira never walks a whole repository" a property of the signature.
//!
//! **The caller cannot name a Git argument.** A [`CommitId`] is four to forty
//! hexadecimal characters, checked when it deserialises, so `HEAD`, `--exec=…`,
//! `../../etc/passwd` and a refspec all fail on the wire. Mira does not run `git`
//! (ADR-0009), and this is the second wall in case it ever did.

use std::path::PathBuf;
use std::sync::Arc;

use mira_core::{MiraError, Project, ProjectId, Result};
use mira_git::{
    ChangedFiles, CommitGraph, CommitId, CommitLookup, CommitPage, DiffScope, FileDiff,
    FileHistory, FileSubject, FilteredHistory, GitProvider, HistoryFilter, KnownAuthors, KnownRefs,
    Libgit2,
};
use mira_platform::{Clipboard, ClipboardHost};
use mira_projects::ProjectService;
use serde::{Deserialize, Serialize};
use tauri::State;
use ts_rs::TS;

use crate::state::AppState;

/// Which spelling of a commit id to copy.
///
/// Two words, because there are two: the seven characters a person quotes in a
/// message, and the forty a tool wants. Nothing else can be asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ShaForm {
    /// The abbreviation Mira shows.
    Short,
    /// The whole object id.
    Full,
}

/// `git.history` — one page of a project's repository history.
///
/// `cursor` is the id the previous page returned as its `next`, and `None` starts
/// at `HEAD`. Nothing here is polled: history is read when a person opens the
/// surface, presses Refresh, or asks for more, and the scheduler has no observer
/// for it (`information-architecture.md` §3, the on-view tier).
#[tauri::command]
pub async fn git_history(
    project_id: ProjectId,
    cursor: Option<CommitId>,
    state: State<'_, Arc<AppState>>,
) -> Result<CommitPage> {
    let root = repository_of(&state.projects().get(project_id)?);

    // On the blocking pool: a repository read is syscall work and does not belong
    // on the webview's thread (`architecture.md` §5 rule 3).
    read(move || Libgit2.history(&root, cursor.as_ref())).await
}

/// `git.graph` — the same page as `git.history`, with the shape of it.
///
/// Parents, lanes and reference labels, computed over that page and nothing else.
/// The same cursor, the same page size, the same bound: there is no second
/// traversal here and no way to ask for one, because there is still no parameter
/// that says how much to read.
///
/// It is a **picture**. Nothing reachable from this command checks anything out,
/// merges, rebases, resets, cherry-picks, creates a commit, stages a path, or
/// touches a remote ([ADR-0015](../../../docs/adr/0015-graph-lanes.md)).
#[tauri::command]
pub async fn git_graph(
    project_id: ProjectId,
    cursor: Option<CommitId>,
    state: State<'_, Arc<AppState>>,
) -> Result<CommitGraph> {
    let root = repository_of(&state.projects().get(project_id)?);

    read(move || Libgit2.graph(&root, cursor.as_ref())).await
}

/// `git.commit` — one commit, in the detail its own view shows.
///
/// Read-only, and deliberately less than a diff: 5a says what a commit *is*, and
/// what it *changed* is 5b's read-only diff view.
#[tauri::command]
pub async fn git_commit(
    project_id: ProjectId,
    commit: CommitId,
    state: State<'_, Arc<AppState>>,
) -> Result<CommitLookup> {
    let root = repository_of(&state.projects().get(project_id)?);

    read(move || Libgit2.commit(&root, &commit)).await
}

/// `git.copy_commit` — put a commit id on the clipboard.
///
/// The commit is **resolved from the repository first**, and what is copied is
/// what came back. That is the whole reason this is a command rather than a line
/// of JavaScript: the interface asks to copy *a commit*, not *a string*, so there
/// is no path by which a page could use Mira to place text of its own choosing on
/// the clipboard of the person running it. A commit that is not in the repository
/// copies nothing and says so.
///
/// Returns what was copied, so the interface can confirm it without keeping its
/// own idea of what the clipboard holds.
#[tauri::command]
pub async fn git_copy_commit(
    project_id: ProjectId,
    commit: CommitId,
    form: ShaForm,
    state: State<'_, Arc<AppState>>,
) -> Result<String> {
    let root = repository_of(&state.projects().get(project_id)?);
    let looked_up = read(move || Libgit2.commit(&root, &commit)).await?;

    let found = match looked_up {
        CommitLookup::Ready { commit } => commit.commit,
        CommitLookup::Unknown => {
            return Err(MiraError::NotFound {
                what: "That commit".to_owned(),
            })
        }
        CommitLookup::NotARepository => {
            return Err(MiraError::NotFound {
                what: "A repository for this project".to_owned(),
            })
        }
        CommitLookup::Unreadable { detail } => {
            return Err(MiraError::External {
                source: "Git".to_owned(),
                detail,
            })
        }
    };

    let value = match form {
        ShaForm::Short => found.short_sha,
        ShaForm::Full => found.sha,
    };

    Clipboard::new(state.platform.clone()).copy(&value)?;
    Ok(value)
}

/// `git.changes` — what a commit changed, or what the working tree has.
///
/// The scope is a two-variant enum: a **commit** (whose id is validated as it
/// deserialises, like every other Git value the interface may name) or the
/// **working tree**. They are separate questions and stay separate answers — a
/// working tree is not a commit, and one list of both would make it impossible to
/// tell what is recorded from what is merely on disk.
///
/// Bounded by `mira_git::MAX_FILES`, and the answer says when the bound bit.
/// There is still no parameter that says how much to read.
#[tauri::command]
pub async fn git_changes(
    project_id: ProjectId,
    scope: DiffScope,
    state: State<'_, Arc<AppState>>,
) -> Result<ChangedFiles> {
    let root = repository_of(&state.projects().get(project_id)?);

    read(move || Libgit2.changed_files(&root, &scope)).await
}

/// `git.file_diff` — one file's patch.
///
/// **`at` is an ordinal, not a path.** It is a position in the list `git.changes`
/// returned, so the interface can only ask for a file Mira already decided to
/// offer — there is no argument here through which a page could name a location on
/// disk, and a guard test fails the build if one appears
/// (`security-and-privacy.md` §5 rule 8, [ADR-0016](../../../docs/adr/0016-bounded-diffs.md)).
///
/// An ordinal past the list is a stale selection, which is a state rather than an
/// error. Every limit that bites comes back as a value saying which one it was:
/// nothing is truncated silently, and a binary file is identified rather than
/// decoded.
#[tauri::command]
pub async fn git_file_diff(
    project_id: ProjectId,
    scope: DiffScope,
    at: u32,
    state: State<'_, Arc<AppState>>,
) -> Result<FileDiff> {
    let root = repository_of(&state.projects().get(project_id)?);

    read(move || Libgit2.file_diff(&root, &scope, at)).await
}

/// `git.file_history` — the commits that touched one file.
///
/// **The file is named by a `subject`, never by a path.** A subject is a change
/// set Mira produced, a position in it, and which side of that change to take the
/// name from. The interface receives one — in a change list, or in the cursor of
/// a previous page — and hands it back. It has no way to build a different one,
/// which is what lets a walk that follows a file across renames keep
/// `security-and-privacy.md` §5 rule 8 intact.
///
/// **Bounded by commits examined**, because looking is the cost: file history is
/// inherently O(repository history), and `mira_git::MAX_SCAN` is where one
/// request stops. A page that ran out of budget says how far it got and offers a
/// cursor, so partial is never lost and never mistaken for empty
/// ([ADR-0017](../../../docs/adr/0017-file-history.md)).
#[tauri::command]
pub async fn git_file_history(
    project_id: ProjectId,
    subject: FileSubject,
    cursor: Option<CommitId>,
    state: State<'_, Arc<AppState>>,
) -> Result<FileHistory> {
    let root = repository_of(&state.projects().get(project_id)?);

    read(move || Libgit2.file_history(&root, &subject, cursor.as_ref())).await
}

/// `git.search` — the commits matching a filter.
///
/// Composable: branch, author, subject text and file narrow together, and every
/// one that is set has to match.
///
/// **Nothing in `wanted` becomes a Git argument.** A branch is a `CommitId` — a
/// tip Mira listed — so there is no ref *name* on the wire at all. A file is a
/// `FileSubject`, 5d's change-set position, so there is no path. Author and
/// subject are validated terms that are only ever *compared*, in Rust, against
/// fields of a commit already in memory; neither reaches libgit2.
///
/// Bounded by `mira_git::MAX_FILTER_SCAN` commits examined. A search that ran out
/// of budget says how far it looked, because *"nothing in the first two thousand
/// commits"* and *"nothing in this history"* are different facts and the second
/// one would be a lie ([ADR-0018](../../../docs/adr/0018-history-filters.md)).
#[tauri::command]
pub async fn git_search(
    project_id: ProjectId,
    wanted: HistoryFilter,
    cursor: Option<CommitId>,
    state: State<'_, Arc<AppState>>,
) -> Result<FilteredHistory> {
    let root = repository_of(&state.projects().get(project_id)?);

    read(move || Libgit2.filtered_history(&root, &wanted, cursor.as_ref())).await
}

/// `git.refs` — the branches and tags a search may start from.
///
/// Each carries the commit it points at, and **that** is what a filter sends
/// back. The name is for reading; the tip is for asking.
#[tauri::command]
pub async fn git_refs(project_id: ProjectId, state: State<'_, Arc<AppState>>) -> Result<KnownRefs> {
    let root = repository_of(&state.projects().get(project_id)?);

    read(move || Libgit2.known_refs(&root)).await
}

/// `git.authors` — the authors of the commits within one scan budget.
///
/// Of what was *examined*, not of the repository: a name missing from the list
/// may still be further back, and the answer says how far it looked. It is a menu
/// to choose from, which is why it is bounded like everything else.
#[tauri::command]
pub async fn git_authors(
    project_id: ProjectId,
    state: State<'_, Arc<AppState>>,
) -> Result<KnownAuthors> {
    let root = repository_of(&state.projects().get(project_id)?);

    read(move || Libgit2.known_authors(&root, None)).await
}

/// The repository a project's history belongs to.
///
/// **History belongs to the repository, not to the directory that was added.** A
/// package inside a monorepo has no history of its own; it shares the one above
/// it. `git_root` is the worktree root whenever it differs from the project's own
/// directory, so every package in a monorepo resolves to the same repository and
/// there is no second, per-package walk to keep in step
/// ([ADR-0010](../../../docs/adr/0010-monorepo-detection.md)).
fn repository_of(project: &Project) -> PathBuf {
    PathBuf::from(
        project
            .git_root
            .as_ref()
            .unwrap_or(&project.root_path)
            .as_str(),
    )
}

/// Run a repository read off the webview's thread.
async fn read<T, F>(reading: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(reading)
        .await
        .map_err(|joined| MiraError::external("Mira", joined))
}

#[cfg(test)]
mod tests {
    use super::{repository_of, ShaForm};
    use mira_core::{Project, ProjectId};

    fn project(root: &str, git_root: Option<&str>) -> Project {
        Project {
            id: ProjectId::new(1),
            name: "web".to_owned(),
            root_path: root.to_owned(),
            is_git: true,
            git_root: git_root.map(ToOwned::to_owned),
            markers: Vec::new(),
            last_opened_at: None,
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn a_standalone_project_reads_its_own_directory() {
        assert_eq!(
            repository_of(&project("/home/dev/aviora", None)),
            std::path::Path::new("/home/dev/aviora")
        );
    }

    #[test]
    fn a_package_reads_the_repository_above_it() {
        // Two packages in one monorepo resolve to one repository, which is what
        // stops history being duplicated per package.
        let web = repository_of(&project(
            "/home/dev/aviora/apps/web",
            Some("/home/dev/aviora"),
        ));
        let api = repository_of(&project(
            "/home/dev/aviora/apps/api",
            Some("/home/dev/aviora"),
        ));

        assert_eq!(web, api);
        assert_eq!(web, std::path::Path::new("/home/dev/aviora"));
    }

    #[test]
    fn a_sha_can_be_asked_for_in_exactly_two_forms() {
        for accepted in ["short", "full"] {
            assert!(serde_json::from_str::<ShaForm>(&format!("\"{accepted}\"")).is_ok());
        }
        for refused in ["\"Short\"", "\"whole\"", "0", "null", "\"short; id\""] {
            assert!(
                serde_json::from_str::<ShaForm>(refused).is_err(),
                "{refused} must not deserialise into a form"
            );
        }
    }
}
