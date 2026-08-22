//! Which of Mira's actions belong to a workspace.
//!
//! The repository stores identities; this layer decides what a caller is allowed
//! to ask for and reads the workspace first so that one that is gone says so.
//!
//! The rule the file is really about is isolation. Two workspaces on one project
//! keep separate lists, and neither can add to, remove from, or read the other's
//! — which matters more here than for services, because an action is a thing
//! that *happens*.

use mira_core::action::ActionId;
use mira_core::{MiraError, ProjectId, WorkspaceId};
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

fn workspace(db: &Db, project_id: ProjectId, name: &str) -> WorkspaceId {
    service(db)
        .create(project_id, name, None, 1_800_000_000)
        .expect("create")
        .id
}

fn act(raw: &str) -> ActionId {
    raw.parse().expect("a catalogue id")
}

#[test]
fn a_workspace_has_no_actions_until_somebody_gives_it_one() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");

    assert!(service(&db).actions(web).expect("actions").is_empty());
}

#[test]
fn giving_an_action_returns_the_whole_list_back() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");

    let after = service(&db)
        .set_action(web, &act("open-editor"), true, 1_800_000_100)
        .expect("give");

    assert_eq!(after, [act("open-editor")]);
}

#[test]
fn taking_an_action_away_returns_the_whole_list_back() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");

    service(&db)
        .set_action(web, &act("open-editor"), true, 1_800_000_100)
        .expect("give");
    service(&db)
        .set_action(web, &act("refresh"), true, 1_800_000_100)
        .expect("give");

    let after = service(&db)
        .set_action(web, &act("open-editor"), false, 1_800_000_200)
        .expect("take away");

    assert_eq!(after, [act("refresh")]);
}

#[test]
fn a_workspace_that_is_gone_says_so_before_anything_is_written() {
    let db = Db::open_in_memory().expect("db");

    assert!(matches!(
        service(&db).actions(WorkspaceId::new(404)),
        Err(MiraError::NotFound { .. })
    ));
    assert!(matches!(
        service(&db).set_action(WorkspaceId::new(404), &act("refresh"), true, 1),
        Err(MiraError::NotFound { .. })
    ));
}

#[test]
fn an_id_the_catalogue_has_no_row_for_is_stored_only_if_the_caller_allowed_it() {
    // This layer does not know a catalogue exists — that is the command layer's
    // check, and it is asserted there. What this layer guarantees is that it
    // does not *invent* one: what goes in comes out.
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");

    let after = service(&db)
        .set_action(web, &act("not-a-catalogue-row"), true, 1_800_000_100)
        .expect("store");

    assert_eq!(after, [act("not-a-catalogue-row")]);
}

// ── Isolation ────────────────────────────────────────────────────────────────

#[test]
fn two_workspaces_on_one_project_keep_independent_action_lists() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");
    let api = workspace(&db, aviora, "API");

    service(&db)
        .set_action(web, &act("open-editor"), true, 1_800_000_100)
        .expect("give");
    service(&db)
        .set_action(api, &act("open-terminal"), true, 1_800_000_100)
        .expect("give");

    assert_eq!(
        service(&db).actions(web).expect("actions"),
        [act("open-editor")]
    );
    assert_eq!(
        service(&db).actions(api).expect("actions"),
        [act("open-terminal")]
    );
}

#[test]
fn taking_an_action_from_one_workspace_leaves_the_other_holding_it() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");
    let api = workspace(&db, aviora, "API");

    for id in [web, api] {
        service(&db)
            .set_action(id, &act("refresh"), true, 1_800_000_100)
            .expect("give");
    }

    service(&db)
        .set_action(web, &act("refresh"), false, 1_800_000_200)
        .expect("take away");

    assert!(service(&db).actions(web).expect("actions").is_empty());
    assert_eq!(
        service(&db).actions(api).expect("actions"),
        [act("refresh")]
    );
}

#[test]
fn workspaces_on_different_projects_do_not_share_actions_either() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let mira = project(&db, "Mira", "/home/dev/mira");
    let ours = workspace(&db, aviora, "Web");
    let theirs = workspace(&db, mira, "Web");

    service(&db)
        .set_action(ours, &act("open-editor"), true, 1_800_000_100)
        .expect("give");

    assert!(service(&db).actions(theirs).expect("actions").is_empty());
}

#[test]
fn every_catalogue_action_can_be_given_and_taken_away() {
    // Not a formality: it is what makes "the catalogue is the whole surface"
    // testable, and it fails the moment a row is added that the layer cannot
    // store.
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");

    for action in mira_core::CATALOGUE {
        service(&db)
            .set_action(web, &act(action.id), true, 1_800_000_100)
            .expect("give");
    }
    assert_eq!(
        service(&db).actions(web).expect("actions").len(),
        mira_core::CATALOGUE.len()
    );

    for action in mira_core::CATALOGUE {
        service(&db)
            .set_action(web, &act(action.id), false, 1_800_000_200)
            .expect("take away");
    }
    assert!(service(&db).actions(web).expect("actions").is_empty());
}
