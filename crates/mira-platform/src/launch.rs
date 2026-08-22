//! Opening a workspace's context in a real application.
//!
//! The trusted boundary this module exists to draw, read downwards:
//!
//! ```text
//! Frontend  → a workspace id and a kind. Never a path, never a command.
//! Typed IPC → deserialises into a row id and a three-variant enum.
//! Service   → workspace → project → canonical root.
//! Launcher  → the root, plus a row from a table compiled into the binary.
//! ```
//!
//! Nothing crosses that boundary except **one target**: a directory Mira
//! resolved from its own database, or an address Mira built from a port it is
//! already watching. Everything else in an argv is a `&'static str` from
//! [`crate::applications`], so "Mira cannot be made to run an arbitrary command"
//! is a property of the types rather than of a review
//! (`security-and-privacy.md` §5 rules 1 and 5).
//!
//! [`plan`] is pure and takes its existence check as a parameter, so every
//! platform's decision is assertable from any one platform, and [`Perform`] is a
//! seam so a test can watch what *would* have started without starting it.

use std::ffi::OsString;
use std::path::PathBuf;

use mira_core::{AppId, AppKind, Capability, MiraError, Result};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::applications::{candidates, first_openable, probe_for, Candidate, Launch, Probe};
use crate::env::Os;
use crate::macos;
use crate::platform::PlatformCapabilities;
use crate::shell::{is_openable, open_url_command, spawn};
use crate::{AppReport, Applications};

/// The one thing an application is handed.
///
/// Two shapes, because there are two questions: *where are you working* and
/// *what is listening*. Neither is ever spoken by the interface — a directory
/// comes from a project row the user registered through the native picker, and
/// an address is built in Rust from an observed port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchTarget {
    /// A project root. What an editor or a terminal opens at.
    Directory(PathBuf),
    /// A web address Mira constructed. What a browser opens.
    WebAddress(String),
}

/// How an application is started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMethod {
    /// macOS: ask the window server to open the target with this bundle.
    /// A launch, not a child process — the application ends up owned by the
    /// system rather than by Mira, which is what makes quitting Mira harmless.
    Bundle {
        /// The absolute path of the `.app`.
        bundle: &'static str,
    },
    /// Windows and Linux: start this program with this argv.
    Program {
        /// The program name, looked up on `PATH`.
        program: &'static str,
        /// Literal flags, placed before the target.
        args: &'static [&'static str],
    },
    /// Hand the target to whatever the desktop registered for it.
    ///
    /// Used for web addresses, and only for them. Mira knows which browsers are
    /// installed, but *which browser you use* is a choice you already made; a
    /// service opening in Chrome because Chrome is installed, when the default
    /// is Safari, is the unrelated-application substitution §6 forbids.
    DefaultHandler,
}

/// A resolved launch: everything decided, nothing done.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchPlan {
    /// What the user will see start, when Mira picked it. `None` for
    /// [`LaunchMethod::DefaultHandler`], where the desktop picks.
    pub application: Option<&'static str>,
    /// How to start it.
    pub method: LaunchMethod,
    /// What it is handed.
    pub target: LaunchTarget,
}

impl LaunchPlan {
    /// The argv for a [`LaunchMethod::Program`]: literals, then the one target.
    ///
    /// Empty for the other methods, which do not use one.
    #[must_use]
    pub fn argv(&self) -> Vec<OsString> {
        let LaunchMethod::Program { args, .. } = self.method else {
            return Vec::new();
        };

        let mut argv: Vec<OsString> = args.iter().map(OsString::from).collect();
        argv.push(match &self.target {
            LaunchTarget::Directory(path) => OsString::from(path),
            LaunchTarget::WebAddress(url) => OsString::from(url),
        });
        argv
    }
}

/// What was started.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Launched {
    /// The application Mira chose, or `None` when the desktop chose.
    pub application: Option<String>,
}

/// What a workspace's choice for one kind resolves to on this machine.
///
/// Five shapes because there are five different sentences, and collapsing any
/// two of them would be a lie about somebody's machine. In particular
/// [`ChosenApp::Missing`] is **not** a reason to open something else: quietly
/// starting a different editor than the one that was chosen is the
/// unrelated-application substitution ADR-0013 refused, and a choice makes it
/// worse rather than better (ADR-0019).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum ChosenApp {
    /// Nothing was chosen. Mira takes the first row this machine has.
    #[serde(rename_all = "camelCase")]
    Automatic {
        /// What that is today, or `None` when nothing here fits.
        application: Option<String>,
    },
    /// Chosen, here, and openable.
    #[serde(rename_all = "camelCase")]
    Ready {
        /// The catalogue row.
        id: AppId,
        /// What to call it.
        name: String,
    },
    /// Chosen, and not on this machine any more.
    #[serde(rename_all = "camelCase")]
    Missing {
        /// The catalogue row, kept so the choice survives the application coming back.
        id: AppId,
        /// What to call it.
        name: String,
    },
    /// Chosen and here, and not something Mira can open a directory in.
    #[serde(rename_all = "camelCase")]
    NotOpenable {
        /// The catalogue row.
        id: AppId,
        /// What to call it.
        name: String,
    },
    /// Chosen on some other machine, and not in this platform's list at all.
    #[serde(rename_all = "camelCase")]
    Unknown {
        /// What was stored, so the interface can say which one it was.
        id: AppId,
    },
}

/// Resolve a workspace's choice against this machine, without touching anything.
///
/// Pure, and takes its existence test as a parameter, so every case is assertable
/// from any platform.
pub fn chosen(
    os: Os,
    kind: AppKind,
    preferred: Option<&AppId>,
    exists: impl FnMut(&Candidate) -> bool,
) -> ChosenApp {
    let mut exists = exists;

    let Some(id) = preferred else {
        return ChosenApp::Automatic {
            application: automatic(os, kind, &mut exists)
                .map(|candidate| candidate.name.to_owned()),
        };
    };

    let Some(candidate) = crate::applications::find(os, kind, id) else {
        return ChosenApp::Unknown { id: id.clone() };
    };
    let name = candidate.name.to_owned();

    if !exists(candidate) {
        return ChosenApp::Missing {
            id: id.clone(),
            name,
        };
    }
    if candidate.launch == Launch::NotFromHere {
        return ChosenApp::NotOpenable {
            id: id.clone(),
            name,
        };
    }

    ChosenApp::Ready {
        id: id.clone(),
        name,
    }
}

/// What automatic resolves to for one kind: the first row here Mira can open with.
fn automatic<'a>(
    os: Os,
    kind: AppKind,
    exists: impl FnMut(&Candidate) -> bool,
) -> Option<&'a Candidate> {
    first_openable(candidates(os, kind), exists)
}

/// Decide what to open `target` with, without touching anything.
///
/// `preferred` is a workspace's choice, and it is obeyed or refused — never
/// substituted. A chosen application that is not here is an error naming it, so
/// the person finds out which one went missing rather than finding out that
/// something else opened.
///
/// # Errors
///
/// [`MiraError::Invalid`] if the kind and the target disagree, or if a web
/// address is not one Mira will open; [`MiraError::NotFound`] if a chosen
/// application is not on this machine; [`MiraError::Unsupported`] if this machine
/// has no application of that kind that Mira can open something with.
pub fn plan(
    os: Os,
    kind: AppKind,
    preferred: Option<&AppId>,
    target: LaunchTarget,
    exists: impl FnMut(&Candidate) -> bool,
) -> Result<LaunchPlan> {
    let target = agreed(kind, target)?;
    let mut exists = exists;

    let candidate = match preferred {
        // A choice is answered by the choice. Every way it can fail is its own
        // sentence, and none of them is "so Mira opened something else".
        Some(id) => match chosen(os, kind, Some(id), &mut exists) {
            ChosenApp::Ready { .. } => {
                crate::applications::find(os, kind, id).ok_or_else(|| unknown_choice(id))?
            }
            ChosenApp::Missing { name, .. } => return Err(gone(&name)),
            ChosenApp::NotOpenable { name, .. } => return Err(not_from_here(&name)),
            ChosenApp::Unknown { id } => return Err(unknown_choice(&id)),
            // `chosen` only answers Automatic when nothing was preferred.
            ChosenApp::Automatic { .. } => return Err(unknown_choice(id)),
        },

        // Nothing chosen, and a browser: presence decides whether the action
        // exists, and the desktop decides which browser runs. *Which* browser you
        // use is a choice already made elsewhere, so Mira does not overrule it
        // with the first row in a list.
        None if kind == AppKind::Browser => {
            if !candidates(os, kind).iter().any(&mut exists) {
                return Err(nothing_here(kind));
            }
            return Ok(LaunchPlan {
                application: None,
                method: LaunchMethod::DefaultHandler,
                target,
            });
        }

        None => automatic(os, kind, &mut exists).ok_or_else(|| nothing_here(kind))?,
    };

    let Launch::With(args) = candidate.launch else {
        return Err(nothing_here(kind));
    };

    let method = match candidate.probe {
        Probe::Bundle(bundle) => LaunchMethod::Bundle { bundle },
        Probe::Program(program) => LaunchMethod::Program { program, args },
        // Neither path returns one: a desktop entry proves an application is
        // installed and is not a program name, so it is never openable.
        Probe::Desktop(_) => return Err(not_from_here(candidate.name)),
    };

    Ok(LaunchPlan {
        application: Some(candidate.name),
        method,
        target,
    })
}

fn gone(name: &str) -> MiraError {
    MiraError::NotFound {
        what: format!(
            "{name}, which this workspace uses. Mira did not open anything else — choose \
             another application and it will remember"
        ),
    }
}

fn not_from_here(name: &str) -> MiraError {
    MiraError::Unsupported {
        capability: Capability::LaunchApplication,
        reason: format!("{name} is here, and is not something Mira can open a folder in."),
    }
}

fn unknown_choice(id: &AppId) -> MiraError {
    MiraError::invalid(
        "application",
        format!("Mira has no application called {id} on this platform."),
    )
}

/// The target, if it is the shape this kind opens.
fn agreed(kind: AppKind, target: LaunchTarget) -> Result<LaunchTarget> {
    match (kind, &target) {
        (AppKind::Editor | AppKind::Terminal, LaunchTarget::Directory(_)) => Ok(target),
        (AppKind::Browser, LaunchTarget::WebAddress(url)) => {
            if is_openable(url) {
                Ok(target)
            } else {
                Err(MiraError::invalid(
                    "target",
                    format!("Mira only opens web addresses, and {url} is not one."),
                ))
            }
        }
        (AppKind::Editor | AppKind::Terminal, LaunchTarget::WebAddress(_)) => Err(
            MiraError::invalid("target", "An editor and a terminal open a folder."),
        ),
        (AppKind::Browser, LaunchTarget::Directory(_)) => Err(MiraError::invalid(
            "target",
            "A browser opens a service, not a folder.",
        )),
    }
}

fn nothing_here(kind: AppKind) -> MiraError {
    MiraError::Unsupported {
        capability: Capability::LaunchApplication,
        reason: format!(
            "Mira could not find {} on this machine that it knows how to open.",
            match kind {
                AppKind::Editor => "an editor",
                AppKind::Terminal => "a terminal",
                AppKind::Browser => "a browser",
            }
        ),
    }
}

/// Carrying out a resolved plan.
///
/// A trait so the decision can be tested to the last step without a window ever
/// appearing: production uses [`Desktop`], and tests use one that writes the plan
/// down (slice brief §11).
pub trait Perform {
    /// Start what the plan describes.
    ///
    /// # Errors
    ///
    /// [`MiraError::External`] if the application could not be started, and
    /// [`MiraError::Unsupported`] for a method this platform has no way to run.
    fn perform(&self, plan: &LaunchPlan) -> Result<()>;
}

impl<T: Perform + ?Sized> Perform for &T {
    fn perform(&self, plan: &LaunchPlan) -> Result<()> {
        (**self).perform(plan)
    }
}

/// The real desktop.
#[derive(Debug, Clone, Copy)]
pub struct Desktop {
    os: Os,
}

impl Desktop {
    /// Perform through this platform's mechanism.
    #[must_use]
    pub const fn new(os: Os) -> Self {
        Self { os }
    }
}

impl Perform for Desktop {
    fn perform(&self, plan: &LaunchPlan) -> Result<()> {
        match plan.method {
            LaunchMethod::Bundle { bundle } => macos::open_with_application(bundle, &plan.target),
            LaunchMethod::Program { program, .. } => spawn(
                program,
                &plan.argv(),
                plan.application.unwrap_or("the application"),
                &described(plan),
            ),
            LaunchMethod::DefaultHandler => self.hand_over(plan),
        }
    }
}

impl Desktop {
    /// Give the target to the desktop's own handler.
    fn hand_over(&self, plan: &LaunchPlan) -> Result<()> {
        let LaunchTarget::WebAddress(url) = &plan.target else {
            return Err(MiraError::invalid(
                "target",
                "Only a web address is handed to the desktop.",
            ));
        };

        if self.os == Os::MacOs {
            return macos::open_with_default(url);
        }

        let (program, args) = open_url_command(self.os, url);
        spawn(program, &args, "the browser", url)
    }
}

fn described(plan: &LaunchPlan) -> String {
    match &plan.target {
        LaunchTarget::Directory(path) => path.display().to_string(),
        LaunchTarget::WebAddress(url) => url.clone(),
    }
}

/// What Mira may ask the desktop to start.
pub trait LaunchHost {
    /// The kinds this machine can open a directory in, and with what.
    fn openable(&self) -> Vec<AppReport>;

    /// What a workspace's choice resolves to here, without starting anything.
    ///
    /// The question the interface asks *before* offering a button, so a chosen
    /// application that has been uninstalled is a sentence on the row rather
    /// than an error after a click.
    fn chosen(&self, kind: AppKind, preferred: Option<&AppId>) -> ChosenApp;

    /// Open `target` in an application of `kind`, obeying a choice if there is one.
    ///
    /// # Errors
    ///
    /// [`MiraError::Unsupported`] where launching is off or no such application
    /// is here, [`MiraError::NotFound`] if a chosen application is gone,
    /// [`MiraError::Invalid`] if the target is the wrong shape, and
    /// [`MiraError::External`] if the application refused to start.
    fn launch(
        &self,
        kind: AppKind,
        preferred: Option<&AppId>,
        target: LaunchTarget,
    ) -> Result<Launched>;
}

/// Launching on this machine.
#[derive(Debug, Clone, Copy)]
pub struct Launcher<P, E = Desktop> {
    os: Os,
    platform: P,
    desktop: E,
}

impl<P: PlatformCapabilities> Launcher<P, Desktop> {
    /// Launch for real.
    pub const fn new(os: Os, platform: P) -> Self {
        Self {
            os,
            platform,
            desktop: Desktop::new(os),
        }
    }
}

impl<P: PlatformCapabilities, E: Perform> Launcher<P, E> {
    /// Launch through some other performer. The seam tests use.
    pub const fn with(os: Os, platform: P, desktop: E) -> Self {
        Self {
            os,
            platform,
            desktop,
        }
    }
}

impl<P: PlatformCapabilities, E: Perform> LaunchHost for Launcher<P, E> {
    fn openable(&self) -> Vec<AppReport> {
        if !self
            .platform
            .status(Capability::LaunchApplication)
            .is_usable()
        {
            return Vec::new();
        }

        Applications::for_os(self.os).openable()
    }

    fn chosen(&self, kind: AppKind, preferred: Option<&AppId>) -> ChosenApp {
        chosen(self.os, kind, preferred, |candidate| {
            probe_for(self.os, candidate)
        })
    }

    fn launch(
        &self,
        kind: AppKind,
        preferred: Option<&AppId>,
        target: LaunchTarget,
    ) -> Result<Launched> {
        let status = self.platform.status(Capability::LaunchApplication);
        if !status.is_usable() {
            return Err(MiraError::Unsupported {
                capability: Capability::LaunchApplication,
                reason: match status {
                    mira_core::CapabilityStatus::Unavailable { reason, .. } => reason,
                    _ => "Mira cannot start applications on this machine.".to_owned(),
                },
            });
        }

        let plan = plan(self.os, kind, preferred, target, |candidate| {
            probe_for(self.os, candidate)
        })?;
        self.desktop.perform(&plan)?;

        Ok(Launched {
            application: plan.application.map(str::to_owned),
        })
    }
}
