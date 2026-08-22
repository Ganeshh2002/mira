//! Resolving a launch: which application, and what it is handed.
//!
//! Every test here is about the *decision*, not the act. `plan` takes the
//! existence check as a parameter, so every platform's answer is assertable from
//! any one platform, and nothing is ever started — the one test that goes as far
//! as launching uses a performer that writes the plan down instead
//! (slice brief §11: never launch real applications from automated tests).

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use mira_core::{AppKind, Capability, MiraError};
use mira_platform::{
    candidates, plan, AppPresence, Candidate, Launch, LaunchHost, LaunchMethod, LaunchTarget,
    Launcher, Os, Perform, Platform, Probe,
};

fn root() -> LaunchTarget {
    LaunchTarget::Directory(PathBuf::from("/home/dev/aviora"))
}

fn service() -> LaunchTarget {
    LaunchTarget::WebAddress("http://localhost:3000".to_owned())
}

/// An existence check that says yes to exactly these applications.
fn only(names: &'static [&'static str]) -> impl Fn(&Candidate) -> bool {
    move |candidate| names.contains(&candidate.name)
}

fn nothing(_: &Candidate) -> bool {
    false
}

fn everything(_: &Candidate) -> bool {
    true
}

// ── Editors ──────────────────────────────────────────────────────────────────

#[test]
fn an_editor_is_handed_the_project_root() {
    let plan = plan(Os::MacOs, AppKind::Editor, None, root(), everything).expect("plan");

    assert_eq!(plan.application, Some("Visual Studio Code"));
    assert_eq!(plan.target, root());
    assert_eq!(
        plan.method,
        LaunchMethod::Bundle {
            bundle: "/Applications/Visual Studio Code.app"
        }
    );
}

#[test]
fn the_first_editor_that_is_actually_there_wins() {
    // Preference order is the list's order, and absence skips rather than fails.
    let plan = plan(Os::MacOs, AppKind::Editor, None, root(), only(&["Zed"])).expect("plan");

    assert_eq!(plan.application, Some("Zed"));
}

#[test]
fn an_editor_that_needs_a_terminal_is_not_offered() {
    // Neovim is a real editor and a real answer to "do you have one". Starting
    // it from a windowed application gets a headless process nobody can see, so
    // it is discovered but never launched.
    assert!(matches!(
        plan(
            Os::MacOs,
            AppKind::Editor,
            None,
            root(),
            only(&["Neovim", "Vim"])
        ),
        Err(MiraError::Unsupported { .. })
    ));
}

#[test]
fn every_platform_can_open_an_editor() {
    for os in [Os::MacOs, Os::Windows, Os::Linux] {
        let plan = plan(os, AppKind::Editor, None, root(), everything)
            .unwrap_or_else(|error| panic!("{os:?} has no way to open an editor: {error:?}"));
        assert_eq!(plan.target, root(), "{os:?}");
    }
}

// ── Terminals ────────────────────────────────────────────────────────────────

#[test]
fn a_terminal_opens_at_the_project_root() {
    let plan = plan(
        Os::Linux,
        AppKind::Terminal,
        None,
        root(),
        only(&["Konsole"]),
    )
    .expect("plan");

    assert_eq!(plan.application, Some("Konsole"));
    assert_eq!(
        plan.method,
        LaunchMethod::Program {
            program: "konsole",
            args: &["--workdir"]
        }
    );
    assert_eq!(
        plan.argv(),
        vec![
            std::ffi::OsString::from("--workdir"),
            std::ffi::OsString::from("/home/dev/aviora")
        ]
    );
}

#[test]
fn no_terminal_is_offered_that_cannot_be_told_where_to_open() {
    // A terminal Mira cannot point at a directory would open at $HOME, which is
    // the wrong project silently. Better to have no button than a wrong one.
    for os in [Os::MacOs, Os::Windows, Os::Linux] {
        for candidate in candidates(os, AppKind::Terminal) {
            let Launch::With(args) = candidate.launch else {
                continue;
            };
            match candidate.probe {
                // macOS hands the directory to the bundle as a URL; there is no
                // flag to carry.
                Probe::Bundle(_) => {}
                Probe::Program(_) => assert!(
                    !args.is_empty(),
                    "{} on {os:?} is offered with no way to say which directory",
                    candidate.name
                ),
                Probe::Desktop(_) => panic!("{} cannot be launched", candidate.name),
            }
        }
    }
}

#[test]
fn every_platform_can_open_a_terminal() {
    for os in [Os::MacOs, Os::Windows, Os::Linux] {
        let plan = plan(os, AppKind::Terminal, None, root(), everything)
            .unwrap_or_else(|error| panic!("{os:?} has no way to open a terminal: {error:?}"));
        assert_eq!(plan.target, root(), "{os:?}");
    }
}

// ── Browsers ─────────────────────────────────────────────────────────────────

#[test]
fn a_web_address_goes_to_the_browser_the_person_chose() {
    // Not to the first browser in Mira's list. Opening a service in Chrome
    // because Chrome is installed, when the default is Safari, is precisely the
    // "unrelated application" substitution the brief forbids (§6).
    for os in [Os::MacOs, Os::Windows, Os::Linux] {
        let plan = plan(os, AppKind::Browser, None, service(), everything).expect("plan");

        assert_eq!(plan.method, LaunchMethod::DefaultHandler, "{os:?}");
        assert_eq!(plan.application, None, "{os:?}");
    }
}

#[test]
fn there_is_no_browser_action_when_there_is_no_browser() {
    assert!(matches!(
        plan(Os::Linux, AppKind::Browser, None, service(), nothing),
        Err(MiraError::Unsupported { .. })
    ));
}

#[test]
fn only_a_web_address_reaches_the_browser() {
    // The same scheme rule as Slice 2's open_url, applied one layer earlier.
    for refused in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "data:text/html,<script>",
        "mira://anything",
        " http://localhost:3000",
    ] {
        let target = LaunchTarget::WebAddress(refused.to_owned());
        match plan(Os::MacOs, AppKind::Browser, None, target, everything) {
            Err(MiraError::Invalid { .. }) => {}
            other => panic!("{refused} must be refused, got {other:?}"),
        }
    }
}

// ── A kind and its target must agree ─────────────────────────────────────────

#[test]
fn an_editor_is_not_handed_a_web_address() {
    assert!(matches!(
        plan(Os::MacOs, AppKind::Editor, None, service(), everything),
        Err(MiraError::Invalid { .. })
    ));
}

#[test]
fn a_browser_is_not_handed_a_directory() {
    assert!(matches!(
        plan(Os::MacOs, AppKind::Browser, None, root(), everything),
        Err(MiraError::Invalid { .. })
    ));
}

// ── Nothing is derived from input except the one target ──────────────────────

#[test]
fn every_argument_is_a_literal_from_the_table_or_the_target_itself() {
    // The property that makes "no arbitrary command" structural rather than
    // reviewed: an argv is a fixed slice of `&'static str` plus exactly one
    // value, and that value is the resolved directory or the address Mira built.
    for os in [Os::MacOs, Os::Windows, Os::Linux] {
        for kind in [AppKind::Editor, AppKind::Terminal] {
            let Ok(resolved) = plan(os, kind, None, root(), everything) else {
                continue;
            };
            let LaunchMethod::Program { args, .. } = resolved.method else {
                continue;
            };

            let argv = resolved.argv();
            assert_eq!(argv.len(), args.len() + 1, "{os:?} {kind:?}");
            assert_eq!(
                argv.last().map(std::ffi::OsString::as_os_str),
                Some(Path::new("/home/dev/aviora").as_os_str()),
                "the target is the last argument, and there is only one"
            );
        }
    }
}

// ── Launching, without launching ─────────────────────────────────────────────

/// A performer that writes down what it was asked to do.
#[derive(Debug, Default)]
struct Wrote {
    plans: RefCell<Vec<mira_platform::LaunchPlan>>,
}

impl Perform for Wrote {
    fn perform(&self, plan: &mira_platform::LaunchPlan) -> mira_core::Result<()> {
        self.plans.borrow_mut().push(plan.clone());
        Ok(())
    }
}

fn machine() -> Platform {
    Platform::detect()
}

/// This machine, not a named one: whether a launch happens depends on what is
/// installed here, so the assertions below are about the *relationship* between
/// what `openable` promises and what `launch` does — which holds on any machine.
fn here() -> Os {
    mira_platform::EnvFacts::detect().os
}

#[test]
fn what_is_promised_as_openable_is_what_gets_performed() {
    for report in Launcher::with(here(), machine(), Wrote::default()).openable() {
        let wrote = Wrote::default();
        let launcher = Launcher::with(here(), machine(), &wrote);
        let target = LaunchTarget::Directory(PathBuf::from("/tmp"));

        match (report.presence, launcher.launch(report.kind, None, target)) {
            (AppPresence::Available { name }, Ok(launched)) => {
                assert_eq!(
                    launched.application.as_deref(),
                    Some(name.as_str()),
                    "what the caller is told is what the list promised"
                );
                let performed = wrote.plans.borrow();
                assert_eq!(performed.len(), 1, "one launch, one performance");
                assert_eq!(
                    performed[0].application,
                    Some(name.as_str()),
                    "and it is what was performed"
                );
            }
            (AppPresence::NotInstalled, Err(MiraError::Unsupported { capability, .. })) => {
                assert_eq!(capability, Capability::LaunchApplication);
                assert!(
                    wrote.plans.borrow().is_empty(),
                    "a kind with nothing installed must start nothing"
                );
            }
            (presence, result) => {
                panic!(
                    "{presence:?} and {result:?} disagree about {:?}",
                    report.kind
                )
            }
        }
    }
}

#[test]
fn nothing_is_performed_when_the_target_is_the_wrong_shape() {
    let wrote = Wrote::default();
    let launcher = Launcher::with(here(), machine(), &wrote);

    let refused = launcher.launch(
        AppKind::Editor,
        None,
        LaunchTarget::WebAddress("http://x.dev".into()),
    );

    assert!(matches!(refused, Err(MiraError::Invalid { .. })));
    assert!(wrote.plans.borrow().is_empty());
}

#[test]
fn what_can_be_opened_is_reported_per_kind() {
    let launcher = Launcher::with(here(), machine(), Wrote::default());

    assert_eq!(
        launcher
            .openable()
            .iter()
            .map(|report| report.kind)
            .collect::<Vec<_>>(),
        vec![AppKind::Editor, AppKind::Terminal],
        "a browser opens a service, not a workspace, so it is not in this list"
    );
}
