//! The workspace repository.
//!
//! A workspace is a *way of working on* one project. That relationship is the
//! thing these tests hold: it is stored, it is unique per project by name, it
//! survives everything except the project itself, and when the project goes the
//! workspaces go with it — deliberately, because a way of working on a project
//! that no longer exists is not a thing a person can be given back.

use mira_core::service::Port;
use mira_core::{AppKind, MiraError, ProjectId, WorkspaceId, WorkspaceServiceId};
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

// ── Which application, per workspace ─────────────────────────────────────────
//
// The kind is intent and lives above; this is the *choice*, and the property
// worth holding is that it is per workspace and stored as an identity rather
// than as anything that could be run.

fn app(id: &str) -> mira_core::AppId {
    id.parse().expect("an application id")
}

#[test]
fn a_workspace_remembers_which_application_it_was_given() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    db.set_workspace_preference(web.id, AppKind::Editor, Some(&app("zed")), 1_800_000_100)
        .expect("prefer");

    let read = db.get_workspace(web.id).expect("read");
    assert_eq!(read.preferences.len(), 1);
    assert_eq!(read.preferences[0].kind, AppKind::Editor);
    assert_eq!(read.preferences[0].application, app("zed"));
}

#[test]
fn a_choice_survives_being_closed_and_opened_again() {
    // Persistence across a restart, which for a file-backed database is the same
    // question as persistence across a reconnect.
    let file = tempfile::tempdir().expect("temp");
    let path = file.path().join("mira.db");

    let id = {
        let db = Db::open(&path).expect("open");
        let aviora = project(&db, "Aviora", "/home/dev/aviora");
        let web = db
            .create(&new(aviora, "Web"), 1_800_000_000)
            .expect("create");
        db.set_workspace_preference(web.id, AppKind::Terminal, Some(&app("ghostty")), 1)
            .expect("prefer");
        web.id
    };

    let reopened = Db::open(&path).expect("reopen");
    let read = reopened.get_workspace(id).expect("read");

    assert_eq!(read.preferences[0].application, app("ghostty"));
}

#[test]
fn choosing_for_one_workspace_leaves_every_other_alone() {
    // The isolation property, asserted across two workspaces on the *same*
    // project — which is the case where sharing would be easiest to do by
    // accident, because everything else about them is shared.
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 1).expect("create");
    let api = db.create(&new(aviora, "API"), 1).expect("create");
    let other = project(&db, "Other", "/home/dev/other");
    let apart = db.create(&new(other, "Apart"), 1).expect("create");

    db.set_workspace_preference(web.id, AppKind::Editor, Some(&app("zed")), 2)
        .expect("prefer");

    assert_eq!(
        db.get_workspace(web.id).expect("read").preferences[0].application,
        app("zed")
    );
    assert!(db
        .get_workspace(api.id)
        .expect("read")
        .preferences
        .is_empty());
    assert!(db
        .get_workspace(apart.id)
        .expect("read")
        .preferences
        .is_empty());
}

#[test]
fn choosing_again_replaces_rather_than_accumulates() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 1).expect("create");

    db.set_workspace_preference(web.id, AppKind::Editor, Some(&app("zed")), 2)
        .expect("prefer");
    db.set_workspace_preference(web.id, AppKind::Editor, Some(&app("cursor")), 3)
        .expect("prefer");

    let read = db.get_workspace(web.id).expect("read");
    assert_eq!(read.preferences.len(), 1, "one choice per kind");
    assert_eq!(read.preferences[0].application, app("cursor"));
}

#[test]
fn clearing_a_choice_puts_that_kind_back_on_automatic() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 1).expect("create");

    db.set_workspace_preference(web.id, AppKind::Editor, Some(&app("zed")), 2)
        .expect("prefer");
    db.set_workspace_preference(web.id, AppKind::Terminal, Some(&app("iterm")), 2)
        .expect("prefer");
    db.set_workspace_preference(web.id, AppKind::Editor, None, 3)
        .expect("clear");

    let read = db.get_workspace(web.id).expect("read");
    assert_eq!(read.preferences.len(), 1, "only the terminal is chosen now");
    assert_eq!(read.preferences[0].kind, AppKind::Terminal);
}

#[test]
fn choices_come_back_in_the_vocabulary_order() {
    // So the list reads the same every time, whichever order they were made in.
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 1).expect("create");

    for (kind, id) in [
        (AppKind::Browser, "firefox"),
        (AppKind::Terminal, "iterm"),
        (AppKind::Editor, "zed"),
    ] {
        db.set_workspace_preference(web.id, kind, Some(&app(id)), 2)
            .expect("prefer");
    }

    let kinds: Vec<AppKind> = db
        .get_workspace(web.id)
        .expect("read")
        .preferences
        .into_iter()
        .map(|preference| preference.kind)
        .collect();

    assert_eq!(
        kinds,
        vec![AppKind::Editor, AppKind::Terminal, AppKind::Browser]
    );
}

#[test]
fn a_choice_goes_when_its_workspace_goes() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 1).expect("create");

    db.set_workspace_preference(web.id, AppKind::Editor, Some(&app("zed")), 2)
        .expect("prefer");
    db.remove_workspace(web.id).expect("remove");

    let orphans: i64 = db
        .with_connection(|conn| {
            conn.query_row(
                "SELECT COUNT(*) FROM workspace_app_preferences",
                [],
                |row| row.get(0),
            )
        })
        .expect("count");

    assert_eq!(
        orphans, 0,
        "ON DELETE CASCADE, like every other workspace row"
    );
}

#[test]
fn choosing_for_a_workspace_that_is_gone_says_so() {
    let db = Db::open_in_memory().expect("open");

    let refused =
        db.set_workspace_preference(WorkspaceId::new(404), AppKind::Editor, Some(&app("zed")), 1);

    assert!(matches!(refused, Err(MiraError::NotFound { .. })));
}

#[test]
fn the_column_cannot_hold_anything_shaped_like_a_program() {
    // Defence in depth: the wall is that an id only becomes an application by
    // being found in the compiled catalogue. But a column that cannot hold a
    // slash, a space or a quote is a column nobody has to wonder about.
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db.create(&new(aviora, "Web"), 1).expect("create");

    for smuggled in [
        "/usr/bin/env",
        "code --wait",
        "rm -rf ~",
        "../../etc/passwd",
        "Visual Studio Code",
        "",
        &"x".repeat(33),
    ] {
        let written = db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO workspace_app_preferences \
                 (workspace_id, kind, application_id, chosen_at) VALUES (?1, 'editor', ?2, 1)",
                rusqlite::params![web.id.get(), smuggled],
            )
        });

        assert!(
            written.is_err(),
            "the schema accepted {smuggled:?} as an application id"
        );
    }
}

// ── Watched services ─────────────────────────────────────────────────────────

fn p(raw: u16) -> Port {
    Port::try_from(raw).expect("a port")
}

#[test]
fn a_workspace_starts_watching_nothing() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    assert!(db.workspace_services(web.id).expect("services").is_empty());
}

#[test]
fn a_watched_service_comes_back_with_what_was_stored() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    let added = db
        .watch_service(web.id, p(5_173), 1_800_000_100)
        .expect("watch");

    assert_eq!(added.workspace_id, web.id);
    assert_eq!(added.port, p(5_173));
    assert_eq!(added.added_at, 1_800_000_100);
    assert_eq!(db.workspace_services(web.id).expect("services"), [added]);
}

#[test]
fn watched_services_come_back_in_port_order() {
    // By port rather than by when it was added, so the list reads the same every
    // time and removing one does not reshuffle the rest.
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    for (port, at) in [(8_080, 1), (3_000, 2), (5_173, 3)] {
        db.watch_service(web.id, p(port), 1_800_000_000 + at)
            .expect("watch");
    }

    let ports: Vec<u16> = db
        .workspace_services(web.id)
        .expect("services")
        .iter()
        .map(|service| service.port.get())
        .collect();
    assert_eq!(ports, [3_000, 5_173, 8_080]);
}

#[test]
fn watching_the_same_port_twice_is_refused_by_name() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    db.watch_service(web.id, p(5_173), 1_800_000_100)
        .expect("watch");

    let again = db.watch_service(web.id, p(5_173), 1_800_000_200);
    assert!(
        matches!(again, Err(MiraError::Invalid { ref detail, .. }) if detail.contains("5173")),
        "got {again:?}"
    );
    assert_eq!(db.workspace_services(web.id).expect("services").len(), 1);
}

#[test]
fn two_workspaces_on_one_project_watch_different_services() {
    // The point of the feature. Same project, same observations, two different
    // views of which of them matter.
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");
    let api = db
        .create(&new(aviora, "API"), 1_800_000_000)
        .expect("create");

    db.watch_service(web.id, p(5_173), 1_800_000_100)
        .expect("watch");
    db.watch_service(api.id, p(8_080), 1_800_000_100)
        .expect("watch");

    let of = |id: WorkspaceId| -> Vec<u16> {
        db.workspace_services(id)
            .expect("services")
            .iter()
            .map(|service| service.port.get())
            .collect()
    };

    assert_eq!(of(web.id), [5_173]);
    assert_eq!(of(api.id), [8_080]);
}

#[test]
fn two_workspaces_may_watch_the_same_port() {
    // A shared dev server is one service two people are working against, not a
    // conflict. Uniqueness is per workspace, which is what the primary key says.
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");
    let api = db
        .create(&new(aviora, "API"), 1_800_000_000)
        .expect("create");

    db.watch_service(web.id, p(5_173), 1_800_000_100)
        .expect("watch");
    db.watch_service(api.id, p(5_173), 1_800_000_100)
        .expect("also watch");

    assert_eq!(db.workspace_services(web.id).expect("services").len(), 1);
    assert_eq!(db.workspace_services(api.id).expect("services").len(), 1);
}

#[test]
fn a_service_id_from_another_workspace_reaches_nothing() {
    // Isolation, at the layer that enforces it. A sibling's row id is not merely
    // hidden — the delete and the read are keyed by both, so it matches nothing.
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");
    let api = db
        .create(&new(aviora, "API"), 1_800_000_000)
        .expect("create");

    let theirs = db
        .watch_service(api.id, p(8_080), 1_800_000_100)
        .expect("watch");

    assert!(matches!(
        db.workspace_service(web.id, theirs.id),
        Err(MiraError::NotFound { .. })
    ));
    assert!(matches!(
        db.forget_service(web.id, theirs.id),
        Err(MiraError::NotFound { .. })
    ));
    assert_eq!(
        db.workspace_services(api.id).expect("services"),
        [theirs],
        "and the sibling still has it"
    );
}

#[test]
fn forgetting_a_service_removes_that_one_and_no_others() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    let vite = db
        .watch_service(web.id, p(5_173), 1_800_000_100)
        .expect("watch");
    db.watch_service(web.id, p(8_080), 1_800_000_100)
        .expect("watch");

    db.forget_service(web.id, vite.id).expect("forget");

    let ports: Vec<u16> = db
        .workspace_services(web.id)
        .expect("services")
        .iter()
        .map(|service| service.port.get())
        .collect();
    assert_eq!(ports, [8_080]);
}

#[test]
fn forgetting_a_service_that_is_not_there_says_so() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    assert!(matches!(
        db.forget_service(web.id, WorkspaceServiceId::new(404)),
        Err(MiraError::NotFound { .. })
    ));
}

#[test]
fn a_removed_workspace_takes_its_watched_services_with_it() {
    // Deletion policy, stated and tested. A workspace's services describe that
    // workspace, so without it there is nothing left for them to describe.
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");
    db.watch_service(web.id, p(5_173), 1_800_000_100)
        .expect("watch");

    db.remove_workspace(web.id).expect("remove");

    let rows: i64 = db
        .with_connection(|conn| {
            conn.query_row("SELECT count(*) FROM workspace_services", [], |row| {
                row.get(0)
            })
        })
        .expect("count");
    assert_eq!(rows, 0, "the cascade is the schema's, not the caller's");
}

#[test]
fn a_removed_project_takes_every_workspaces_services_with_it() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");
    let api = db
        .create(&new(aviora, "API"), 1_800_000_000)
        .expect("create");
    db.watch_service(web.id, p(5_173), 1_800_000_100)
        .expect("watch");
    db.watch_service(api.id, p(8_080), 1_800_000_100)
        .expect("watch");

    db.remove(aviora).expect("remove project");

    let rows: i64 = db
        .with_connection(|conn| {
            conn.query_row("SELECT count(*) FROM workspace_services", [], |row| {
                row.get(0)
            })
        })
        .expect("count");
    assert_eq!(rows, 0, "two cascades deep, and both are the schema's");
}

#[test]
fn the_stored_row_is_a_port_and_three_numbers() {
    // The absences, checked against the table rather than against the migration
    // file. A `label`, a `process` or a `path` column here would be observation
    // written down, or a URL waiting to be concatenated.
    let db = Db::open_in_memory().expect("open");

    let columns: Vec<String> = db
        .with_connection(|conn| {
            let mut statement =
                conn.prepare("SELECT name FROM pragma_table_info('workspace_services')")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect()
        })
        .expect("columns");

    assert_eq!(columns, ["id", "workspace_id", "port", "added_at"]);
}

#[test]
fn the_database_refuses_a_port_that_is_not_a_port() {
    // The `CHECK` constraint, tested directly. Mira's own path cannot produce
    // one — `Port` refused it long before — so this is the wall behind the wall.
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    for refused in [0_i64, -1, 65_536, 1_000_000] {
        let written = db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO workspace_services (workspace_id, port, added_at) VALUES (?1, ?2, ?3)",
                rusqlite::params![web.id.get(), refused, 1_800_000_100],
            )
        });
        assert!(written.is_err(), "{refused} was accepted");
    }
}

// ── Actions ──────────────────────────────────────────────────────────────────

fn act(raw: &str) -> mira_core::ActionId {
    raw.parse().expect("a catalogue id")
}

#[test]
fn a_workspace_starts_with_no_actions() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    assert!(db.workspace_actions(web.id).expect("actions").is_empty());
}

#[test]
fn an_action_given_to_a_workspace_comes_back() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    db.set_workspace_action(web.id, &act("open-editor"), true, 1_800_000_100)
        .expect("give");

    assert_eq!(
        db.workspace_actions(web.id).expect("actions"),
        [act("open-editor")]
    );
}

#[test]
fn actions_come_back_in_catalogue_order_however_they_were_added() {
    // Ordered by the catalogue rather than by insertion, so the list reads the
    // same every time and removing one does not reshuffle the rest.
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    for id in ["refresh", "open-editor", "reveal-project"] {
        db.set_workspace_action(web.id, &act(id), true, 1_800_000_100)
            .expect("give");
    }

    let order: Vec<String> = db
        .workspace_actions(web.id)
        .expect("actions")
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect();
    assert_eq!(order, ["open-editor", "reveal-project", "refresh"]);
}

#[test]
fn giving_the_same_action_twice_changes_nothing() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    db.set_workspace_action(web.id, &act("refresh"), true, 1_800_000_100)
        .expect("give");
    db.set_workspace_action(web.id, &act("refresh"), true, 1_800_000_200)
        .expect("give again");

    assert_eq!(db.workspace_actions(web.id).expect("actions").len(), 1);
}

#[test]
fn taking_an_action_away_removes_that_one_and_no_others() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    for id in ["open-editor", "refresh"] {
        db.set_workspace_action(web.id, &act(id), true, 1_800_000_100)
            .expect("give");
    }
    db.set_workspace_action(web.id, &act("open-editor"), false, 1_800_000_200)
        .expect("take away");

    assert_eq!(
        db.workspace_actions(web.id).expect("actions"),
        [act("refresh")]
    );
}

#[test]
fn taking_away_an_action_a_workspace_never_had_is_not_an_error() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    assert!(db
        .set_workspace_action(web.id, &act("refresh"), false, 1_800_000_100)
        .is_ok());
}

#[test]
fn a_stored_id_the_catalogue_no_longer_has_comes_back_last_rather_than_vanishing() {
    // The upgrade case: an action a later Mira removed. It must still be
    // returned, because a person needs to be told it is there and offered a way
    // to clear it — dropping it silently would leave a row nobody can see.
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    db.set_workspace_action(web.id, &act("open-editor"), true, 1_800_000_100)
        .expect("give");
    db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO workspace_actions (workspace_id, action, added_at) \
             VALUES (?1, 'an-action-from-the-future', ?2)",
            rusqlite::params![web.id.get(), 1_800_000_100],
        )
    })
    .expect("seed a stale row");

    let ids: Vec<String> = db
        .workspace_actions(web.id)
        .expect("actions")
        .iter()
        .map(|id| id.as_str().to_owned())
        .collect();
    assert_eq!(ids, ["open-editor", "an-action-from-the-future"]);
}

#[test]
fn two_workspaces_on_one_project_keep_separate_actions() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");
    let api = db
        .create(&new(aviora, "API"), 1_800_000_000)
        .expect("create");

    db.set_workspace_action(web.id, &act("open-editor"), true, 1_800_000_100)
        .expect("give");
    db.set_workspace_action(api.id, &act("refresh"), true, 1_800_000_100)
        .expect("give");

    assert_eq!(
        db.workspace_actions(web.id).expect("actions"),
        [act("open-editor")]
    );
    assert_eq!(
        db.workspace_actions(api.id).expect("actions"),
        [act("refresh")]
    );

    // And removing from one leaves the other alone.
    db.set_workspace_action(web.id, &act("open-editor"), false, 1_800_000_200)
        .expect("take away");
    assert!(db.workspace_actions(web.id).expect("actions").is_empty());
    assert_eq!(
        db.workspace_actions(api.id).expect("actions"),
        [act("refresh")]
    );
}

#[test]
fn a_removed_workspace_takes_its_actions_with_it() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");
    db.set_workspace_action(web.id, &act("refresh"), true, 1_800_000_100)
        .expect("give");

    db.remove_workspace(web.id).expect("remove");

    let rows: i64 = db
        .with_connection(|conn| {
            conn.query_row("SELECT count(*) FROM workspace_actions", [], |row| {
                row.get(0)
            })
        })
        .expect("count");
    assert_eq!(rows, 0, "the cascade is the schema's, not the caller's");
}

#[test]
fn a_removed_project_takes_every_workspaces_actions_with_it() {
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    for name in ["Web", "API"] {
        let workspace = db
            .create(&new(aviora, name), 1_800_000_000)
            .expect("create");
        db.set_workspace_action(workspace.id, &act("refresh"), true, 1_800_000_100)
            .expect("give");
    }

    db.remove(aviora).expect("remove project");

    let rows: i64 = db
        .with_connection(|conn| {
            conn.query_row("SELECT count(*) FROM workspace_actions", [], |row| {
                row.get(0)
            })
        })
        .expect("count");
    assert_eq!(rows, 0, "two cascades deep, and both are the schema's");
}

#[test]
fn the_stored_action_row_is_an_identity_and_two_numbers() {
    let db = Db::open_in_memory().expect("open");

    let columns: Vec<String> = db
        .with_connection(|conn| {
            let mut statement =
                conn.prepare("SELECT name FROM pragma_table_info('workspace_actions')")?;
            let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect()
        })
        .expect("columns");

    assert_eq!(columns, ["workspace_id", "action", "added_at"]);
}

#[test]
fn the_action_column_cannot_hold_anything_shaped_like_a_command() {
    // Defence in depth: the wall is that an id only becomes an action by being
    // found in the compiled catalogue. But a column that cannot hold a slash, a
    // space, a semicolon or a quote is a column nobody has to wonder about.
    let db = Db::open_in_memory().expect("open");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = db
        .create(&new(aviora, "Web"), 1_800_000_000)
        .expect("create");

    for smuggled in [
        "npm run dev",
        "pnpm -w build",
        "cargo run",
        "/usr/local/bin/node",
        "zsh -lc 'rm -rf ~'",
        "open-editor; rm -rf ~",
        "open-editor && curl evil",
        "../../etc/passwd",
        "Open-Editor",
        "",
        &"x".repeat(33),
    ] {
        let written = db.with_connection(|conn| {
            conn.execute(
                "INSERT INTO workspace_actions (workspace_id, action, added_at) \
                 VALUES (?1, ?2, 1)",
                rusqlite::params![web.id.get(), smuggled],
            )
        });

        assert!(
            written.is_err(),
            "the schema accepted {smuggled:?} as an action"
        );
    }
}
