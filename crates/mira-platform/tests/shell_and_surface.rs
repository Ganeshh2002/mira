//! The two OS-facing decisions Slice 1 adds: how to open a directory in the
//! platform's file manager, and which window material to ask for.
//!
//! Both are resolved as *data* — an argv array, a treatment name — so they can be
//! asserted for all three operating systems from any one of them. Actually spawning
//! a file manager is the one line that cannot be tested without opening a window on
//! the developer's screen, and it is the only line not covered here.

use std::path::Path;

use mira_core::Capability;
use mira_platform::{
    reveal_command, surface_treatment, DisplayServer, EnvFacts, LinuxPackaging, Os, Platform,
    PlatformCapabilities, SurfaceTreatment,
};

fn facts(os: Os, display_server: DisplayServer) -> EnvFacts {
    EnvFacts {
        os,
        display_server,
        has_logind: false,
        packaging: LinuxPackaging::NotApplicable,
    }
}

// ── Opening a directory ──────────────────────────────────────────────────────

#[test]
fn macos_opens_a_directory_with_open() {
    let (program, args) = reveal_command(Os::MacOs, Path::new("/Users/dev/aviora"));

    assert_eq!(program, "open");
    assert_eq!(args, [Path::new("/Users/dev/aviora")]);
}

#[test]
fn windows_opens_a_directory_with_explorer() {
    let (program, args) = reveal_command(Os::Windows, Path::new(r"C:\dev\aviora"));

    assert_eq!(program, "explorer");
    assert_eq!(args, [Path::new(r"C:\dev\aviora")]);
}

#[test]
fn linux_opens_a_directory_with_xdg_open() {
    let (program, args) = reveal_command(Os::Linux, Path::new("/home/dev/aviora"));

    assert_eq!(program, "xdg-open");
    assert_eq!(args, [Path::new("/home/dev/aviora")]);
}

#[test]
fn the_path_is_an_argument_and_never_part_of_the_program() {
    // security-and-privacy.md §5 rule 1. A directory called `; rm -rf ~` is one
    // argument, not a command, because there is no shell anywhere on this path.
    let hostile = Path::new("/home/dev/; rm -rf ~/");

    for os in [Os::MacOs, Os::Windows, Os::Linux] {
        let (program, args) = reveal_command(os, hostile);

        assert!(
            !program.contains(';') && !program.contains(' '),
            "{os:?}: the program is a fixed name, never built from input"
        );
        assert_eq!(args, [hostile], "{os:?}: the path stays one whole argument");
    }
}

#[test]
fn the_program_is_one_of_three_fixed_names() {
    let programs: Vec<&str> = [Os::MacOs, Os::Windows, Os::Linux]
        .into_iter()
        .map(|os| reveal_command(os, Path::new("/tmp")).0)
        .collect();

    for program in &programs {
        assert!(
            ["open", "explorer", "xdg-open"].contains(program),
            "{program} is not one of the three programs Mira may run"
        );
    }
}

#[test]
fn revealing_is_available_on_every_platform_mira_supports() {
    // The capability is Full on macOS and Windows and Degraded on Linux, where the
    // file manager opens the folder without selecting anything. Degraded still
    // works, so the action is offered everywhere (platform-abstraction §5).
    for (os, display_server) in [
        (Os::MacOs, DisplayServer::Quartz),
        (Os::Windows, DisplayServer::Dwm),
        (Os::Linux, DisplayServer::Wayland),
    ] {
        let platform = Platform::from_facts(facts(os, display_server));

        assert!(
            platform.status(Capability::RevealInFileManager).is_usable(),
            "{os:?} can open a project directory"
        );
    }
}

// ── Path identity ────────────────────────────────────────────────────────────

#[test]
fn linux_paths_are_case_sensitive_and_the_others_are_not() {
    assert!(facts(Os::Linux, DisplayServer::X11).paths_are_case_sensitive());
    assert!(!facts(Os::MacOs, DisplayServer::Quartz).paths_are_case_sensitive());
    assert!(!facts(Os::Windows, DisplayServer::Dwm).paths_are_case_sensitive());
}

// ── Window material ──────────────────────────────────────────────────────────

#[test]
fn macos_asks_for_the_system_material() {
    // On macOS 26 the system material *is* Liquid Glass. Mira asks AppKit for its
    // current material rather than drawing an imitation of one, which is why this
    // needs no version check: the platform decides what the material looks like.
    assert_eq!(
        surface_treatment(&facts(Os::MacOs, DisplayServer::Quartz)),
        SurfaceTreatment::SystemMaterial
    );
}

#[test]
fn windows_asks_for_the_system_material_too() {
    assert_eq!(
        surface_treatment(&facts(Os::Windows, DisplayServer::Dwm)),
        SurfaceTreatment::SystemMaterial
    );
}

#[test]
fn linux_stays_opaque_because_no_material_is_portable_there() {
    for display_server in [
        DisplayServer::X11,
        DisplayServer::Wayland,
        DisplayServer::Unknown,
    ] {
        assert_eq!(
            surface_treatment(&facts(Os::Linux, display_server)),
            SurfaceTreatment::Opaque,
            "{display_server:?}: blur depends on the compositor, so Mira does not claim it"
        );
    }
}

#[test]
fn every_treatment_names_itself_for_the_interface() {
    for treatment in [SurfaceTreatment::SystemMaterial, SurfaceTreatment::Opaque] {
        assert!(
            !treatment.token().is_empty(),
            "the frontend selects a treatment by name, never by operating system"
        );
    }
    assert_ne!(
        SurfaceTreatment::SystemMaterial.token(),
        SurfaceTreatment::Opaque.token()
    );
}
