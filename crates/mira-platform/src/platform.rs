//! The resolved platform.

use mira_core::{Capability, CapabilityReport, CapabilityStatus};

use crate::env::EnvFacts;

/// What the rest of Mira is allowed to ask about the operating system.
pub trait PlatformCapabilities {
    /// The status of one capability on this machine.
    fn status(&self, capability: Capability) -> CapabilityStatus;

    /// Every capability and its status.
    fn report(&self) -> Vec<CapabilityReport>;
}

/// One machine, observed once.
#[derive(Debug, Clone)]
pub struct Platform {
    facts: EnvFacts,
    reports: Vec<CapabilityReport>,
}

impl Platform {
    /// Observe the real machine and resolve every capability.
    #[must_use]
    pub fn detect() -> Self {
        Self::from_facts(EnvFacts::detect())
    }

    /// Resolve every capability against facts supplied by the caller.
    ///
    /// Resolution happens once, here. Statuses are cached rather than recomputed per
    /// query, per `docs/architecture/platform-abstraction.md` §2 rule 2 — they are
    /// re-resolved by building a new `Platform` when a relevant system event arrives.
    #[must_use]
    pub fn from_facts(facts: EnvFacts) -> Self {
        let reports = crate::resolve::resolve_all(&facts);
        Self { facts, reports }
    }

    /// What was observed.
    #[must_use]
    pub const fn facts(&self) -> &EnvFacts {
        &self.facts
    }
}

impl PlatformCapabilities for Platform {
    fn status(&self, capability: Capability) -> CapabilityStatus {
        self.reports
            .iter()
            .find(|report| report.capability == capability)
            .map_or(CapabilityStatus::Full, |report| report.status.clone())
    }

    fn report(&self) -> Vec<CapabilityReport> {
        self.reports.clone()
    }
}
