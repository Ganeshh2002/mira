//! Which project a listening process belongs to.
//!
//! The rule this file exists to hold: **never guess.** A service Mira cannot
//! place is shown as unattributed with the reason, which is useful. A service
//! placed in the wrong project is worse than useless — it is a confident lie
//! about where your work is running.
//!
//! Attribution is a pure function over paths, so every case below is exact and
//! needs no process, no socket, and no repository.

use std::path::Path;

use mira_core::ProjectId;
use mira_fs::PathMatching;
use mira_ports::{attribute, Attribution, PackageBoundary, ProjectRoot, Unattributed};

fn aviora() -> ProjectRoot {
    ProjectRoot {
        id: ProjectId::new(1),
        root: "/home/dev/aviora".to_owned(),
        packages: vec![
            PackageBoundary {
                name: "@aviora/web".to_owned(),
                path: "apps/web".to_owned(),
            },
            PackageBoundary {
                name: "@aviora/api".to_owned(),
                path: "apps/api".to_owned(),
            },
        ],
    }
}

fn solo() -> ProjectRoot {
    ProjectRoot {
        id: ProjectId::new(2),
        root: "/home/dev/solo".to_owned(),
        packages: Vec::new(),
    }
}

fn place(cwd: &str, roots: &[ProjectRoot]) -> Attribution {
    attribute(Some(Path::new(cwd)), roots, PathMatching::CaseSensitive)
}

// ── Placing a service ────────────────────────────────────────────────────────

#[test]
fn a_process_running_in_a_project_belongs_to_it() {
    let placed = place("/home/dev/solo", &[aviora(), solo()]);

    assert_eq!(
        placed,
        Attribution::Project {
            project_id: ProjectId::new(2),
            package: None,
        }
    );
}

#[test]
fn a_process_running_below_a_project_still_belongs_to_it() {
    let placed = place("/home/dev/solo/src/server", &[solo()]);

    assert_eq!(
        placed,
        Attribution::Project {
            project_id: ProjectId::new(2),
            package: None,
        }
    );
}

#[test]
fn a_process_running_in_a_package_is_placed_in_that_package() {
    let placed = place("/home/dev/aviora/apps/web", &[aviora()]);

    assert_eq!(
        placed,
        Attribution::Project {
            project_id: ProjectId::new(1),
            package: Some(PackageBoundary {
                name: "@aviora/web".to_owned(),
                path: "apps/web".to_owned(),
            }),
        }
    );
}

#[test]
fn two_packages_of_one_monorepo_are_told_apart() {
    let roots = [aviora()];

    let web = place("/home/dev/aviora/apps/web", &roots);
    let api = place("/home/dev/aviora/apps/api/src", &roots);

    assert_ne!(
        web, api,
        "each service is placed in the package it runs from"
    );
    assert!(matches!(
        api,
        Attribution::Project {
            package: Some(ref boundary),
            ..
        } if boundary.path == "apps/api"
    ));
}

#[test]
fn a_process_at_the_monorepo_root_is_the_project_and_no_package() {
    let placed = place("/home/dev/aviora", &[aviora()]);

    assert_eq!(
        placed,
        Attribution::Project {
            project_id: ProjectId::new(1),
            package: None,
        },
        "a repository-level task belongs to the repository, not to a package"
    );
}

#[test]
fn the_innermost_project_wins_when_one_contains_another() {
    // A package added as its own project sits inside the monorepo that is also a
    // project. The service is running in the narrower one, and that is the answer.
    let outer = aviora();
    let inner = ProjectRoot {
        id: ProjectId::new(3),
        root: "/home/dev/aviora/apps/web".to_owned(),
        packages: Vec::new(),
    };

    let placed = place("/home/dev/aviora/apps/web/src", &[outer, inner]);

    assert_eq!(
        placed,
        Attribution::Project {
            project_id: ProjectId::new(3),
            package: None,
        }
    );
}

#[test]
fn the_deepest_package_wins_when_one_contains_another() {
    let nested = ProjectRoot {
        id: ProjectId::new(1),
        root: "/home/dev/aviora".to_owned(),
        packages: vec![
            PackageBoundary {
                name: "group".to_owned(),
                path: "packages".to_owned(),
            },
            PackageBoundary {
                name: "ui".to_owned(),
                path: "packages/ui".to_owned(),
            },
        ],
    };

    let placed = place("/home/dev/aviora/packages/ui/src", &[nested]);

    assert!(matches!(
        placed,
        Attribution::Project { package: Some(ref boundary), .. } if boundary.path == "packages/ui"
    ));
}

// ── Refusing to place one ────────────────────────────────────────────────────

#[test]
fn a_process_outside_every_project_is_unattributed() {
    let placed = place("/usr/local/bin", &[aviora(), solo()]);

    assert_eq!(
        placed,
        Attribution::Unattributed {
            reason: Unattributed::OutsideEveryProject
        }
    );
}

#[test]
fn a_process_whose_working_directory_is_unknown_is_unattributed() {
    // Windows does not expose another process's working directory. That is a
    // platform limit, and the honest answer is to say the service is there and
    // that Mira cannot place it (`platform-abstraction.md` §5).
    let placed = attribute(None, &[aviora()], PathMatching::CaseSensitive);

    assert_eq!(
        placed,
        Attribution::Unattributed {
            reason: Unattributed::NoWorkingDirectory
        }
    );
}

#[test]
fn nothing_is_attributed_when_no_projects_have_been_added() {
    let placed = place("/home/dev/aviora/apps/web", &[]);

    assert_eq!(
        placed,
        Attribution::Unattributed {
            reason: Unattributed::OutsideEveryProject
        }
    );
}

#[test]
fn a_sibling_directory_with_a_shared_prefix_is_not_inside_the_project() {
    // `/home/dev/aviora-scratch` starts with `/home/dev/aviora` as text. Placing
    // a service there into `aviora` would be exactly the confident lie this
    // module exists to prevent.
    let placed = place("/home/dev/aviora-scratch/apps/web", &[aviora()]);

    assert_eq!(
        placed,
        Attribution::Unattributed {
            reason: Unattributed::OutsideEveryProject
        }
    );
}

#[test]
fn a_package_directory_with_a_shared_prefix_is_not_that_package() {
    let placed = place("/home/dev/aviora/apps/web-legacy", &[aviora()]);

    assert_eq!(
        placed,
        Attribution::Project {
            project_id: ProjectId::new(1),
            package: None,
        },
        "inside the project, but not inside any package it declares"
    );
}

#[test]
fn case_is_compared_the_way_the_filesystem_does() {
    let roots = [aviora()];
    let cwd = Path::new("/home/dev/AVIORA/apps/web");

    assert!(matches!(
        attribute(Some(cwd), &roots, PathMatching::CaseInsensitive),
        Attribution::Project { .. }
    ));
    assert!(matches!(
        attribute(Some(cwd), &roots, PathMatching::CaseSensitive),
        Attribution::Unattributed { .. }
    ));
}

#[test]
fn every_refusal_carries_a_reason_a_person_can_read() {
    for reason in [
        Unattributed::NoOwningProcess,
        Unattributed::NoWorkingDirectory,
        Unattributed::OutsideEveryProject,
    ] {
        let sentence = reason.explain();
        assert!(!sentence.is_empty());
        assert!(
            sentence.ends_with('.'),
            "shown as a sentence, not a code: {sentence}"
        );
    }
}
