//! The machine-wide Ports view: how an observation becomes three groups.
//!
//! The grouping is a pure function, so every case the IA names is assertable
//! without a running application: this project's listeners, other projects',
//! and the ones Mira could not place. The one worth care is the last — a
//! listener Mira failed to attribute must be *shown with its reason*, not
//! dropped, because on Windows that is most of them and an empty view beside a
//! running dev server would be a worse lie than an uncertain row.

use mira_core::ProjectId;
use mira_ports::{Attribution, Listener, PackageBoundary, Unattributed};
use mira_processes::ProcessFacts;

use mira_lib::commands::ports::group;
use mira_lib::live::{Service, ServiceObservation};

const AVIORA: ProjectId = ProjectId::new(1);
const MIRA: ProjectId = ProjectId::new(2);

fn facts(pid: u32) -> ProcessFacts {
    ProcessFacts {
        pid,
        name: "node".to_owned(),
        executable: Some("/usr/local/bin/node".to_owned()),
        parent: Some(1),
        working_directory: Some("/home/dev/aviora".to_owned()),
        cpu_share: Some(12.5),
        memory_bytes: Some(188_743_680),
        uptime_seconds: Some(7_200),
    }
}

fn listener(port: u16, pid: Option<u32>) -> Listener {
    Listener {
        port,
        local_address: "127.0.0.1".to_owned(),
        pid,
    }
}

fn placed(port: u16, project: ProjectId) -> Service {
    Service {
        listener: listener(port, Some(u32::from(port))),
        process: Some(facts(u32::from(port))),
        attribution: Attribution::Project {
            project_id: project,
            package: None,
        },
    }
}

fn unplaced(port: u16, reason: Unattributed) -> Service {
    Service {
        listener: listener(port, None),
        process: None,
        attribution: Attribution::Unattributed { reason },
    }
}

fn observation(services: Vec<Service>) -> ServiceObservation {
    ServiceObservation {
        services,
        error: None,
        observed_at: Some(1_800_000_000),
    }
}

fn names() -> Vec<(ProjectId, String)> {
    vec![(AVIORA, "Aviora".to_owned()), (MIRA, "Mira".to_owned())]
}

// ── Grouping ─────────────────────────────────────────────────────────────────

#[test]
fn nothing_listening_is_an_empty_view_rather_than_an_error() {
    let view = group(&observation(Vec::new()), &names());

    assert!(view.projects.is_empty());
    assert!(view.unattributed.is_empty());
    assert_eq!(view.observed_at, Some(1_800_000_000));
    assert_eq!(view.error, None);
}

#[test]
fn listeners_are_grouped_under_the_project_that_owns_them() {
    let view = group(
        &observation(vec![
            placed(3_000, AVIORA),
            placed(8_080, MIRA),
            placed(5_173, AVIORA),
        ]),
        &names(),
    );

    assert_eq!(view.projects.len(), 2);
    assert_eq!(view.projects[0].project, "Aviora");
    assert_eq!(
        view.projects[0]
            .rows
            .iter()
            .map(|row| row.port)
            .collect::<Vec<_>>(),
        [3_000, 5_173]
    );
    assert_eq!(view.projects[1].project, "Mira");
    assert_eq!(view.projects[1].rows.len(), 1);
    assert!(view.unattributed.is_empty());
}

#[test]
fn groups_come_back_in_project_order_however_the_listeners_arrived() {
    // So the view does not reshuffle between readings.
    let view = group(
        &observation(vec![placed(8_080, MIRA), placed(3_000, AVIORA)]),
        &names(),
    );

    let ids: Vec<i64> = view
        .projects
        .iter()
        .map(|group| group.project_id.get())
        .collect();
    assert_eq!(ids, [1, 2]);
}

#[test]
fn a_listener_mira_could_not_place_is_shown_with_the_others_it_could_not_place() {
    for reason in [
        Unattributed::NoOwningProcess,
        Unattributed::NoWorkingDirectory,
        Unattributed::OutsideEveryProject,
    ] {
        let view = group(
            &observation(vec![placed(3_000, AVIORA), unplaced(9_999, reason)]),
            &names(),
        );

        assert_eq!(view.projects.len(), 1, "{reason:?}");
        assert_eq!(view.unattributed.len(), 1, "{reason:?}");
        assert_eq!(view.unattributed[0].port, 9_999);
    }
}

#[test]
fn a_machine_where_nothing_can_be_attributed_still_shows_everything() {
    // Windows, where no listener has a working directory. An empty view beside a
    // running dev server would be a worse lie than an uncertain row.
    let view = group(
        &observation(vec![
            unplaced(3_000, Unattributed::NoWorkingDirectory),
            unplaced(5_173, Unattributed::NoWorkingDirectory),
        ]),
        &names(),
    );

    assert!(view.projects.is_empty());
    assert_eq!(view.unattributed.len(), 2);
}

#[test]
fn a_listener_attributed_to_a_project_mira_no_longer_knows_falls_to_unattributed() {
    // The project was removed between the reading and the render. Showing it
    // under a name Mira cannot produce would be inventing one.
    let view = group(
        &observation(vec![placed(3_000, ProjectId::new(404))]),
        &names(),
    );

    assert!(view.projects.is_empty());
    assert_eq!(view.unattributed.len(), 1);
    assert_eq!(view.unattributed[0].port, 3_000);
}

#[test]
fn a_listener_in_a_package_is_still_its_projects() {
    // Package boundaries group a *project's* Services section; the machine-wide
    // view groups by project, so a monorepo's packages do not fragment it.
    let mut service = placed(3_000, AVIORA);
    service.attribution = Attribution::Project {
        project_id: AVIORA,
        package: Some(PackageBoundary {
            name: "@aviora/web".to_owned(),
            path: "apps/web".to_owned(),
        }),
    };

    let view = group(&observation(vec![service]), &names());
    assert_eq!(view.projects.len(), 1);
    assert_eq!(view.projects[0].rows.len(), 1);
}

// ── What each row carries ────────────────────────────────────────────────────

#[test]
fn a_row_carries_its_position_so_opening_needs_no_port() {
    // `at` is the index in the observation, which is what `live.open_service`
    // takes. Ports travel outward to be read; positions travel back.
    let view = group(
        &observation(vec![
            placed(3_000, AVIORA),
            unplaced(9_999, Unattributed::NoOwningProcess),
            placed(8_080, MIRA),
        ]),
        &names(),
    );

    assert_eq!(view.projects[0].rows[0].at, 0);
    assert_eq!(view.unattributed[0].at, 1);
    assert_eq!(view.projects[1].rows[0].at, 2);
}

#[test]
fn a_row_carries_cpu_memory_and_uptime_and_no_command_line() {
    let view = group(&observation(vec![placed(3_000, AVIORA)]), &names());
    let process = view.projects[0].rows[0]
        .process
        .as_ref()
        .expect("a process");

    assert_eq!(process.cpu_share, Some(12.5));
    assert_eq!(process.memory_bytes, Some(188_743_680));
    assert_eq!(process.uptime_seconds, Some(7_200));

    let json = serde_json::to_string(&view).expect("serialise");
    for absent in ["cmd", "argv", "commandLine", "environ"] {
        assert!(!json.contains(absent), "{absent} reached the interface");
    }
}

#[test]
fn a_listener_with_no_process_says_so_rather_than_inventing_one() {
    let view = group(
        &observation(vec![unplaced(9_999, Unattributed::NoOwningProcess)]),
        &names(),
    );

    assert!(view.unattributed[0].process.is_none());
}

// ── Honesty about the reading itself ─────────────────────────────────────────

#[test]
fn a_reading_that_never_happened_is_not_an_empty_machine() {
    let view = group(
        &ServiceObservation {
            services: Vec::new(),
            error: None,
            observed_at: None,
        },
        &names(),
    );

    assert_eq!(view.observed_at, None, "the interface must be able to tell");
    assert_eq!(view.error, None);
}

#[test]
fn a_reading_that_failed_carries_the_reason_through() {
    let view = group(
        &ServiceObservation {
            services: Vec::new(),
            error: Some("Reading the list of listening ports was refused.".to_owned()),
            observed_at: Some(1_800_000_000),
        },
        &names(),
    );

    assert_eq!(
        view.error.as_deref(),
        Some("Reading the list of listening ports was refused."),
        "the platform's reason is passed through, not replaced"
    );
}
