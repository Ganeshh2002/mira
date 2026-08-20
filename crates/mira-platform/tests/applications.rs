//! Finding out whether an editor, a terminal, and a browser exist here.
//!
//! The rule this file holds is the one the slice brief states twice: **do not
//! assume the machine.** `macOS = VS Code` is wrong for anyone using Zed, and
//! `Linux = gnome-terminal` is wrong for most of Linux. Each platform gets a list
//! of candidates, the first one actually present wins, and none present is a real
//! answer that the interface shows rather than hides.
//!
//! Resolution takes the "does this exist" test as an argument, so every case
//! below is exact and none of it depends on what happens to be installed on the
//! machine running the tests.

use mira_core::AppKind;
use mira_platform::Os;
use mira_platform::{candidates, first_present, AppPresence, Applications, Probe};

const EVERY_OS: [Os; 3] = [Os::MacOs, Os::Windows, Os::Linux];

// ── The candidate lists ──────────────────────────────────────────────────────

#[test]
fn every_platform_knows_where_to_look_for_all_three_kinds() {
    for os in EVERY_OS {
        for kind in AppKind::ALL {
            assert!(
                !candidates(os, kind).is_empty(),
                "{os:?} has nowhere to look for a {kind:?}"
            );
        }
    }
}

#[test]
fn no_platform_is_assumed_to_have_exactly_one_editor() {
    // The brief's example of the mistake. An editor list of one is an assumption
    // about what people use, not a search.
    for os in EVERY_OS {
        assert!(
            candidates(os, AppKind::Editor).len() > 1,
            "{os:?} looks for only one editor"
        );
    }
}

#[test]
fn every_candidate_is_named_for_a_person() {
    for os in EVERY_OS {
        for kind in AppKind::ALL {
            for candidate in candidates(os, kind) {
                assert!(!candidate.name.is_empty(), "{os:?}/{kind:?}");
            }
        }
    }
}

#[test]
fn a_program_candidate_is_a_program_name_and_not_a_command_line() {
    // `security-and-privacy.md` §5: a program is resolved, never handed to an
    // interpreter. A candidate carrying a space or a shell character would be a
    // command line pretending to be a program name.
    for os in EVERY_OS {
        for kind in AppKind::ALL {
            for candidate in candidates(os, kind) {
                let Probe::Program(program) = candidate.probe else {
                    continue;
                };
                assert!(
                    !program.contains(' ')
                        && !program.contains(';')
                        && !program.contains('|')
                        && !program.contains('/')
                        && !program.contains('\\'),
                    "{os:?}/{kind:?}: {program:?} is not a bare program name"
                );
            }
        }
    }
}

#[test]
fn a_bundle_candidate_is_an_absolute_path() {
    for kind in AppKind::ALL {
        for candidate in candidates(Os::MacOs, kind) {
            if let Probe::Bundle(path) = candidate.probe {
                assert!(path.starts_with('/'), "{path:?} is not absolute");
                assert!(path.ends_with(".app"), "{path:?} is not an application");
            }
        }
    }
}

// ── Resolving ────────────────────────────────────────────────────────────────

#[test]
fn the_first_candidate_that_is_present_wins() {
    let list = candidates(Os::MacOs, AppKind::Editor);
    let second = list[1].name;

    // Nothing is installed except the second candidate.
    let found = first_present(list, |candidate| candidate.name == second);

    assert_eq!(
        found,
        AppPresence::Available {
            name: second.to_owned()
        }
    );
}

#[test]
fn order_decides_when_several_are_present() {
    let list = candidates(Os::MacOs, AppKind::Editor);
    let first = list[0].name;

    let found = first_present(list, |_| true);

    assert_eq!(
        found,
        AppPresence::Available {
            name: first.to_owned()
        },
        "the list is a preference order, not a set"
    );
}

#[test]
fn nothing_installed_is_an_answer_rather_than_a_failure() {
    // `Not installed` is shown to the person. It is not an error, and it does not
    // make the workspace's association with that kind go away.
    for os in EVERY_OS {
        for kind in AppKind::ALL {
            assert_eq!(
                first_present(candidates(os, kind), |_| false),
                AppPresence::NotInstalled,
                "{os:?}/{kind:?}"
            );
        }
    }
}

#[test]
fn resolution_asks_about_candidates_and_nothing_else() {
    // The probe is a parameter, which is what keeps discovery from being able to
    // run anything: there is no code path here that could spawn a process, and a
    // guard test keeps `Command::new` out of this module entirely.
    let mut asked = Vec::new();
    let list = candidates(Os::Linux, AppKind::Terminal);

    let _ = first_present(list, |candidate| {
        asked.push(candidate.name);
        false
    });

    assert_eq!(asked.len(), list.len());
    assert!(asked
        .iter()
        .all(|name| list.iter().any(|c| c.name == *name)));
}

#[test]
fn resolution_stops_at_the_first_match() {
    let mut asked = 0;
    let list = candidates(Os::Linux, AppKind::Browser);

    let _ = first_present(list, |_| {
        asked += 1;
        true
    });

    assert_eq!(asked, 1, "no reason to keep looking once one is found");
}

// ── The real machine ─────────────────────────────────────────────────────────

#[test]
fn a_survey_answers_for_every_kind_exactly_once() {
    let survey = Applications::detect().survey();

    assert_eq!(survey.len(), AppKind::ALL.len());
    for kind in AppKind::ALL {
        assert_eq!(
            survey.iter().filter(|report| report.kind == kind).count(),
            1,
            "{kind:?} is answered once"
        );
    }
}

#[test]
fn anything_the_survey_calls_available_is_named() {
    // An "Available" with no name would render as a blank row, which is worse
    // than saying it is not installed.
    for report in Applications::detect().survey() {
        if let AppPresence::Available { name } = &report.presence {
            assert!(
                !name.is_empty(),
                "{:?} is available but unnamed",
                report.kind
            );
        }
    }
}
