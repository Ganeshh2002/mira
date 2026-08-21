//! Adding, listing, opening and removing projects.
//!
//! The service is where a directory the user picked becomes a project: validated,
//! canonicalised, probed for Git and type markers, and stored. Git arrives through
//! its trait, so these tests describe project behaviour without needing a
//! repository on disk (`architecture.md` §4, "boundary tests").

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use mira_core::MiraError;
use mira_db::{Db, ProjectRepo};
use mira_fs::PathMatching;
use mira_git::{
    ChangedFiles, CommitGraph, CommitId, CommitLookup, CommitPage, DiffScope, FileDiff,
    FileHistory, FileSubject, FilteredHistory, GitOverview, GitProvider, Head, HistoryFilter,
    KnownAuthors, KnownRefs,
};
use mira_projects::{ProjectService, Projects};
use tempfile::TempDir;

/// A Git provider with answers supplied by the test.
#[derive(Default)]
struct FakeGit {
    worktree: Option<PathBuf>,
    asked: RefCell<Vec<PathBuf>>,
}

impl FakeGit {
    fn none() -> Self {
        Self::default()
    }

    fn rooted_at(path: &Path) -> Self {
        Self {
            worktree: Some(path.to_path_buf()),
            asked: RefCell::new(Vec::new()),
        }
    }
}

impl GitProvider for FakeGit {
    fn discover(&self, start: &Path) -> Option<PathBuf> {
        self.asked.borrow_mut().push(start.to_path_buf());
        self.worktree.clone()
    }

    fn overview(&self, _root: &Path) -> GitOverview {
        GitOverview::Ready {
            head: Head::Branch {
                name: "main".to_owned(),
            },
            clean: true,
            changed: 0,
            last_commit: None,
            upstream: None,
        }
    }

    // Projects never reads history — it is a question about a repository, asked
    // by `commands::git` and answered by the provider directly. These exist so
    // the fake is a whole provider, and asserting they are never called is what
    // `the_project_service_asks_git_only_where_a_project_is` does next door.
    fn history(&self, _root: &Path, _from: Option<&CommitId>) -> CommitPage {
        panic!("the project service must not read history");
    }

    fn commit(&self, _root: &Path, _id: &CommitId) -> CommitLookup {
        panic!("the project service must not read a commit");
    }

    fn graph(&self, _root: &Path, _from: Option<&CommitId>) -> CommitGraph {
        panic!("the project service must not draw a graph");
    }

    fn changed_files(&self, _root: &Path, _scope: &DiffScope) -> ChangedFiles {
        panic!("the project service must not read a diff");
    }

    fn file_diff(&self, _root: &Path, _scope: &DiffScope, _at: u32) -> FileDiff {
        panic!("the project service must not read a diff");
    }

    fn file_history(
        &self,
        _root: &Path,
        _subject: &FileSubject,
        _from: Option<&CommitId>,
    ) -> FileHistory {
        panic!("the project service must not trace a file");
    }

    fn filtered_history(
        &self,
        _root: &Path,
        _filter: &HistoryFilter,
        _from: Option<&CommitId>,
    ) -> FilteredHistory {
        panic!("the project service must not search a history");
    }

    fn known_refs(&self, _root: &Path) -> KnownRefs {
        panic!("the project service must not read references");
    }

    fn known_authors(&self, _root: &Path, _from: Option<&CommitId>) -> KnownAuthors {
        panic!("the project service must not read authors");
    }
}

fn service<G: GitProvider>(db: &Db, git: G) -> Projects<&Db, G> {
    Projects::new(db, git, PathMatching::CaseSensitive)
}

#[test]
fn adding_a_directory_creates_a_project_named_after_it() {
    let dir = TempDir::new().expect("tempdir");
    let root = dir.path().join("aviora");
    fs::create_dir(&root).expect("mkdir");
    let db = Db::open_in_memory().expect("db");

    let project = service(&db, FakeGit::none())
        .add(&root, 1_700_000_000)
        .expect("add");

    assert_eq!(project.name, "aviora");
    assert_eq!(
        Path::new(&project.root_path),
        root.canonicalize().expect("canonical"),
        "the stored path is canonical, so the project has one identity"
    );
    assert_eq!(db.count().expect("count"), 1);
}

#[test]
fn adding_a_directory_that_is_not_there_says_so_without_storing_anything() {
    let dir = TempDir::new().expect("tempdir");
    let db = Db::open_in_memory().expect("db");

    let failed = service(&db, FakeGit::none()).add(&dir.path().join("gone"), 1);

    assert!(matches!(failed, Err(MiraError::NotFound { .. })));
    assert_eq!(db.count().expect("count"), 0, "a failed add stores nothing");
}

#[test]
fn adding_a_file_is_refused_because_a_project_is_a_directory() {
    let dir = TempDir::new().expect("tempdir");
    let file = dir.path().join("notes.md");
    fs::write(&file, "hello").expect("write");
    let db = Db::open_in_memory().expect("db");

    assert!(matches!(
        service(&db, FakeGit::none()).add(&file, 1),
        Err(MiraError::Invalid { .. })
    ));
}

#[test]
fn adding_the_same_directory_twice_names_the_project_already_using_it() {
    let dir = TempDir::new().expect("tempdir");
    let db = Db::open_in_memory().expect("db");
    service(&db, FakeGit::none())
        .add(dir.path(), 1)
        .expect("add");

    match service(&db, FakeGit::none()).add(dir.path(), 2) {
        Err(MiraError::Invalid { field, detail }) => {
            assert_eq!(field, "path");
            assert!(
                detail.contains(
                    &dir.path()
                        .file_name()
                        .expect("name")
                        .to_string_lossy()
                        .to_string()
                ),
                "the message names the project already there: {detail}"
            );
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
    assert_eq!(db.count().expect("count"), 1);
}

#[test]
fn a_directory_reached_by_a_different_spelling_is_still_the_same_project() {
    let dir = TempDir::new().expect("tempdir");
    let root = dir.path().join("aviora");
    fs::create_dir(&root).expect("mkdir");
    let db = Db::open_in_memory().expect("db");
    service(&db, FakeGit::none()).add(&root, 1).expect("add");

    let again = service(&db, FakeGit::none()).add(&root.join("..").join("aviora"), 2);

    assert!(
        matches!(again, Err(MiraError::Invalid { .. })),
        "canonicalisation happens before the uniqueness check"
    );
}

// ── Detection ────────────────────────────────────────────────────────────────

#[test]
fn a_directory_without_git_is_stored_as_not_a_repository() {
    let dir = TempDir::new().expect("tempdir");
    let db = Db::open_in_memory().expect("db");

    let project = service(&db, FakeGit::none())
        .add(dir.path(), 1)
        .expect("add");

    assert!(!project.is_git);
    assert_eq!(project.git_root, None);
}

#[test]
fn a_worktree_root_above_the_project_is_recorded() {
    let dir = TempDir::new().expect("tempdir");
    let root = dir.path().canonicalize().expect("canonical");
    let inner = root.join("web");
    fs::create_dir(&inner).expect("mkdir");
    let db = Db::open_in_memory().expect("db");

    let project = service(&db, FakeGit::rooted_at(&root))
        .add(&inner, 1)
        .expect("add");

    assert!(project.is_git);
    assert_eq!(
        project.git_root.as_deref().map(Path::new),
        Some(root.as_path()),
        "FR-1.3 walks up to the worktree root and remembers where it landed"
    );
}

#[test]
fn a_stored_worktree_root_has_no_trailing_separator() {
    // libgit2 reports a worktree as `/home/dev/aviora/`. Paths compare by
    // components so the trailing separator changes no behaviour, but it is shown
    // to a person under "Repository", and a path with a dangling slash reads like
    // a bug in Mira.
    let dir = TempDir::new().expect("tempdir");
    let root = dir.path().canonicalize().expect("canonical");
    let inner = root.join("web");
    fs::create_dir(&inner).expect("mkdir");
    let db = Db::open_in_memory().expect("db");

    let trailing = PathBuf::from(format!("{}/", root.display()));
    let project = service(&db, FakeGit::rooted_at(&trailing))
        .add(&inner, 1)
        .expect("add");

    let stored = project.git_root.expect("a worktree root");
    assert!(
        !stored.ends_with('/') && !stored.ends_with('\\'),
        "stored as {stored:?}"
    );
    assert_eq!(Path::new(&stored), root);
}

#[test]
fn a_worktree_root_equal_to_the_project_root_is_not_repeated() {
    let dir = TempDir::new().expect("tempdir");
    let root = dir.path().canonicalize().expect("canonical");
    let db = Db::open_in_memory().expect("db");

    let project = service(&db, FakeGit::rooted_at(&root))
        .add(&root, 1)
        .expect("add");

    assert!(project.is_git);
    assert_eq!(
        project.git_root, None,
        "git_root is stored only when it differs (data-model §3.1)"
    );
}

#[test]
fn type_markers_are_detected_from_files_directly_inside_the_project() {
    let dir = TempDir::new().expect("tempdir");
    for name in ["package.json", "Cargo.toml", "docker-compose.yml"] {
        fs::write(dir.path().join(name), "{}").expect("write");
    }
    let db = Db::open_in_memory().expect("db");

    let project = service(&db, FakeGit::none())
        .add(dir.path(), 1)
        .expect("add");

    assert_eq!(project.markers, ["docker", "node", "rust"]);
}

#[test]
fn marker_detection_never_walks_into_subdirectories() {
    // The risk this guards is in `prd.md` feature 1: probing must look at depth 1
    // only, never recurse. A marker one level down must not be found.
    let dir = TempDir::new().expect("tempdir");
    fs::create_dir(dir.path().join("service")).expect("mkdir");
    fs::write(dir.path().join("service/go.mod"), "module x").expect("write");
    let db = Db::open_in_memory().expect("db");

    let project = service(&db, FakeGit::none())
        .add(dir.path(), 1)
        .expect("add");

    assert!(project.markers.is_empty());
}

#[test]
fn a_directory_named_only_by_a_separator_still_gets_a_name() {
    let db = Db::open_in_memory().expect("db");

    let project = service(&db, FakeGit::none())
        .add(Path::new("/"), 1)
        .expect("add");

    assert!(
        !project.name.is_empty(),
        "a project always has something to call it"
    );
}

// ── Listing, opening, removing ───────────────────────────────────────────────

#[test]
fn projects_list_most_recently_opened_first() {
    let dir = TempDir::new().expect("tempdir");
    let db = Db::open_in_memory().expect("db");
    let projects = service(&db, FakeGit::none());
    for name in ["one", "two", "three"] {
        fs::create_dir(dir.path().join(name)).expect("mkdir");
    }
    let one = projects.add(&dir.path().join("one"), 100).expect("add");
    projects.add(&dir.path().join("two"), 200).expect("add");
    projects.add(&dir.path().join("three"), 300).expect("add");

    projects.open(one.id, 400).expect("open");

    let names: Vec<String> = projects
        .list()
        .expect("list")
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, ["one", "three", "two"]);
}

#[test]
fn opening_a_project_returns_it_with_the_new_timestamp() {
    let dir = TempDir::new().expect("tempdir");
    let db = Db::open_in_memory().expect("db");
    let projects = service(&db, FakeGit::none());
    let project = projects.add(dir.path(), 100).expect("add");

    let reopened = projects.open(project.id, 900).expect("open");

    assert_eq!(reopened.id, project.id);
    assert_eq!(reopened.last_opened_at, Some(900));
}

#[test]
fn opening_a_project_that_was_removed_is_not_found() {
    let dir = TempDir::new().expect("tempdir");
    let db = Db::open_in_memory().expect("db");
    let projects = service(&db, FakeGit::none());
    let project = projects.add(dir.path(), 1).expect("add");
    projects.remove(project.id).expect("remove");

    assert!(matches!(
        projects.open(project.id, 2),
        Err(MiraError::NotFound { .. })
    ));
}

#[test]
fn removing_a_project_leaves_the_directory_alone() {
    let dir = TempDir::new().expect("tempdir");
    fs::write(dir.path().join("keep.txt"), "still here").expect("write");
    let db = Db::open_in_memory().expect("db");
    let projects = service(&db, FakeGit::none());
    let project = projects.add(dir.path(), 1).expect("add");

    projects.remove(project.id).expect("remove");

    assert_eq!(db.count().expect("count"), 0);
    assert_eq!(
        fs::read_to_string(dir.path().join("keep.txt")).expect("read"),
        "still here",
        "AC-1.3: removal never touches the filesystem"
    );
}

#[test]
fn several_projects_keep_their_own_git_answers() {
    let dir = TempDir::new().expect("tempdir");
    let db = Db::open_in_memory().expect("db");
    for name in ["with-git", "without-git"] {
        fs::create_dir(dir.path().join(name)).expect("mkdir");
    }
    let tracked = dir
        .path()
        .join("with-git")
        .canonicalize()
        .expect("canonical");

    service(&db, FakeGit::rooted_at(&tracked))
        .add(&tracked, 100)
        .expect("add");
    service(&db, FakeGit::none())
        .add(&dir.path().join("without-git"), 200)
        .expect("add");

    let all = service(&db, FakeGit::none()).list().expect("list");
    let git: Vec<bool> = all.iter().map(|p| p.is_git).collect();

    assert_eq!(
        git,
        [false, true],
        "most recent first; each keeps its own state"
    );
}
