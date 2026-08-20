//! Where a workspace's applications open.
//!
//! A workspace has no directory of its own (ADR-0012), so the answer is always
//! its *project's* root — and the interesting part is the ways of not having one.
//! A project that was removed, a folder that was moved, a workspace that no
//! longer exists: three different sentences, none of which may end in something
//! being launched (slice brief §11).

use std::fs;

use mira_core::{MiraError, ProjectId, WorkspaceId};
use mira_db::{Db, NewProject, ProjectRepo};
use mira_workspaces::{WorkspaceService, Workspaces};
use tempfile::TempDir;

fn project(db: &Db, name: &str, root: &std::path::Path) -> ProjectId {
    db.insert(
        &NewProject {
            name: name.to_owned(),
            root_path: root.to_string_lossy().into_owned(),
            is_git: false,
            git_root: None,
            markers: Vec::new(),
        },
        1_800_000_000,
    )
    .expect("insert project")
    .id
}

fn service(db: &Db) -> Workspaces<&Db> {
    Workspaces::new(db)
}

#[test]
fn a_workspace_opens_at_its_projects_directory() {
    let db = Db::open_in_memory().expect("db");
    let folder = TempDir::new().expect("folder");
    let aviora = project(&db, "Aviora", folder.path());
    let web = service(&db).create(aviora, "Web", None, 1).expect("create");

    let root = service(&db).working_directory(web.id).expect("root");

    assert_eq!(
        root,
        fs::canonicalize(folder.path()).expect("canonical"),
        "the canonical root, so a symlinked home is still one directory"
    );
}

#[test]
fn every_workspace_on_a_project_opens_at_the_same_directory() {
    // The relationship, stated as a test: a workspace narrows nothing about
    // *where*, only about what you call the work.
    let db = Db::open_in_memory().expect("db");
    let folder = TempDir::new().expect("folder");
    let aviora = project(&db, "Aviora", folder.path());
    let web = service(&db).create(aviora, "Web", None, 1).expect("create");
    let api = service(&db).create(aviora, "API", None, 1).expect("create");

    assert_eq!(
        service(&db).working_directory(web.id).expect("web"),
        service(&db).working_directory(api.id).expect("api")
    );
}

#[test]
fn workspaces_on_different_projects_open_at_different_directories() {
    let db = Db::open_in_memory().expect("db");
    let one = TempDir::new().expect("one");
    let two = TempDir::new().expect("two");
    let web = service(&db)
        .create(project(&db, "Aviora", one.path()), "Web", None, 1)
        .expect("create");
    let app = service(&db)
        .create(project(&db, "Mobile", two.path()), "App", None, 1)
        .expect("create");

    assert_ne!(
        service(&db).working_directory(web.id).expect("web"),
        service(&db).working_directory(app.id).expect("app")
    );
}

#[test]
fn a_workspace_whose_folder_is_missing_opens_nothing() {
    // FR-1.5: the folder is observed, the workspace is stated. The workspace
    // survives; there is simply nowhere to open.
    let db = Db::open_in_memory().expect("db");
    let folder = TempDir::new().expect("folder");
    let path = folder.path().to_path_buf();
    let aviora = project(&db, "Aviora", &path);
    let web = service(&db).create(aviora, "Web", None, 1).expect("create");
    drop(folder);

    match service(&db).working_directory(web.id) {
        Err(MiraError::NotFound { what }) => assert!(
            what.contains("Aviora"),
            "the message names the project, not a row id: {what}"
        ),
        other => panic!("expected NotFound, got {other:?}"),
    }

    assert_eq!(
        service(&db).get(web.id).expect("get").name,
        "Web",
        "and the workspace is still there"
    );
}

#[test]
fn a_workspace_that_does_not_exist_opens_nothing() {
    let db = Db::open_in_memory().expect("db");

    assert!(matches!(
        service(&db).working_directory(WorkspaceId::new(404)),
        Err(MiraError::NotFound { .. })
    ));
}

#[test]
fn a_workspace_whose_project_was_removed_opens_nothing() {
    // The cascade takes the workspace with the project, so this is `NotFound`
    // for the workspace rather than a dangling reference to a project row.
    let db = Db::open_in_memory().expect("db");
    let folder = TempDir::new().expect("folder");
    let aviora = project(&db, "Aviora", folder.path());
    let web = service(&db).create(aviora, "Web", None, 1).expect("create");

    db.remove(aviora).expect("remove project");

    assert!(matches!(
        service(&db).working_directory(web.id),
        Err(MiraError::NotFound { .. })
    ));
}

#[test]
fn a_file_where_the_project_should_be_opens_nothing() {
    let db = Db::open_in_memory().expect("db");
    let folder = TempDir::new().expect("folder");
    let file = folder.path().join("not-a-project");
    fs::write(&file, "").expect("write");
    let aviora = project(&db, "Aviora", &file);
    let web = service(&db).create(aviora, "Web", None, 1).expect("create");

    assert!(matches!(
        service(&db).working_directory(web.id),
        Err(MiraError::Invalid { .. })
    ));
}
