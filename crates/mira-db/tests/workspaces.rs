//! The workspace repository.
//!
//! A workspace is a *way of working on* one project. That relationship is the
//! thing these tests hold: it is stored, it is unique per project by name, it
//! survives everything except the project itself, and when the project goes the
//! workspaces go with it — deliberately, because a way of working on a project
//! that no longer exists is not a thing a person can be given back.

use mira_core::{AppKind, MiraError, ProjectId, WorkspaceId};
use mira_db::{Db, NewProject, NewWorkspace, ProjectRepo, WorkspaceRepo};

fn project(db: &Db, name: &str, root: &str) -> ProjectId {
    db.insert(
        &NewProject {
            name: name.to_owned(),
            root_path: root.to_owned(),
            is_git: false,
            git_root: None,
            markers: Vec::new(),
        },
        1_800_000_000,
    )
    .expect("insert project")
    .id
}

fn new(project_id: ProjectId, name: &str) -> NewWorkspace {
    NewWorkspace {
        project_id,
        name: name.to_owned(),
        description: None,
    }
}

// ── Creating and reading ─────────────────────────────────────────────────────

#[test]
fn a_created_workspace_comes_back_with_what_was_stored() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");

    let saved = db
        .create(
            &NewWorkspace {
                description: Some("The web app and its API.".to_owned()),
                ..new(aviora, "Web Development")
            },
            1_800_000_100,
        )
        .expect("create");

    assert_eq!(saved.project_id, aviora);
    assert_eq!(saved.name, "Web Development");
    assert_eq!(
        saved.description.as_deref(),
        Some("The web app and its API.")
    );
    assert_eq!(saved.created_at, 1_800_000_100);
    assert_eq!(saved.updated_at, 1_800_000_100);
    assert_eq!(
        saved.last_opened_at, None,
        "created is not opened; a workspace you have never opened should not sort as if you had"
    );
    assert!(saved.applications.is_empty(), "no application context yet");
}

#[test]
fn a_description_is_optional() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");

    let saved = db.create(&new(aviora, "Backend"), 1).expect("create");

    assert_eq!(saved.description, None);
}

#[test]
fn one_project_holds_many_independent_workspaces() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");

    for name in ["Web Development", "Mobile Development", "API Development"] {
        db.create(&new(aviora, name), 1).expect("create");
    }

    let listed = db.list_for(aviora).expect("list");
    assert_eq!(listed.len(), 3);
    assert_eq!(
        listed.iter().map(|w| w.name.clone()).collect::<Vec<_>>(),
        ["API Development", "Mobile Development", "Web Development"],
        "never opened, so ordered by name rather than by an absent timestamp"
    );
}

#[test]
fn two_projects_do_not_see_each_others_workspaces() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let mobile = project(&db, "Mobile", "/home/dev/mobile");

    db.create(&new(aviora, "Web Development"), 1)
        .expect("create");
    db.create(&new(mobile, "Web Development"), 1)
        .expect("create");

    assert_eq!(db.list_for(aviora).expect("list").len(), 1);
    assert_eq!(db.list_for(mobile).expect("list").len(), 1);
}

#[test]
fn one_project_cannot_hold_two_workspaces_of_the_same_name() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    db.create(&new(aviora, "Web Development"), 1)
        .expect("create");

    assert!(
        db.create(&new(aviora, "Web Development"), 2).is_err(),
        "UNIQUE (project_id, name) — two identically named workspaces are indistinguishable"
    );
    assert_eq!(db.list_for(aviora).expect("list").len(), 1);
}

#[test]
fn a_workspace_that_was_never_created_is_not_found() {
    let db = Db::open_in_memory().expect("open");

    assert!(matches!(
        db.get_workspace(WorkspaceId::new(404)),
        Err(MiraError::NotFound { .. })
    ));
}

// ── Opening and renaming ─────────────────────────────────────────────────────

#[test]
fn opening_a_workspace_records_when() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 100).expect("create");

    db.touch_workspace(web.id, 500).expect("open");

    assert_eq!(
        db.get_workspace(web.id).expect("get").last_opened_at,
        Some(500)
    );
}

#[test]
fn opening_one_workspace_does_not_touch_another() {
    // Slice brief §7: opening one workspace must not mutate another.
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 100).expect("create");
    let api = db.create(&new(aviora, "API"), 100).expect("create");

    db.touch_workspace(web.id, 900).expect("open");

    let untouched = db.get_workspace(api.id).expect("get");
    assert_eq!(untouched.last_opened_at, None);
    assert_eq!(untouched.updated_at, 100);
}

#[test]
fn workspaces_list_most_recently_opened_first() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 100).expect("create");
    db.create(&new(aviora, "Mobile"), 100).expect("create");
    let api = db.create(&new(aviora, "API"), 100).expect("create");

    db.touch_workspace(api.id, 200).expect("open");
    db.touch_workspace(web.id, 300).expect("open");

    let names: Vec<String> = db
        .list_for(aviora)
        .expect("list")
        .into_iter()
        .map(|w| w.name)
        .collect();
    assert_eq!(
        names,
        ["Web", "API", "Mobile"],
        "opened ones first, most recent leading; never-opened fall to the end"
    );
}

#[test]
fn renaming_a_workspace_keeps_everything_else() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 100).expect("create");
    db.touch_workspace(web.id, 200).expect("open");

    db.rename_workspace(
        web.id,
        "Web Development",
        Some("Now with a description."),
        300,
    )
    .expect("rename");

    let renamed = db.get_workspace(web.id).expect("get");
    assert_eq!(renamed.name, "Web Development");
    assert_eq!(
        renamed.description.as_deref(),
        Some("Now with a description.")
    );
    assert_eq!(renamed.created_at, 100, "created is when it was created");
    assert_eq!(renamed.updated_at, 300);
    assert_eq!(renamed.last_opened_at, Some(200), "renaming is not opening");
}

#[test]
fn renaming_to_a_name_the_project_already_uses_is_refused() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    db.create(&new(aviora, "Web"), 1).expect("create");
    let api = db.create(&new(aviora, "API"), 1).expect("create");

    assert!(db.rename_workspace(api.id, "Web", None, 2).is_err());
    assert_eq!(db.get_workspace(api.id).expect("get").name, "API");
}

#[test]
fn renaming_a_workspace_that_is_gone_is_not_found() {
    let db = Db::open_in_memory().expect("open");

    assert!(matches!(
        db.rename_workspace(WorkspaceId::new(404), "x", None, 1),
        Err(MiraError::NotFound { .. })
    ));
}

// ── Removing ─────────────────────────────────────────────────────────────────

#[test]
fn removing_a_workspace_leaves_its_siblings() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 1).expect("create");
    db.create(&new(aviora, "API"), 1).expect("create");

    db.remove_workspace(web.id).expect("remove");

    let left = db.list_for(aviora).expect("list");
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].name, "API");
}

#[test]
fn removing_a_workspace_that_is_gone_is_not_found() {
    let db = Db::open_in_memory().expect("open");

    assert!(matches!(
        db.remove_workspace(WorkspaceId::new(404)),
        Err(MiraError::NotFound { .. })
    ));
}

#[test]
fn removing_a_project_removes_its_workspaces() {
    // The deletion policy, stated as a test. A workspace is a way of working on a
    // project; without the project there is nothing left for it to describe, and
    // an orphan the user cannot see or reach is worse than a clean removal. The
    // project's own confirmation names what goes with it.
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let mobile = project(&db, "Mobile", "/home/dev/mobile");
    db.create(&new(aviora, "Web"), 1).expect("create");
    db.create(&new(aviora, "API"), 1).expect("create");
    let kept = db.create(&new(mobile, "Mobile Web"), 1).expect("create");

    db.remove(aviora).expect("remove project");

    assert!(db.list_for(aviora).expect("list").is_empty());
    assert_eq!(
        db.get_workspace(kept.id).expect("get").name,
        "Mobile Web",
        "another project's workspaces are untouched"
    );
}

#[test]
fn removing_a_project_removes_its_workspaces_application_context_too() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 1).expect("create");
    db.set_workspace_applications(web.id, &[AppKind::Editor], 1)
        .expect("set");

    db.remove(aviora).expect("remove project");

    let orphans: u32 = db
        .with_connection(|conn| {
            conn.query_row("SELECT COUNT(*) FROM workspace_applications", [], |row| {
                row.get(0)
            })
        })
        .expect("count");
    assert_eq!(orphans, 0, "ON DELETE CASCADE, two levels down");
}

// ── Application context ──────────────────────────────────────────────────────

#[test]
fn a_workspace_remembers_the_kinds_of_application_it_works_with() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 1).expect("create");

    db.set_workspace_applications(web.id, &[AppKind::Editor, AppKind::Browser], 100)
        .expect("set");

    assert_eq!(
        db.get_workspace(web.id).expect("get").applications,
        [AppKind::Editor, AppKind::Browser],
        "in the order the kinds are declared, so the list does not reshuffle"
    );
}

#[test]
fn setting_the_application_context_replaces_it_rather_than_adding_to_it() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 1).expect("create");

    db.set_workspace_applications(web.id, &[AppKind::Editor, AppKind::Terminal], 1)
        .expect("set");
    db.set_workspace_applications(web.id, &[AppKind::Browser], 2)
        .expect("set");

    assert_eq!(
        db.get_workspace(web.id).expect("get").applications,
        [AppKind::Browser]
    );
}

#[test]
fn the_same_kind_twice_is_stored_once() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 1).expect("create");

    db.set_workspace_applications(web.id, &[AppKind::Editor, AppKind::Editor], 1)
        .expect("set");

    assert_eq!(
        db.get_workspace(web.id).expect("get").applications,
        [AppKind::Editor]
    );
}

#[test]
fn one_workspaces_application_context_is_not_anothers() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 1).expect("create");
    let api = db.create(&new(aviora, "API"), 1).expect("create");

    db.set_workspace_applications(web.id, &[AppKind::Editor], 1)
        .expect("set");
    db.set_workspace_applications(api.id, &[AppKind::Terminal], 1)
        .expect("set");

    assert_eq!(
        db.get_workspace(web.id).expect("get").applications,
        [AppKind::Editor]
    );
    assert_eq!(
        db.get_workspace(api.id).expect("get").applications,
        [AppKind::Terminal]
    );
}

#[test]
fn nothing_observed_about_a_workspace_is_stored() {
    // `data-model.md` §1 rule 2, checked against the table itself. Git state,
    // ports and processes are read live; a column for any of them here would be a
    // cache that goes wrong silently.
    let db = Db::open_in_memory().expect("open");

    let columns: Vec<String> = db
        .with_connection(|conn| {
            let mut statement = conn.prepare("SELECT name FROM pragma_table_info('workspaces')")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect()
        })
        .expect("columns");

    for observed in [
        "branch", "dirty", "changed", "git", "port", "ports", "pid", "process", "cpu", "memory",
        "status",
    ] {
        assert!(
            !columns.iter().any(|column| column.contains(observed)),
            "`{observed}` is observed, not stated: {columns:?}"
        );
    }
}
