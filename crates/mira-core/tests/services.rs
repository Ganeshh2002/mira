//! What a watched service resolves to, and what it never resolves to.
//!
//! The stored row is one port. Everything a person reads on the screen comes
//! from matching that port against the reading the observers already took, and
//! this file is the whole state machine of that match.
//!
//! The state worth being careful about is [`ServiceState::Taken`]. A workspace
//! watches port 5173; the dev server stops; something unrelated binds 5173.
//! Reporting that as *running* would be Mira substituting one thing for another
//! and inviting somebody to open it — the same failure that
//! `security-and-privacy.md` §6 forbids for applications.

use mira_core::service::{Listening, Observed, Port, ServiceState, WatchedService};
use mira_core::{resolve, ProjectId, WorkspaceId, WorkspaceServiceId};

const AVIORA: ProjectId = ProjectId::new(1);
const OTHER: ProjectId = ProjectId::new(2);
const WEB: WorkspaceId = WorkspaceId::new(1);

fn port(raw: u16) -> Port {
    Port::try_from(raw).expect("a port")
}

fn watching(raw: u16) -> WatchedService {
    WatchedService {
        id: WorkspaceServiceId::new(i64::from(raw)),
        workspace_id: WEB,
        port: port(raw),
        added_at: 1_800_000_000,
    }
}

fn listening(raw: u16, project: Option<ProjectId>) -> Listening {
    Listening {
        port: port(raw),
        project_id: project,
        address: "127.0.0.1".to_owned(),
        process: Some("node".to_owned()),
        pid: Some(4_242),
    }
}

#[test]
fn a_service_this_project_is_serving_is_running() {
    let seen = [listening(5_173, Some(AVIORA))];
    let resolved = resolve(&[watching(5_173)], AVIORA, Observed::Seen(&seen));

    assert_eq!(
        resolved[0].state,
        ServiceState::Running {
            address: "127.0.0.1".to_owned(),
            process: Some("node".to_owned()),
            pid: Some(4_242),
        }
    );
    assert!(resolved[0].state.is_running());
}

#[test]
fn a_service_nobody_is_serving_is_not_running() {
    // Expected, and not there. Not an error: a service you have not started yet
    // is the normal condition of a morning.
    let seen = [listening(8_080, Some(AVIORA))];
    let resolved = resolve(&[watching(5_173)], AVIORA, Observed::Seen(&seen));

    assert_eq!(resolved[0].state, ServiceState::NotRunning);
    assert!(!resolved[0].state.is_running());
}

#[test]
fn expected_and_running_are_never_the_same_state() {
    // Asserted as a pair rather than separately, because the requirement is that
    // they are *distinguishable* — a UI cannot show two things one value.
    let up = resolve(
        &[watching(5_173)],
        AVIORA,
        Observed::Seen(&[listening(5_173, Some(AVIORA))]),
    );
    let down = resolve(&[watching(5_173)], AVIORA, Observed::Seen(&[]));

    assert_ne!(up[0].state, down[0].state);
    assert_eq!(up[0].watched, down[0].watched, "the same stored row");
}

#[test]
fn a_port_something_else_took_is_never_reported_as_running() {
    // The substitution guard, in the domain. The dev server stopped and an
    // unrelated process bound the number; the workspace's service is not up, and
    // saying it is would invite somebody to open a stranger's process.
    let seen = [Listening {
        process: Some("postgres".to_owned()),
        ..listening(5_173, Some(OTHER))
    }];
    let resolved = resolve(&[watching(5_173)], AVIORA, Observed::Seen(&seen));

    assert_eq!(
        resolved[0].state,
        ServiceState::Taken {
            process: Some("postgres".to_owned()),
        }
    );
    assert!(!resolved[0].state.is_running());
}

#[test]
fn a_port_mira_cannot_place_is_also_not_claimed() {
    // Windows cannot read another process's working directory, so attribution
    // there is Degraded and many listeners have no project. "Something is here
    // and Mira cannot tell whose" is `Taken`, not `Running`.
    let seen = [listening(5_173, None)];
    let resolved = resolve(&[watching(5_173)], AVIORA, Observed::Seen(&seen));

    assert!(matches!(resolved[0].state, ServiceState::Taken { .. }));
}

#[test]
fn before_the_first_reading_mira_says_it_has_not_looked() {
    let resolved = resolve(&[watching(5_173)], AVIORA, Observed::NotYet);
    assert_eq!(resolved[0].state, ServiceState::NeverObserved);
}

#[test]
fn a_reading_that_failed_says_so_rather_than_saying_nothing_is_running() {
    // The `information-architecture.md` §5 rule, and the one this whole enum
    // exists for: "Mira could not look" must never render as "your server is
    // down".
    let resolved = resolve(
        &[watching(5_173)],
        AVIORA,
        Observed::Failed("Reading the list of listening ports was refused."),
    );

    assert_eq!(
        resolved[0].state,
        ServiceState::Unreadable {
            reason: "Reading the list of listening ports was refused.".to_owned(),
        }
    );
    assert_ne!(resolved[0].state, ServiceState::NotRunning);
}

#[test]
fn every_watched_service_comes_back_in_the_order_it_was_given() {
    let watched = [watching(3_000), watching(5_173), watching(8_080)];
    let seen = [listening(5_173, Some(AVIORA))];
    let resolved = resolve(&watched, AVIORA, Observed::Seen(&seen));

    let ports: Vec<u16> = resolved
        .iter()
        .map(|service| service.watched.port.get())
        .collect();
    assert_eq!(ports, [3_000, 5_173, 8_080]);
    assert_eq!(
        resolved
            .iter()
            .filter(|service| service.state.is_running())
            .count(),
        1
    );
}

#[test]
fn watching_nothing_resolves_to_nothing() {
    let seen = [listening(5_173, Some(AVIORA))];
    assert!(resolve(&[], AVIORA, Observed::Seen(&seen)).is_empty());
}

#[test]
fn a_workspace_on_another_project_does_not_see_this_ones_service() {
    // One project's observation, two projects' workspaces. Attribution is what
    // keeps them apart, and it is checked per service rather than per list.
    let seen = [listening(5_173, Some(AVIORA))];

    let ours = resolve(&[watching(5_173)], AVIORA, Observed::Seen(&seen));
    let theirs = resolve(&[watching(5_173)], OTHER, Observed::Seen(&seen));

    assert!(ours[0].state.is_running());
    assert!(matches!(theirs[0].state, ServiceState::Taken { .. }));
}

#[test]
fn a_port_is_a_number_from_one_to_sixty_five_thousand() {
    assert!(Port::try_from(0_u16).is_err(), "port 0 means \"any\"");
    assert_eq!(Port::try_from(1_u16).expect("a port").get(), 1);
    assert_eq!(Port::try_from(65_535_u16).expect("a port").get(), 65_535);
}

#[test]
fn a_port_from_a_database_column_is_checked_the_same_way() {
    for refused in [0_i64, -1, 65_536, i64::MIN, i64::MAX] {
        assert!(Port::try_from(refused).is_err(), "{refused} was accepted");
    }
    assert_eq!(Port::try_from(5_173_i64).expect("a port").get(), 5_173);
}

#[test]
fn a_port_is_written_the_way_a_person_says_it() {
    assert_eq!(port(5_173).to_string(), "5173");
}
