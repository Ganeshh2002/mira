//! Which of a project's services a workspace watches.
//!
//! The repository stores ports; this decides what a person is allowed to *ask
//! for*. The rule the whole layer exists to hold is that a port is never a
//! parameter: a service is added by naming a **position** in the list Mira
//! offered, and that list is built from Mira's own observation of this
//! workspace's project.
//!
//! Two failures are worth having tests of their own. Naming a position that is
//! not there — the list moved between the render and the click — must be a
//! refusal rather than a wrong service. And an offer belonging to another
//! project must be refused even when it arrives at a valid position, because the
//! day somebody passes the machine's whole list is the day that would otherwise
//! attach a stranger's process to a workspace.

use mira_core::service::{Listening, Observed, Port, ServiceState};
use mira_core::{MiraError, ProjectId, WorkspaceId, WorkspaceServiceId};
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

fn listening(port: u16, project: Option<ProjectId>) -> Listening {
    Listening {
        port: Port::try_from(port).expect("a port"),
        project_id: project,
        address: "127.0.0.1".to_owned(),
        process: Some("node".to_owned()),
        pid: Some(4_242),
        cpu_share: Some(2.5),
        memory_bytes: Some(188_743_680),
        uptime_seconds: Some(3_600),
    }
}

// ── Watching ─────────────────────────────────────────────────────────────────

#[test]
fn a_workspace_watches_nothing_until_somebody_says_otherwise() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");

    assert!(service(&db)
        .services(web, &[])
        .expect("services")
        .is_empty());
}

#[test]
fn a_service_is_added_by_its_position_in_the_offered_list() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");

    let offers = [
        listening(3_000, Some(aviora)),
        listening(5_173, Some(aviora)),
        listening(8_080, Some(aviora)),
    ];

    let added = service(&db)
        .watch(web, 1, &offers, 1_800_000_100)
        .expect("watch");

    assert_eq!(added.port.get(), 5_173, "position 1, not port 1");
    assert_eq!(added.workspace_id, web);
}

#[test]
fn a_position_past_the_list_is_a_stale_selection_rather_than_a_wrong_service() {
    // The list moved between the render and the click. Refusing is the only
    // honest answer; taking the last one instead would silently watch something
    // nobody picked.
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");
    let offers = [listening(5_173, Some(aviora))];

    for at in [1_u32, 2, 99, u32::MAX] {
        assert!(
            matches!(
                service(&db).watch(web, at, &offers, 1_800_000_100),
                Err(MiraError::NotFound { .. })
            ),
            "position {at} must be refused"
        );
    }
    assert!(service(&db)
        .services(web, &[])
        .expect("services")
        .is_empty());
}

#[test]
fn an_empty_offer_list_admits_nothing() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");

    assert!(matches!(
        service(&db).watch(web, 0, &[], 1_800_000_100),
        Err(MiraError::NotFound { .. })
    ));
}

#[test]
fn an_offer_belonging_to_another_project_is_refused() {
    // A valid position in a list that is not this workspace's. The command layer
    // filters by project before it ever gets here; this is the check that makes
    // that filtering a belt rather than the only thing holding it up.
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let mira = project(&db, "Mira", "/home/dev/mira");
    let web = workspace(&db, aviora, "Web");

    let offers = [listening(5_173, Some(mira))];

    assert!(matches!(
        service(&db).watch(web, 0, &offers, 1_800_000_100),
        Err(MiraError::Invalid { .. })
    ));
}

#[test]
fn an_unattributed_offer_is_refused() {
    // A socket Mira could not place is not this project's to claim.
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");

    assert!(matches!(
        service(&db).watch(web, 0, &[listening(5_173, None)], 1_800_000_100),
        Err(MiraError::Invalid { .. })
    ));
}

#[test]
fn watching_a_service_a_workspace_already_watches_is_refused_by_name() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");
    let offers = [listening(5_173, Some(aviora))];

    service(&db)
        .watch(web, 0, &offers, 1_800_000_100)
        .expect("watch");

    let again = service(&db).watch(web, 0, &offers, 1_800_000_200);
    assert!(
        matches!(again, Err(MiraError::Invalid { ref detail, .. }) if detail.contains("5173")),
        "got {again:?}"
    );
}

#[test]
fn a_workspace_that_is_gone_says_so_before_anything_is_written() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let offers = [listening(5_173, Some(aviora))];

    assert!(matches!(
        service(&db).watch(WorkspaceId::new(404), 0, &offers, 1_800_000_100),
        Err(MiraError::NotFound { .. })
    ));
}

// ── Reading ──────────────────────────────────────────────────────────────────

#[test]
fn a_watched_service_that_is_up_reads_as_running() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");
    let offers = [listening(5_173, Some(aviora))];
    service(&db)
        .watch(web, 0, &offers, 1_800_000_100)
        .expect("watch");

    let resolved = service(&db).services(web, &offers).expect("services");
    assert!(resolved[0].state.is_running());
}

#[test]
fn a_watched_service_that_stopped_reads_as_not_running() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");
    service(&db)
        .watch(web, 0, &[listening(5_173, Some(aviora))], 1_800_000_100)
        .expect("watch");

    let resolved = service(&db).services(web, &[]).expect("services");
    assert_eq!(resolved[0].state, ServiceState::NotRunning);
    assert_eq!(
        resolved[0].watched.port.get(),
        5_173,
        "and it is still the service that was chosen"
    );
}

#[test]
fn a_reading_that_never_happened_is_not_a_service_that_is_down() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");
    service(&db)
        .watch(web, 0, &[listening(5_173, Some(aviora))], 1_800_000_100)
        .expect("watch");

    let never = service(&db)
        .services_unobserved(web, Observed::NotYet)
        .expect("services");
    let failed = service(&db)
        .services_unobserved(web, Observed::Failed("Refused by the platform."))
        .expect("services");

    assert_eq!(never[0].state, ServiceState::NeverObserved);
    assert!(matches!(failed[0].state, ServiceState::Unreadable { .. }));
    assert_ne!(never[0].state, ServiceState::NotRunning);
    assert_ne!(failed[0].state, ServiceState::NotRunning);
}

// ── Isolation ────────────────────────────────────────────────────────────────

#[test]
fn two_workspaces_on_one_project_keep_separate_lists() {
    // The feature, stated as a test. One project, one set of observations, two
    // different answers to "which of these matter".
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");
    let api = workspace(&db, aviora, "API");

    let offers = [
        listening(5_173, Some(aviora)),
        listening(8_080, Some(aviora)),
    ];
    service(&db)
        .watch(web, 0, &offers, 1_800_000_100)
        .expect("watch");
    service(&db)
        .watch(api, 1, &offers, 1_800_000_100)
        .expect("watch");

    let ports = |id: WorkspaceId| -> Vec<u16> {
        service(&db)
            .services(id, &offers)
            .expect("services")
            .iter()
            .map(|s| s.watched.port.get())
            .collect()
    };

    assert_eq!(ports(web), [5_173]);
    assert_eq!(ports(api), [8_080]);
}

#[test]
fn one_workspace_cannot_forget_anothers_service() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");
    let api = workspace(&db, aviora, "API");
    let offers = [listening(8_080, Some(aviora))];

    let theirs = service(&db)
        .watch(api, 0, &offers, 1_800_000_100)
        .expect("watch");

    assert!(matches!(
        service(&db).forget(web, theirs.id),
        Err(MiraError::NotFound { .. })
    ));
    assert_eq!(
        service(&db).services(api, &offers).expect("services").len(),
        1,
        "and the sibling still has it"
    );
}

#[test]
fn one_workspace_cannot_open_anothers_service() {
    // `port_of` is the one place a row id becomes a number, and it is keyed by
    // both — so a sibling's id never resolves to a port to open.
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");
    let api = workspace(&db, aviora, "API");

    let theirs = service(&db)
        .watch(api, 0, &[listening(8_080, Some(aviora))], 1_800_000_100)
        .expect("watch");

    assert!(matches!(
        service(&db).port_of(web, theirs.id),
        Err(MiraError::NotFound { .. })
    ));
    assert_eq!(
        service(&db).port_of(api, theirs.id).expect("port").get(),
        8_080
    );
}

#[test]
fn a_workspace_on_another_project_sees_a_taken_port_rather_than_a_running_one() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let mira = project(&db, "Mira", "/home/dev/mira");
    let ours = workspace(&db, aviora, "Web");
    let theirs = workspace(&db, mira, "Web");

    service(&db)
        .watch(ours, 0, &[listening(5_173, Some(aviora))], 1_800_000_100)
        .expect("watch");
    service(&db)
        .watch(theirs, 0, &[listening(5_173, Some(mira))], 1_800_000_100)
        .expect("watch");

    // Now only Aviora's is actually up.
    let seen = [listening(5_173, Some(aviora))];
    assert!(service(&db).services(ours, &seen).expect("services")[0]
        .state
        .is_running());
    assert!(matches!(
        service(&db).services(theirs, &seen).expect("services")[0].state,
        ServiceState::Taken { .. }
    ));
}

// ── Forgetting ───────────────────────────────────────────────────────────────

#[test]
fn forgetting_a_service_leaves_the_others_alone() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");
    let offers = [
        listening(5_173, Some(aviora)),
        listening(8_080, Some(aviora)),
    ];

    let vite = service(&db)
        .watch(web, 0, &offers, 1_800_000_100)
        .expect("watch");
    service(&db)
        .watch(web, 1, &offers, 1_800_000_100)
        .expect("watch");

    service(&db).forget(web, vite.id).expect("forget");

    let ports: Vec<u16> = service(&db)
        .services(web, &offers)
        .expect("services")
        .iter()
        .map(|s| s.watched.port.get())
        .collect();
    assert_eq!(ports, [8_080]);
}

#[test]
fn forgetting_something_that_is_not_watched_says_so() {
    let db = Db::open_in_memory().expect("db");
    let aviora = project(&db, "Aviora", "/home/dev/aviora");
    let web = workspace(&db, aviora, "Web");

    assert!(matches!(
        service(&db).forget(web, WorkspaceServiceId::new(404)),
        Err(MiraError::NotFound { .. })
    ));
}
