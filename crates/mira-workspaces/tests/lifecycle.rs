//! The workspace service.
//!
//! The repository stores rows; this decides what a person is allowed to ask for
//! and what they are told when the answer is no. Names are trimmed, blank ones
//! refused, and a duplicate is reported by naming the workspace already using it
//! rather than surfacing a constraint violation.

use mira_core::{AppKind, MiraError, ProjectId, WorkspaceId};
use mira_db::{Db, NewProject, ProjectRepo};
use mira_workspaces::{WorkspaceService, Workspaces};

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

fn service(db: &Db) -> Workspaces<&Db> {
    Workspaces::new(db)
}

// ── Creating ─────────────────────────────────────────────────────────────────

#[test]
fn a_workspace_is_created_against_a_project() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");

    let web = service(&db)
        .create(aviora, "Web Development", None, 100)
        .expect("create");

    assert_eq!(web.project_id, aviora);
    assert_eq!(web.name, "Web Development");
    assert_eq!(web.last_opened_at, None);
}

#[test]
fn a_name_is_trimmed_before_it_is_stored() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");

    let web = service(&db)
        .create(aviora, "  Web Development  ", None, 1)
        .expect("create");

    assert_eq!(web.name, "Web Development");
}

#[test]
fn a_blank_name_is_refused() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");

    for blank in ["", "   ", "\t\n"] {
        match service(&db).create(aviora, blank, None, 1) {
            Err(MiraError::Invalid { field, .. }) => assert_eq!(field, "name"),
            other => panic!("a blank name must be Invalid, got {other:?}"),
        }
    }
    assert_eq!(service(&db).list_for(aviora).expect("list").len(), 0);
}

#[test]
fn a_blank_description_is_stored_as_none_rather_than_as_emptiness() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");

    let web = service(&db)
        .create(aviora, "Web", Some("   "), 1)
        .expect("create");

    assert_eq!(web.description, None);
}

#[test]
fn a_second_workspace_of_the_same_name_names_the_one_already_there() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    service(&db).create(aviora, "Web", None, 1).expect("create");

    match service(&db).create(aviora, "  Web ", None, 2) {
        Err(MiraError::Invalid { field, detail }) => {
            assert_eq!(field, "name");
            assert!(
                detail.contains("Web"),
                "the message names the workspace already using it: {detail}"
            );
        }
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn a_workspace_cannot_be_created_against_a_project_that_does_not_exist() {
    let db = Db::open_in_memory().expect("db");

    assert!(matches!(
        service(&db).create(ProjectId::new(404), "Web", None, 1),
        Err(MiraError::NotFound { .. })
    ));
}

// ── Listing, opening, renaming ───────────────────────────────────────────────

#[test]
fn a_projects_workspaces_are_listed_for_it_alone() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let mobile = project(&db, "Mobile", "/home/dev/mobile");
    for name in ["Web Development", "Mobile Development", "API Development"] {
        service(&db).create(aviora, name, None, 1).expect("create");
    }
    service(&db)
        .create(mobile, "Other", None, 1)
        .expect("create");

    assert_eq!(service(&db).list_for(aviora).expect("list").len(), 3);
    assert_eq!(service(&db).list_for(mobile).expect("list").len(), 1);
}

#[test]
fn opening_a_workspace_returns_it_with_the_new_timestamp() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = service(&db)
        .create(aviora, "Web", None, 100)
        .expect("create");

    let opened = service(&db).open(web.id, 900).expect("open");

    assert_eq!(opened.id, web.id);
    assert_eq!(opened.last_opened_at, Some(900));
}

#[test]
fn opening_a_workspace_changes_nothing_about_its_siblings() {
    // Slice brief §7: opening one workspace must not mutate another.
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = service(&db)
        .create(aviora, "Web", None, 100)
        .expect("create");
    let api = service(&db)
        .create(aviora, "API", None, 100)
        .expect("create");

    service(&db).open(web.id, 900).expect("open");

    let untouched = service(&db).get(api.id).expect("get");
    assert_eq!(untouched.last_opened_at, None);
    assert_eq!(untouched.updated_at, 100);
}

#[test]
fn renaming_refuses_a_blank_name() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = service(&db).create(aviora, "Web", None, 1).expect("create");

    assert!(matches!(
        service(&db).rename(web.id, "  ", None, 2),
        Err(MiraError::Invalid { .. })
    ));
    assert_eq!(service(&db).get(web.id).expect("get").name, "Web");
}

#[test]
fn renaming_to_a_name_a_sibling_uses_is_refused_by_name() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    service(&db).create(aviora, "Web", None, 1).expect("create");
    let api = service(&db).create(aviora, "API", None, 1).expect("create");

    match service(&db).rename(api.id, "Web", None, 2) {
        Err(MiraError::Invalid { detail, .. }) => assert!(detail.contains("Web")),
        other => panic!("expected Invalid, got {other:?}"),
    }
}

#[test]
fn a_workspace_may_be_renamed_to_what_it_is_already_called() {
    // Editing the description without touching the name must not trip the
    // uniqueness check against the workspace's own row.
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = service(&db).create(aviora, "Web", None, 1).expect("create");

    service(&db)
        .rename(web.id, "Web", Some("Now described."), 2)
        .expect("rename");

    let renamed = service(&db).get(web.id).expect("get");
    assert_eq!(renamed.name, "Web");
    assert_eq!(renamed.description.as_deref(), Some("Now described."));
}

// ── Removing ─────────────────────────────────────────────────────────────────

#[test]
fn removing_a_workspace_leaves_the_project_and_its_siblings() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = service(&db).create(aviora, "Web", None, 1).expect("create");
    service(&db).create(aviora, "API", None, 1).expect("create");

    service(&db).remove(web.id).expect("remove");

    assert_eq!(service(&db).list_for(aviora).expect("list").len(), 1);
    assert_eq!(db.count().expect("projects"), 1);
}

#[test]
fn removing_a_workspace_that_is_gone_is_not_found() {
    let db = Db::open_in_memory().expect("db");

    assert!(matches!(
        service(&db).remove(WorkspaceId::new(404)),
        Err(MiraError::NotFound { .. })
    ));
}

// ── Application context ──────────────────────────────────────────────────────

#[test]
fn a_workspace_remembers_which_kinds_of_application_it_works_with() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = service(&db).create(aviora, "Web", None, 1).expect("create");

    let updated = service(&db)
        .set_applications(web.id, &[AppKind::Editor, AppKind::Browser], 2)
        .expect("set");

    assert_eq!(updated.applications, [AppKind::Editor, AppKind::Browser]);
}

#[test]
fn clearing_the_application_context_is_sending_an_empty_list() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = service(&db).create(aviora, "Web", None, 1).expect("create");
    service(&db)
        .set_applications(web.id, &[AppKind::Editor], 2)
        .expect("set");

    let cleared = service(&db)
        .set_applications(web.id, &[], 3)
        .expect("clear");

    assert!(cleared.applications.is_empty());
}

#[test]
fn setting_the_application_context_of_a_workspace_that_is_gone_is_not_found() {
    let db = Db::open_in_memory().expect("db");

    assert!(matches!(
        service(&db).set_applications(WorkspaceId::new(404), &[AppKind::Editor], 1),
        Err(MiraError::NotFound { .. })
    ));
}

#[test]
fn the_service_still_reports_how_many_workspaces_exist() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    service(&db).create(aviora, "Web", None, 1).expect("create");
    service(&db).create(aviora, "API", None, 1).expect("create");

    assert_eq!(service(&db).count().expect("count"), 2);
}
