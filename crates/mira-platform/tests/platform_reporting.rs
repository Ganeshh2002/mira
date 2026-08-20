//! The cached platform view the application shell consumes.

use mira_core::{Capability, CapabilityStatus};
use mira_platform::{
    resolve, DisplayServer, EnvFacts, LinuxPackaging, Os, Platform, PlatformCapabilities,
};

fn wayland() -> EnvFacts {
    EnvFacts {
        os: Os::Linux,
        display_server: DisplayServer::Wayland,
        has_logind: true,
        packaging: LinuxPackaging::Managed,
    }
}

#[test]
fn reports_every_capability_once_in_declaration_order() {
    let platform = Platform::from_facts(wayland());
    let reports = platform.report();

    let seen: Vec<Capability> = reports.iter().map(|r| r.capability).collect();
    assert_eq!(
        seen,
        Capability::ALL.to_vec(),
        "the report is the capability matrix; a missing row is a capability nobody was honest about"
    );
}

#[test]
fn every_report_carries_the_label_so_the_ui_keeps_no_second_copy() {
    let reports = Platform::from_facts(wayland()).report();
    assert_eq!(reports.len(), Capability::ALL.len());
    for report in reports {
        assert_eq!(report.label, report.capability.label());
        assert!(!report.label.is_empty());
    }
}

#[test]
fn status_agrees_with_the_pure_resolver() {
    let facts = wayland();
    let platform = Platform::from_facts(facts.clone());

    for capability in Capability::ALL {
        assert_eq!(
            platform.status(capability),
            resolve(capability, &facts),
            "the cached status for {capability:?} drifted from the resolver"
        );
    }
}

#[test]
fn a_wayland_machine_reports_its_shortcut_fallback_through_the_trait() {
    let platform = Platform::from_facts(wayland());
    match platform.status(Capability::GlobalShortcut) {
        CapabilityStatus::Unavailable { fallback, .. } => {
            assert_eq!(fallback.as_deref(), Some("mira --toggle"));
        }
        other => panic!("expected Unavailable, got {other:?}"),
    }
}

#[test]
fn facts_survive_resolution_so_settings_can_explain_the_session() {
    let platform = Platform::from_facts(wayland());
    assert_eq!(platform.facts().display_server, DisplayServer::Wayland);
    assert_eq!(platform.facts().os.label(), "Linux");
}
