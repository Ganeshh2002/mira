//! Whether an editor, a terminal, and a browser exist on this machine.
//!
//! Mira answers three questions and no more: *is there an editor here, is there
//! a terminal, is there a browser*. It does not enumerate installed
//! applications, and it does not ask what the user's default is — both would be
//! reading more of the machine than the feature needs.
//!
//! **No platform is assumed.** `macOS = VS Code` is wrong for anyone using Zed;
//! `Linux = gnome-terminal` is wrong for most of Linux. Each platform gets an
//! ordered list of candidates, the first one present wins, and none present is a
//! real answer the interface shows rather than hides.
//!
//! Nothing here runs anything. A candidate is checked by looking for a file — a
//! bundle on macOS, a program on `PATH`, a `.desktop` entry on Linux — and the
//! check is a parameter to [`first_present`], so the pure part is testable for all
//! three platforms from any one of them.
//!
//! Finding an application and *opening something with it* are separate questions,
//! and this module answers both without conflating them: [`Launch`] records which
//! candidates can be opened with, and [`crate::launch`] does the opening.

use std::path::{Path, PathBuf};

use mira_core::{AppId, AppKind};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::env::Os;

/// How to tell whether one application is present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Probe {
    /// A macOS application bundle at an absolute path.
    Bundle(&'static str),
    /// A program to look for on `PATH`. A bare name, never a command line.
    Program(&'static str),
    /// A freedesktop `.desktop` entry id, without the extension.
    Desktop(&'static str),
}

/// Whether Mira can open something *with* an application, once it is found.
///
/// Discovery and launching ask different questions, and a candidate may answer
/// the first and not the second. Neovim is an editor by any measure, so "do you
/// have an editor" is yes — but starting it from a windowed application produces
/// a headless process nobody can see, so "open this here" has no answer. Saying
/// that in the table is what keeps the two from drifting apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Launch {
    /// Openable. The parts are literal argv elements placed *before* the target
    /// when the application is started by program name — a terminal's way of
    /// being told which directory. Nothing here is ever derived from input.
    With(&'static [&'static str]),
    /// Found, and not something Mira can open a directory in.
    NotFromHere,
}

/// The common case: the target is the only argument.
const OPENS: Launch = Launch::With(&[]);

/// One application Mira knows how to look for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Candidate {
    /// How this row is named when it is chosen.
    ///
    /// **The same id on every platform for the same application**, so a
    /// workspace that prefers `vscode` still prefers it after moving from a Mac
    /// to a Linux machine — the row it resolves to changes, the choice does not.
    /// Unique within one platform's list for one kind, which a test asserts.
    pub id: &'static str,
    /// What to call it if it is there.
    pub name: &'static str,
    /// Where to look.
    pub probe: Probe,
    /// Whether something can be opened with it, and how.
    pub launch: Launch,
}

const fn bundle(id: &'static str, name: &'static str, path: &'static str) -> Candidate {
    Candidate {
        id,
        name,
        probe: Probe::Bundle(path),
        launch: OPENS,
    }
}

const fn program(id: &'static str, name: &'static str, command: &'static str) -> Candidate {
    Candidate {
        id,
        name,
        probe: Probe::Program(command),
        launch: OPENS,
    }
}

/// A program that takes the directory after a flag of its own.
const fn opens_at(
    id: &'static str,
    name: &'static str,
    command: &'static str,
    args: &'static [&'static str],
) -> Candidate {
    Candidate {
        id,
        name,
        probe: Probe::Program(command),
        launch: Launch::With(args),
    }
}

/// Present, but not something Mira can open anything with.
const fn found_only(id: &'static str, name: &'static str, command: &'static str) -> Candidate {
    Candidate {
        id,
        name,
        probe: Probe::Program(command),
        launch: Launch::NotFromHere,
    }
}

/// A freedesktop entry. Good enough to prove an application is installed —
/// a Flatpak leaves nothing on `PATH` — and not a program name, so nothing is
/// ever started from one.
const fn desktop(id: &'static str, name: &'static str, entry: &'static str) -> Candidate {
    Candidate {
        id,
        name,
        probe: Probe::Desktop(entry),
        launch: Launch::NotFromHere,
    }
}

// The lists are data. Adding an application is adding a row, which is the whole
// point of `architecture.md` §12's "supporting a new editor is a descriptor".

const MACOS_EDITORS: &[Candidate] = &[
    bundle(
        "vscode",
        "Visual Studio Code",
        "/Applications/Visual Studio Code.app",
    ),
    bundle("cursor", "Cursor", "/Applications/Cursor.app"),
    bundle("zed", "Zed", "/Applications/Zed.app"),
    bundle(
        "sublime-text",
        "Sublime Text",
        "/Applications/Sublime Text.app",
    ),
    bundle("nova", "Nova", "/Applications/Nova.app"),
    bundle("xcode", "Xcode", "/Applications/Xcode.app"),
    found_only("neovim", "Neovim", "nvim"),
    found_only("vim", "Vim", "vim"),
];

const MACOS_TERMINALS: &[Candidate] = &[
    bundle("iterm", "iTerm", "/Applications/iTerm.app"),
    bundle("ghostty", "Ghostty", "/Applications/Ghostty.app"),
    bundle("wezterm", "WezTerm", "/Applications/WezTerm.app"),
    bundle("warp", "Warp", "/Applications/Warp.app"),
    bundle("alacritty", "Alacritty", "/Applications/Alacritty.app"),
    // Last because it is always there: anything the user installed on purpose is
    // a better guess at what they want than the one that came with the machine.
    bundle(
        "apple-terminal",
        "Terminal",
        "/System/Applications/Utilities/Terminal.app",
    ),
];

const MACOS_BROWSERS: &[Candidate] = &[
    bundle("arc", "Arc", "/Applications/Arc.app"),
    bundle("chrome", "Google Chrome", "/Applications/Google Chrome.app"),
    bundle("firefox", "Firefox", "/Applications/Firefox.app"),
    bundle("edge", "Microsoft Edge", "/Applications/Microsoft Edge.app"),
    bundle("brave", "Brave", "/Applications/Brave Browser.app"),
    bundle("safari", "Safari", "/Applications/Safari.app"),
];

const WINDOWS_EDITORS: &[Candidate] = &[
    program("vscode", "Visual Studio Code", "code"),
    program("cursor", "Cursor", "cursor"),
    program("zed", "Zed", "zed"),
    program("sublime-text", "Sublime Text", "subl"),
    found_only("neovim", "Neovim", "nvim"),
    // Notepad opens a file, not a folder.
    found_only("notepad", "Notepad", "notepad"),
];

const WINDOWS_TERMINALS: &[Candidate] = &[
    opens_at("windows-terminal", "Windows Terminal", "wt", &["-d"]),
    opens_at("powershell", "PowerShell", "pwsh", &["-WorkingDirectory"]),
    // Windows PowerShell has no working-directory switch; opening it would land
    // in the wrong place quietly.
    found_only("windows-powershell", "Windows PowerShell", "powershell"),
];

const WINDOWS_BROWSERS: &[Candidate] = &[
    program("chrome", "Google Chrome", "chrome"),
    program("firefox", "Firefox", "firefox"),
    program("edge", "Microsoft Edge", "msedge"),
];

const LINUX_EDITORS: &[Candidate] = &[
    program("vscode", "Visual Studio Code", "code"),
    program("cursor", "Cursor", "cursor"),
    program("zed", "Zed", "zeditor"),
    found_only("neovim", "Neovim", "nvim"),
    found_only("vim", "Vim", "vim"),
    found_only("emacs", "GNU Emacs", "emacs"),
    desktop("vscode-desktop", "Visual Studio Code", "code"),
];

const LINUX_TERMINALS: &[Candidate] = &[
    opens_at(
        "gnome-terminal",
        "GNOME Terminal",
        "gnome-terminal",
        &["--working-directory"],
    ),
    opens_at("konsole", "Konsole", "konsole", &["--workdir"]),
    opens_at(
        "alacritty",
        "Alacritty",
        "alacritty",
        &["--working-directory"],
    ),
    opens_at("kitty", "Kitty", "kitty", &["--directory"]),
    opens_at("wezterm", "WezTerm", "wezterm", &["start", "--cwd"]),
    opens_at(
        "xfce-terminal",
        "Xfce Terminal",
        "xfce4-terminal",
        &["--working-directory"],
    ),
    // xterm has no working-directory option at all.
    found_only("xterm", "xterm", "xterm"),
];

const LINUX_BROWSERS: &[Candidate] = &[
    program("firefox", "Firefox", "firefox"),
    program("chrome", "Google Chrome", "google-chrome"),
    program("chromium", "Chromium", "chromium"),
    program("brave", "Brave", "brave-browser"),
    desktop("firefox-desktop", "Firefox", "firefox"),
];

/// Where this platform looks for one kind of application, in preference order.
#[must_use]
pub const fn candidates(os: Os, kind: AppKind) -> &'static [Candidate] {
    match (os, kind) {
        (Os::MacOs, AppKind::Editor) => MACOS_EDITORS,
        (Os::MacOs, AppKind::Terminal) => MACOS_TERMINALS,
        (Os::MacOs, AppKind::Browser) => MACOS_BROWSERS,
        (Os::Windows, AppKind::Editor) => WINDOWS_EDITORS,
        (Os::Windows, AppKind::Terminal) => WINDOWS_TERMINALS,
        (Os::Windows, AppKind::Browser) => WINDOWS_BROWSERS,
        (Os::Linux, AppKind::Editor) => LINUX_EDITORS,
        (Os::Linux, AppKind::Terminal) => LINUX_TERMINALS,
        (Os::Linux, AppKind::Browser) => LINUX_BROWSERS,
    }
}

/// Whether an application of some kind is here, and which one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "state", rename_all = "camelCase")]
#[ts(export)]
pub enum AppPresence {
    /// One was found.
    #[serde(rename_all = "camelCase")]
    Available {
        /// What it is called.
        name: String,
    },
    /// None of the candidates is here. A real answer, not a failure.
    NotInstalled,
}

/// One kind of application, and whether this machine has one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppReport {
    /// Which kind was asked about.
    pub kind: AppKind,
    /// What was found.
    pub presence: AppPresence,
}

/// The first candidate that is present *and* can be opened with.
///
/// Existence is only asked about candidates that could be launched at all, so a
/// machine with nothing but Neovim does not pay for a `PATH` walk to find out it
/// still has no editor Mira can open.
pub fn first_openable(
    list: &[Candidate],
    mut exists: impl FnMut(&Candidate) -> bool,
) -> Option<&Candidate> {
    list.iter()
        .find(|candidate| candidate.launch != Launch::NotFromHere && exists(candidate))
}

/// The first candidate `exists` says is present.
///
/// `exists` is a parameter rather than a call, which is what makes every
/// platform's list assertable from any one platform — and what makes it
/// impossible for discovery to do anything other than ask.
pub fn first_present(list: &[Candidate], exists: impl FnMut(&Candidate) -> bool) -> AppPresence {
    let mut exists = exists;

    list.iter()
        .find(|candidate| exists(candidate))
        .map_or(AppPresence::NotInstalled, |candidate| {
            AppPresence::Available {
                name: candidate.name.to_owned(),
            }
        })
}

/// One row of the catalogue, as the interface sees it.
///
/// Everything needed to offer a choice and to be honest about it: what it is
/// called, whether it is here, and whether Mira could actually open something
/// with it. A row that is installed and not openable is a real state — Neovim is
/// an editor and starting it from a windowed application produces a process
/// nobody can see — and saying so is better than hiding the row and letting
/// somebody wonder where their editor went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppOption {
    /// How this row is named when it is chosen.
    pub id: AppId,
    /// What to call it.
    pub name: String,
    /// Whether this machine has it, right now.
    pub installed: bool,
    /// Whether Mira could open something with it if it were chosen.
    pub openable: bool,
}

/// Every application Mira knows to look for, for one kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Catalogue {
    /// Which kind these are.
    pub kind: AppKind,
    /// The rows, in the order Mira would try them.
    pub options: Vec<AppOption>,
    /// What automatic picks here today, if anything.
    ///
    /// Shown beside the automatic choice so it is a description rather than a
    /// mystery — "Automatic · Ghostty" tells you what pressing the button does.
    pub automatic: Option<String>,
}

/// The catalogue row `id` names, if this platform has one for this kind.
///
/// **The only way an id becomes anything.** An id that names no row resolves to
/// nothing, which is what makes an invented one a refusal rather than a launch.
#[must_use]
pub fn find(os: Os, kind: AppKind, id: &AppId) -> Option<&'static Candidate> {
    candidates(os, kind)
        .iter()
        .find(|candidate| candidate.id == id.as_str())
}

/// Application discovery on this machine.
#[derive(Debug, Clone, Copy)]
pub struct Applications {
    os: Os,
}

impl Applications {
    /// Look with this platform's lists.
    #[must_use]
    pub const fn for_os(os: Os) -> Self {
        Self { os }
    }

    /// Look with the running machine's lists.
    #[must_use]
    pub fn detect() -> Self {
        Self::for_os(crate::env::EnvFacts::detect().os)
    }

    /// Every row Mira knows for one kind, with what is here marked.
    ///
    /// Measured before it was built, because it is the one place discovery stops
    /// short-circuiting: the panel asks "is there an editor" and stops at the
    /// first, a chooser has to ask about all of them. The whole catalogue —
    /// three kinds, every row — costs 0.2 ms on macOS and 1.0 ms for the list
    /// with the most `PATH` misses in it, so it is asked on demand and nothing
    /// is kept between requests (ADR-0019).
    #[must_use]
    pub fn choices(&self, kind: AppKind) -> Catalogue {
        let options: Vec<AppOption> = candidates(self.os, kind)
            .iter()
            .filter_map(|candidate| {
                Some(AppOption {
                    id: candidate.id.parse().ok()?,
                    name: candidate.name.to_owned(),
                    installed: probe_for(self.os, candidate),
                    openable: candidate.launch != Launch::NotFromHere,
                })
            })
            .collect();

        // What automatic would do, computed from the rows just probed rather
        // than by walking the list again.
        let automatic = options
            .iter()
            .find(|option| option.installed && option.openable)
            .map(|option| option.name.clone());

        Catalogue {
            kind,
            options,
            automatic,
        }
    }

    /// One answer per kind, in [`AppKind::ALL`] order.
    ///
    /// *Is one here* — the question the Context panel asks. A kind may be
    /// present here and still absent from [`Self::openable`].
    #[must_use]
    pub fn survey(&self) -> Vec<AppReport> {
        AppKind::ALL
            .into_iter()
            .map(|kind| AppReport {
                kind,
                presence: first_present(candidates(self.os, kind), |candidate| {
                    probe_for(self.os, candidate)
                }),
            })
            .collect()
    }

    /// What this machine can open a directory in.
    ///
    /// Editors and terminals only. A browser is not here because a browser opens
    /// a *service*, not a workspace: there is no address to hand it until
    /// something is listening, and the availability question is answered by the
    /// service list rather than by what is installed (slice brief §4).
    #[must_use]
    pub fn openable(&self) -> Vec<AppReport> {
        [AppKind::Editor, AppKind::Terminal]
            .into_iter()
            .map(|kind| AppReport {
                kind,
                presence: first_openable(candidates(self.os, kind), |candidate| {
                    probe_for(self.os, candidate)
                })
                .map_or(AppPresence::NotInstalled, |candidate| {
                    AppPresence::Available {
                        name: candidate.name.to_owned(),
                    }
                }),
            })
            .collect()
    }
}

/// Whether one candidate is actually here.
///
/// Three file checks and no more. Nothing is executed, no registry is read, and
/// no directory is walked: a bundle is a path that either exists or does not, a
/// program is a name looked up across `PATH`, and a desktop entry is a file in
/// one of a fixed set of directories.
///
/// `os` is a parameter because it decides which program spellings are worth
/// looking for, and because it lets a test probe another platform's list. It is
/// the same `os` the candidate list came from.
pub fn probe_for(os: Os, candidate: &Candidate) -> bool {
    match candidate.probe {
        Probe::Bundle(path) => Path::new(path).exists(),
        Probe::Program(program) => on_path(os, program),
        Probe::Desktop(id) => desktop_entry_exists(id),
    }
}

/// Whether `program` is on `PATH`.
///
/// Measured: a **miss** is the expensive case, because it walks every entry
/// before concluding. On a 63-entry `PATH` that is 63 stats, and it was 252
/// before this function knew which platform it was answering for — Windows
/// spells a program four ways and the other two spell it one way, so trying all
/// four everywhere paid Windows' cost on every machine
/// ([ADR-0019](../../../docs/adr/0019-application-preferences.md)).
fn on_path(os: Os, program: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };

    // Checking the documented extensions is cheaper and more predictable than
    // reading PATHEXT — and off Windows there are no extensions to check.
    let suffixes: &[&str] = if os == Os::Windows {
        &[".exe", ".cmd", ".bat", ""]
    } else {
        &[""]
    };

    std::env::split_paths(&path).any(|directory| {
        suffixes
            .iter()
            .any(|suffix| directory.join(format!("{program}{suffix}")).is_file())
    })
}

/// Whether a `.desktop` entry exists in the standard locations.
fn desktop_entry_exists(id: &str) -> bool {
    let file = format!("{id}.desktop");

    directories()
        .into_iter()
        .any(|directory| directory.join("applications").join(&file).is_file())
}

/// The XDG data directories, defaults included.
fn directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();

    if let Some(home) = std::env::var_os("XDG_DATA_HOME") {
        directories.push(PathBuf::from(home));
    } else if let Some(home) = std::env::var_os("HOME") {
        directories.push(PathBuf::from(home).join(".local/share"));
    }

    match std::env::var_os("XDG_DATA_DIRS") {
        Some(dirs) => directories.extend(std::env::split_paths(&dirs)),
        None => {
            directories.push(PathBuf::from("/usr/local/share"));
            directories.push(PathBuf::from("/usr/share"));
        }
    }

    directories
}
