//! Which window material to ask the platform for.
//!
//! macOS 26 restyled every system material as Liquid Glass. Mira does not draw an
//! imitation of it: it asks AppKit for the standard window material and gets
//! whatever that means on the running version — Liquid Glass on 26, vibrancy
//! before it. That is the difference between using the platform's design and
//! copying its screenshots, and it is why nothing here checks an OS version.
//!
//! Windows gets the same treatment through Mica. Linux gets none: blur there
//! depends on the compositor and the desktop, and claiming it would be exactly the
//! fake parity `platform-abstraction.md` §2 forbids.
//!
//! The treatment is named, not implied. The frontend applies
//! `data-surface="<token>"` and never asks which operating system it is on
//! (ADR-0005).

use crate::env::{EnvFacts, Os};

/// How the window ground should be rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceTreatment {
    /// Ask the platform for its standard window material and let the ground show
    /// it through. Liquid Glass on macOS 26, vibrancy before it, Mica on Windows.
    SystemMaterial,

    /// Paint the ground solid. The default, and the only honest answer where no
    /// portable material exists.
    Opaque,
}

impl SurfaceTreatment {
    /// The name the frontend selects on.
    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::SystemMaterial => "systemMaterial",
            Self::Opaque => "opaque",
        }
    }
}

/// The treatment this machine should be asked for.
///
/// "Asked for" is deliberate: applying it can still fail, and the shell reports
/// what it actually achieved rather than what it wanted.
#[must_use]
pub const fn surface_treatment(facts: &EnvFacts) -> SurfaceTreatment {
    match facts.os {
        Os::MacOs | Os::Windows => SurfaceTreatment::SystemMaterial,
        Os::Linux => SurfaceTreatment::Opaque,
    }
}
