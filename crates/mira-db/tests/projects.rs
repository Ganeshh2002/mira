//! The project repository.
//!
//! `data-model.md` §3.1 makes three promises this file holds to: one project per
//! directory (`root_path UNIQUE`), markers that cascade with their project, and an
//! order that puts the most recently opened first. Time is passed in rather than
//! read from a clock, so every assertion below is deterministic.

use mira_core::ProjectId;
use mira_db::{Db, NewProject, ProjectRepo};

fn seed(name: &str, root: &str) -> NewProject {
    NewProject {
        name: name.to_owned(),
        root_path: root.to_owned(),
        is_git: false,
        git_root: None,
        markers: Vec::new(),
    }
}

#[test]
fn an_added_project_comes_back_with_what_was_stored() {
    let db = Db::open_in_memory().expect("open");

    let saved = db
        .insert(&seed("Aviora", "/home/dev/aviora"), 1_700_000_000)
        .expect("insert");

    assert_eq!(saved.name, "Aviora");
    assert_eq!(saved.root_path, "/home/dev/aviora");
    assert_eq!(saved.created_at, 1_700_000_000);
    assert_eq!(saved.updated_at, 1_700_000_000);
    assert_eq!(
        saved.last_opened_at,
        Some(1_700_000_000),
        "adding a project is the first time it was opened"
    );
    assert!(!saved.is_git);
}

#[test]
fn a_git_project_stores_its_worktree_root() {
    let db = Db::open_in_memory().expect("open");
    let saved = db
        .insert(
            &NewProject {
                name: "Aviora".to_owned(),
                root_path: "/home/dev/aviora/web".to_owned(),
                is_git: true,
                git_root: Some("/home/dev/aviora".to_owned()),
                markers: vec!["node".to_owned()],
            },
            1,
        )
        .expect("insert");

    assert!(saved.is_git);
    assert_eq!(saved.git_root.as_deref(), Some("/home/dev/aviora"));
    assert_eq!(saved.markers, ["node"]);
}

#[test]
fn markers_come_back_in_a_stable_order() {
    let db = Db::open_in_memory().expect("open");
    let saved = db
        .insert(
            &NewProject {
                markers: vec!["rust".to_owned(), "node".to_owned(), "docker".to_owned()],
                ..seed("Mira", "/home/dev/mira")
            },
            1,
        )
        .expect("insert");

    assert_eq!(
        saved.markers,
        ["docker", "node", "rust"],
        "sorted, so the list does not reshuffle between reads"
    );
}

#[test]
fn one_directory_holds_one_project() {
    let db = Db::open_in_memory().expect("open");
    db.insert(&seed("Aviora", "/home/dev/aviora"), 1)
        .expect("insert");

    let again = db.insert(&seed("Another name", "/home/dev/aviora"), 2);

    assert!(
        again.is_err(),
        "FR-1.1: root_path is UNIQUE, so a second project on one directory is refused"
    );
    assert_eq!(db.count().expect("count"), 1);
}

#[test]
fn a_directory_already_registered_is_findable_before_the_insert_is_attempted() {
    let db = Db::open_in_memory().expect("open");
    db.insert(&seed("Aviora", "/home/dev/aviora"), 1)
        .expect("insert");

    let found = db.find_by_root("/home/dev/aviora").expect("find");
    assert_eq!(found.expect("present").name, "Aviora");

    assert!(db
        .find_by_root("/home/dev/elsewhere")
        .expect("find")
        .is_none());
}

#[test]
fn projects_list_most_recently_opened_first() {
    let db = Db::open_in_memory().expect("open");
    db.insert(&seed("First", "/a"), 100).expect("insert");
    let second = db.insert(&seed("Second", "/b"), 200).expect("insert");
    db.insert(&seed("Third", "/c"), 300).expect("insert");

    db.touch_opened(second.id, 400).expect("open");

    let names: Vec<String> = db
        .list()
        .expect("list")
        .into_iter()
        .map(|p| p.name)
        .collect();
    assert_eq!(names, ["Second", "Third", "First"], "FR-1.6");
}

#[test]
fn opening_a_project_moves_it_to_the_front_without_touching_the_others() {
    let db = Db::open_in_memory().expect("open");
    let first = db.insert(&seed("First", "/a"), 100).expect("insert");
    let second = db.insert(&seed("Second", "/b"), 200).expect("insert");

    db.touch_opened(first.id, 500).expect("open");

    assert_eq!(
        db.get(second.id).expect("get").last_opened_at,
        Some(200),
        "one project's state is never written by another's"
    );
    assert_eq!(db.get(first.id).expect("get").last_opened_at, Some(500));
}

#[test]
fn a_project_that_was_never_added_is_not_found() {
    let db = Db::open_in_memory().expect("open");

    match db.get(ProjectId::new(404)) {
        Err(mira_core::MiraError::NotFound { .. }) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn opening_a_project_that_is_gone_is_not_found() {
    let db = Db::open_in_memory().expect("open");

    match db.touch_opened(ProjectId::new(404), 1) {
        Err(mira_core::MiraError::NotFound { .. }) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn removing_a_project_removes_its_markers_too() {
    let db = Db::open_in_memory().expect("open");
    let saved = db
        .insert(
            &NewProject {
                markers: vec!["rust".to_owned()],
                ..seed("Mira", "/home/dev/mira")
            },
            1,
        )
        .expect("insert");

    db.remove(saved.id).expect("remove");

    assert_eq!(db.count().expect("count"), 0);
    let orphans: u32 = db
        .with_connection(|conn| {
            conn.query_row("SELECT COUNT(*) FROM project_markers", [], |row| row.get(0))
        })
        .expect("count markers");
    assert_eq!(orphans, 0, "ON DELETE CASCADE, enforced by foreign keys");
}

#[test]
fn removing_a_project_that_is_gone_is_not_found() {
    let db = Db::open_in_memory().expect("open");

    match db.remove(ProjectId::new(404)) {
        Err(mira_core::MiraError::NotFound { .. }) => {}
        other => panic!("expected NotFound, got {other:?}"),
    }
}

#[test]
fn several_projects_keep_independent_rows() {
    let db = Db::open_in_memory().expect("open");
    for (index, name) in ["Aviora", "Mobile App", "Client API", "Experiment"]
        .into_iter()
        .enumerate()
    {
        db.insert(
            &NewProject {
                is_git: index % 2 == 0,
                git_root: (index % 2 == 0).then(|| format!("/p/{index}")),
                ..seed(name, &format!("/p/{index}"))
            },
            100 + index as i64,
        )
        .expect("insert");
    }

    let all = db.list().expect("list");
    assert_eq!(all.len(), 4);
    assert_eq!(
        all.iter().filter(|p| p.is_git).count(),
        2,
        "each project carries its own detection result"
    );
    assert_eq!(
        all.iter()
            .map(|p| p.root_path.clone())
            .collect::<Vec<_>>()
            .len(),
        4
    );
}
