//! Choosing which application a workspace uses.
//!
//! Two properties run through every test here.
//!
//! **A choice is an identity, not a program.** It names a row in a table
//! compiled into the binary, and an id that names no row resolves to nothing.
//! There is no string here that could become something to run.
//!
//! **A choice is obeyed or refused, never substituted.** An editor that has been
//! uninstalled since it was chosen produces a sentence naming it — not a
//! different editor opening. Quietly starting something else is the
//! unrelated-application substitution `security-and-privacy.md` §6 forbids, and a
//! stated choice makes it worse rather than better (ADR-0019).
//!
//! Everything is pure: `chosen` and `plan` take the existence test as a
//! parameter, so every platform's answer is assertable from any one platform and
//! nothing is ever started.

use std::collections::BTreeSet;

use mira_core::{AppId, AppKind, MiraError};
use mira_platform::{
    candidates, chosen, find, plan, Candidate, ChosenApp, Launch, LaunchMethod, LaunchTarget, Os,
};

const EVERY_OS: [Os; 3] = [Os::MacOs, Os::Windows, Os::Linux];

fn id(raw: &str) -> AppId {
    raw.parse().expect("an application id")
}

fn root() -> LaunchTarget {
    LaunchTarget::Directory(std::path::PathBuf::from("/home/dev/aviora"))
}

/// An existence check that says yes to exactly these catalogue ids.
fn only(ids: &'static [&'static str]) -> impl Fn(&Candidate) -> bool {
    move |candidate| ids.contains(&candidate.id)
}

fn nothing(_: &Candidate) -> bool {
    false
}

fn everything(_: &Candidate) -> bool {
    true
}

// ── The catalogue ────────────────────────────────────────────────────────────

#[test]
fn every_row_on_every_platform_has_an_id() {
    for os in EVERY_OS {
        for kind in AppKind::ALL {
            for candidate in candidates(os, kind) {
                assert!(
                    candidate.id.parse::<AppId>().is_ok(),
                    "{os:?}/{kind:?} {} has an id no one could send back: {:?}",
                    candidate.name,
                    candidate.id
                );
            }
        }
    }
}

#[test]
fn no_two_rows_of_one_list_share_an_id() {
    // An id is how a choice is spelled, so two rows answering to one id would
    // make a choice ambiguous — and `find` would silently pick the earlier.
    for os in EVERY_OS {
        for kind in AppKind::ALL {
            let mut seen = BTreeSet::new();
            for candidate in candidates(os, kind) {
                assert!(
                    seen.insert(candidate.id),
                    "{os:?}/{kind:?} has two rows called {:?}",
                    candidate.id
                );
            }
        }
    }
}

#[test]
fn the_same_application_has_the_same_id_on_every_platform() {
    // What makes a choice survive moving machines: a workspace that prefers
    // `vscode` on a Mac still prefers it on Linux, and resolves to that
    // platform's row rather than to nothing.
    for (kind, shared) in [
        (AppKind::Editor, "vscode"),
        (AppKind::Editor, "cursor"),
        (AppKind::Editor, "zed"),
        (AppKind::Browser, "firefox"),
        (AppKind::Browser, "chrome"),
    ] {
        for os in EVERY_OS {
            assert!(
                find(os, kind, &id(shared)).is_some(),
                "{os:?} has no {kind:?} called {shared}"
            );
        }
    }
}

#[test]
fn an_id_that_names_no_row_resolves_to_nothing() {
    // The whole boundary in one assertion: there is no id that becomes something
    // to run, because becoming anything at all means being found in the table.
    for smuggled in ["code", "usr-bin-env", "sh", "nonexistent", "vscode-1"] {
        assert!(
            find(Os::MacOs, AppKind::Editor, &id(smuggled)).is_none(),
            "{smuggled} resolved to a candidate"
        );
    }
}

#[test]
fn an_application_id_cannot_be_shaped_like_a_path_or_a_command() {
    for refused in [
        "/usr/bin/code",
        "code --wait",
        "../sh",
        "Visual Studio Code",
        "VSCode",
        "code;rm",
        "",
        &"x".repeat(33),
    ] {
        assert!(
            refused.parse::<AppId>().is_err(),
            "{refused:?} parsed as an application id"
        );
    }
}

#[test]
fn a_catalogue_says_what_is_here_and_what_could_be_opened_with() {
    let macos = mira_platform::Applications::for_os(Os::MacOs).choices(AppKind::Editor);

    assert_eq!(macos.kind, AppKind::Editor);
    assert_eq!(
        macos.options.len(),
        candidates(Os::MacOs, AppKind::Editor).len()
    );

    // Neovim is an editor and is not something a windowed application can open a
    // folder in. The row is offered and says so, rather than being hidden.
    let neovim = macos
        .options
        .iter()
        .find(|option| option.id == id("neovim"))
        .expect("neovim is in the list");
    assert!(!neovim.openable);

    let code = macos
        .options
        .iter()
        .find(|option| option.id == id("vscode"))
        .expect("vscode is in the list");
    assert!(code.openable);
}

// ── What a choice resolves to ────────────────────────────────────────────────

#[test]
fn nothing_chosen_is_automatic_and_says_what_that_is_today() {
    let answer = chosen(Os::MacOs, AppKind::Editor, None, only(&["zed"]));

    assert_eq!(
        answer,
        ChosenApp::Automatic {
            application: Some("Zed".to_owned())
        }
    );
}

#[test]
fn automatic_on_a_machine_with_nothing_says_nothing_rather_than_guessing() {
    let answer = chosen(Os::MacOs, AppKind::Editor, None, nothing);

    assert_eq!(answer, ChosenApp::Automatic { application: None });
}

#[test]
fn a_chosen_application_that_is_here_is_ready() {
    let answer = chosen(Os::MacOs, AppKind::Editor, Some(&id("zed")), only(&["zed"]));

    assert_eq!(
        answer,
        ChosenApp::Ready {
            id: id("zed"),
            name: "Zed".to_owned()
        }
    );
}

#[test]
fn a_chosen_application_that_is_gone_says_which_one() {
    // The honest unavailable state. It names the application, and it keeps the
    // id — so plugging the machine back into the world it was chosen on restores
    // the choice rather than requiring it again.
    let answer = chosen(Os::MacOs, AppKind::Editor, Some(&id("zed")), nothing);

    assert_eq!(
        answer,
        ChosenApp::Missing {
            id: id("zed"),
            name: "Zed".to_owned()
        }
    );
}

#[test]
fn a_chosen_application_that_cannot_open_a_folder_says_that_instead() {
    // Different from missing, and a different thing to do about it.
    let answer = chosen(
        Os::MacOs,
        AppKind::Editor,
        Some(&id("neovim")),
        only(&["neovim"]),
    );

    assert_eq!(
        answer,
        ChosenApp::NotOpenable {
            id: id("neovim"),
            name: "Neovim".to_owned()
        }
    );
}

#[test]
fn a_choice_this_platform_has_never_heard_of_is_its_own_state() {
    // Chosen on another machine, on a platform whose list has no such row —
    // `nova` is macOS-only. Not an error and not a silent reset: a sentence.
    let answer = chosen(Os::Linux, AppKind::Editor, Some(&id("nova")), everything);

    assert_eq!(answer, ChosenApp::Unknown { id: id("nova") });
}

// ── What a choice does to a launch ───────────────────────────────────────────

#[test]
fn a_choice_is_obeyed_even_when_something_earlier_in_the_list_is_here() {
    // Without a choice this is VS Code, because it is first. The whole point of
    // the slice is that the list's order stops deciding.
    let plan = plan(
        Os::MacOs,
        AppKind::Editor,
        Some(&id("zed")),
        root(),
        everything,
    )
    .expect("plan");

    assert_eq!(plan.application, Some("Zed"));
    assert_eq!(
        plan.method,
        LaunchMethod::Bundle {
            bundle: "/Applications/Zed.app"
        }
    );
}

#[test]
fn a_chosen_application_that_is_gone_opens_nothing_at_all() {
    // The property this slice most needs to hold. VS Code is installed and is
    // first in the list; the workspace chose Zed; Zed is not here. Mira refuses,
    // and the refusal names Zed.
    let refused = plan(
        Os::MacOs,
        AppKind::Editor,
        Some(&id("zed")),
        root(),
        only(&["vscode"]),
    );

    let Err(MiraError::NotFound { what }) = refused else {
        panic!("a missing choice must not fall through to another application: {refused:?}");
    };
    assert!(what.contains("Zed"), "the refusal must name it: {what}");
    assert!(
        !what.contains("Visual Studio Code"),
        "and must not offer a substitute: {what}"
    );
}

#[test]
fn a_choice_that_names_no_row_is_refused_rather_than_ignored() {
    let refused = plan(
        Os::MacOs,
        AppKind::Editor,
        Some(&id("emacs")),
        root(),
        everything,
    );

    assert!(
        matches!(refused, Err(MiraError::Invalid { .. })),
        "got {refused:?}"
    );
}

#[test]
fn a_chosen_editor_that_cannot_open_a_folder_is_refused_by_name() {
    let refused = plan(
        Os::MacOs,
        AppKind::Editor,
        Some(&id("neovim")),
        root(),
        everything,
    );

    let Err(MiraError::Unsupported { reason, .. }) = refused else {
        panic!("got {refused:?}");
    };
    assert!(reason.contains("Neovim"), "{reason}");
}

#[test]
fn a_chosen_browser_opens_in_that_browser_rather_than_the_default_handler() {
    // Without a choice a service goes to the desktop's own browser, because
    // which browser you use is a choice already made. Stating one here is that
    // same choice, said to Mira.
    let service = LaunchTarget::WebAddress("http://localhost:3000".to_owned());
    let plan = plan(
        Os::MacOs,
        AppKind::Browser,
        Some(&id("firefox")),
        service.clone(),
        everything,
    )
    .expect("plan");

    assert_eq!(plan.application, Some("Firefox"));
    assert_eq!(plan.target, service);
    assert_eq!(
        plan.method,
        LaunchMethod::Bundle {
            bundle: "/Applications/Firefox.app"
        }
    );
}

#[test]
fn no_choice_still_hands_a_service_to_the_desktop() {
    let plan = plan(
        Os::MacOs,
        AppKind::Browser,
        None,
        LaunchTarget::WebAddress("http://localhost:3000".to_owned()),
        everything,
    )
    .expect("plan");

    assert_eq!(plan.method, LaunchMethod::DefaultHandler);
    assert_eq!(plan.application, None);
}

#[test]
fn a_chosen_terminal_still_gets_only_its_own_literal_flags() {
    // The argv rule survives choosing: every element but the last is a
    // `&'static str` from the table, and the last is the target.
    let plan = plan(
        Os::Linux,
        AppKind::Terminal,
        Some(&id("kitty")),
        root(),
        everything,
    )
    .expect("plan");

    assert_eq!(
        plan.method,
        LaunchMethod::Program {
            program: "kitty",
            args: &["--directory"]
        }
    );
    assert_eq!(
        plan.argv(),
        vec![
            std::ffi::OsString::from("--directory"),
            std::ffi::OsString::from("/home/dev/aviora")
        ]
    );
}

#[test]
fn a_desktop_entry_can_be_chosen_and_cannot_be_started() {
    // A Flatpak leaves a `.desktop` file and nothing on `PATH`. The row proves
    // the application is installed; it is not a program name, so choosing it is
    // a refusal that says why rather than a launch of something else.
    let entry = candidates(Os::Linux, AppKind::Editor)
        .iter()
        .find(|candidate| {
            candidate.launch == Launch::NotFromHere && candidate.id.contains("desktop")
        })
        .expect("a desktop-only row");

    let refused = plan(
        Os::Linux,
        AppKind::Editor,
        Some(&id(entry.id)),
        root(),
        everything,
    );

    assert!(
        matches!(refused, Err(MiraError::Unsupported { .. })),
        "got {refused:?}"
    );
}

#[test]
fn choosing_changes_which_application_and_nothing_about_the_target() {
    // A choice may not widen what gets handed over. Same root, every choice.
    for chosen_id in ["vscode", "cursor", "zed", "sublime-text", "nova", "xcode"] {
        let plan = plan(
            Os::MacOs,
            AppKind::Editor,
            Some(&id(chosen_id)),
            root(),
            everything,
        )
        .expect("plan");

        assert_eq!(plan.target, root());
        assert!(plan.argv().is_empty(), "a bundle takes no argv");
    }
}
