//! What an action resolves to, and what it never resolves to.
//!
//! The catalogue is compiled in and the identities are Mira's own, so the
//! interesting cases are not "can somebody send a bad id" — the wire tests cover
//! that — but what happens to an id that is *well formed and means nothing*, and
//! what happens when an action cannot be done.
//!
//! Two rules carry the file. An id with no row resolves to `Unknown` and never
//! to another action. And an action that cannot be done says so with a reason
//! rather than being offered and failing — including the ambiguous case, which
//! is the one somebody would otherwise be tempted to guess at.

use mira_core::action::{find, resolve, ActionId, ActionState, Effect, Support};
use mira_core::{AppKind, CATALOGUE};

fn id(raw: &str) -> ActionId {
    raw.parse().expect("a well-formed id")
}

fn everything(running: &[&str]) -> Support {
    Support {
        openable: vec![
            (AppKind::Editor, "Visual Studio Code".to_owned()),
            (AppKind::Terminal, "Terminal".to_owned()),
        ],
        folder_exists: true,
        reveal: None,
        running: running.iter().map(|s| (*s).to_owned()).collect(),
    }
}

fn state(raw: &str, support: &Support) -> ActionState {
    resolve(&[id(raw)], support)
        .into_iter()
        .next()
        .expect("one action")
        .state
}

// ── The catalogue ────────────────────────────────────────────────────────────

#[test]
fn every_catalogue_row_can_be_found_by_its_own_id() {
    for action in CATALOGUE {
        let found = find(&id(action.id)).expect("a row");
        assert_eq!(found.id, action.id);
        assert_eq!(found.effect, action.effect);
    }
}

#[test]
fn an_id_that_names_no_row_finds_nothing() {
    for invented in ["run", "npm", "deploy", "open-editor-2", "z"] {
        assert!(find(&id(invented)).is_none(), "{invented} found a row");
    }
}

#[test]
fn the_catalogue_contains_nothing_that_runs_anything() {
    // The effect set, asserted from the data rather than from the source text
    // the guard scans — two independent statements of the same rule.
    for action in CATALOGUE {
        match action.effect {
            Effect::OpenIn { .. }
            | Effect::RevealProject
            | Effect::OpenService
            | Effect::MarkOpened
            | Effect::Observe => {}
        }
    }
    assert_eq!(CATALOGUE.len(), 6);
}

// ── Resolving ────────────────────────────────────────────────────────────────

#[test]
fn an_action_this_machine_can_do_is_ready_and_names_what_it_will_reach() {
    assert_eq!(
        state("open-editor", &everything(&[])),
        ActionState::Ready {
            detail: Some("Visual Studio Code".to_owned())
        },
        "the button says which editor before it is pressed"
    );
}

#[test]
fn an_action_with_no_application_behind_it_is_a_sentence_rather_than_a_button() {
    let no_editor = Support {
        openable: vec![(AppKind::Terminal, "Terminal".to_owned())],
        ..everything(&[])
    };

    let state = state("open-editor", &no_editor);
    assert!(matches!(state, ActionState::Unavailable { .. }));
    assert!(!state.is_ready());
}

#[test]
fn every_action_that_reaches_the_folder_refuses_when_the_folder_is_gone() {
    let moved = Support {
        folder_exists: false,
        ..everything(&[])
    };

    for reaching in ["open-editor", "open-terminal", "reveal-project"] {
        assert!(
            matches!(state(reaching, &moved), ActionState::Unavailable { .. }),
            "{reaching} must refuse when the folder is not there"
        );
    }
}

#[test]
fn a_file_manager_this_platform_will_not_open_is_said_in_its_own_words() {
    let refused = Support {
        reveal: Some("This desktop has no file manager Mira can reach.".to_owned()),
        ..everything(&[])
    };

    assert_eq!(
        state("reveal-project", &refused),
        ActionState::Unavailable {
            reason: "This desktop has no file manager Mira can reach.".to_owned()
        },
        "the platform's reason is passed through, not replaced"
    );
}

#[test]
fn the_two_actions_that_never_leave_mira_are_always_ready() {
    // Marking a workspace opened and re-reading observations do not touch the
    // desktop, so there is no machine state that could make them unavailable.
    let bare = Support::default();

    assert!(state("mark-opened", &bare).is_ready());
    assert!(state("refresh", &bare).is_ready());
}

// ── The ambiguous case ───────────────────────────────────────────────────────

#[test]
fn one_running_service_makes_the_service_action_ready_and_names_it() {
    assert_eq!(
        state("open-service", &everything(&[":5173"])),
        ActionState::Ready {
            detail: Some(":5173".to_owned())
        }
    );
}

#[test]
fn no_running_service_makes_it_unavailable_rather_than_a_failing_button() {
    let state = state("open-service", &everything(&[]));
    assert!(matches!(state, ActionState::Unavailable { .. }));
}

#[test]
fn more_than_one_running_service_is_refused_rather_than_guessed_at() {
    // "Make ambiguous actions impossible rather than merely disabled." Two
    // services running is a question this action cannot answer, and opening the
    // first would be Mira choosing on somebody's behalf.
    let ActionState::Unavailable { reason } =
        state("open-service", &everything(&[":5173", ":8080"]))
    else {
        panic!("two running services must not resolve to a ready action");
    };

    assert!(reason.contains('2'), "the reason says how many: {reason}");
    assert!(
        reason.to_lowercase().contains("services list"),
        "and says what to do instead: {reason}"
    );
}

// ── The unknown case ─────────────────────────────────────────────────────────

#[test]
fn an_id_mira_has_no_row_for_resolves_to_unknown_and_never_to_another_action() {
    // A workspace given an action a later Mira removed. It is told, not
    // silently matched to something else and not silently dropped.
    let resolved = resolve(&[id("some-action-mira-removed")], &everything(&[]));

    assert_eq!(resolved.len(), 1, "it stays on the list");
    assert_eq!(resolved[0].state, ActionState::Unknown);
    assert_eq!(resolved[0].id.as_str(), "some-action-mira-removed");
    assert!(resolved[0].effect.is_none(), "it has no effect at all");
    assert!(resolved[0].label.is_empty(), "and nothing to call it");
}

#[test]
fn an_unknown_id_beside_known_ones_leaves_the_others_alone() {
    let support = everything(&[":5173"]);
    let resolved = resolve(&[id("open-editor"), id("gone"), id("refresh")], &support);

    assert!(resolved[0].state.is_ready());
    assert_eq!(resolved[1].state, ActionState::Unknown);
    assert!(resolved[2].state.is_ready());
}

#[test]
fn resolving_preserves_the_order_it_was_given() {
    let support = everything(&[]);
    let given = [id("refresh"), id("mark-opened"), id("reveal-project")];
    let back: Vec<String> = resolve(&given, &support)
        .into_iter()
        .map(|action| action.id.as_str().to_owned())
        .collect();

    assert_eq!(back, ["refresh", "mark-opened", "reveal-project"]);
}

#[test]
fn a_workspace_with_no_actions_resolves_to_none() {
    assert!(resolve(&[], &everything(&[":5173"])).is_empty());
}

// ── The identity itself ──────────────────────────────────────────────────────

#[test]
fn an_action_id_is_lowercase_letters_digits_and_hyphens() {
    for accepted in ["a", "open-editor", "x9", &"a".repeat(32)] {
        assert!(
            accepted.parse::<ActionId>().is_ok(),
            "{accepted} was refused"
        );
    }
    for refused in [
        "",
        &"a".repeat(33),
        "Open-Editor",
        "open editor",
        "open/editor",
        "open.editor",
        "open_editor",
        "npm run dev",
        "../etc/passwd",
        "open;rm",
    ] {
        assert!(
            refused.parse::<ActionId>().is_err(),
            "{refused} was accepted"
        );
    }
}
