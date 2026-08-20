//! Monorepo awareness.
//!
//! A directory a person points Mira at can be three things: a project on its own,
//! the root of a monorepo, or one package inside one. Telling them apart is a
//! question about *workspace manifests*, not about Git, which is why this crate
//! knows nothing about repositories beyond taking a worktree root as a boundary.
//!
//! Three rules hold throughout:
//!
//! 1. **Read-only.** Manifests are read as files. No package manager runs, no
//!    script is executed, nothing is installed, and nothing is written.
//! 2. **Nothing is invented.** A directory is a package only when the manifest
//!    that would make it one is actually there. A name comes from that manifest or
//!    from the directory; there is no third source.
//! 3. **Bounded.** Every walk has a depth cap, a visit budget, and a skip list
//!    (`expand`). Detection on a pathological tree returns less, never late.
//!
//! See [ADR-0010](../../../docs/adr/0010-monorepo-detection.md).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod discover;
mod expand;
mod model;
mod pnpm;
mod providers;

pub use discover::detect;
pub use model::{MonorepoTool, Package, RepositoryLayout};
