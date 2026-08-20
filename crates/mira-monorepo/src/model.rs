//! What Mira says about a repository's shape.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A tool that declares where a repository's packages live.
///
/// Presence of the tool is a fact about the repository. Which packages exist is a
/// separate question, and not every tool answers it: Turborepo delegates to the
/// package manager, and Mira says so rather than guessing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MonorepoTool {
    /// `workspaces` in the root `package.json`, run by npm.
    NpmWorkspaces,
    /// `packages` in `pnpm-workspace.yaml`.
    PnpmWorkspaces,
    /// `workspaces` in the root `package.json`, in Yarn's object form.
    YarnWorkspaces,
    /// `[workspace] members` in the root `Cargo.toml`.
    CargoWorkspace,
    /// `turbo.json`. Declares a monorepo; defers the package list.
    Turborepo,
    /// `nx.json`, with projects under the layout it declares.
    Nx,
}

impl MonorepoTool {
    /// Every tool, in the order they are reported.
    pub const ALL: [Self; 6] = [
        Self::NpmWorkspaces,
        Self::PnpmWorkspaces,
        Self::YarnWorkspaces,
        Self::CargoWorkspace,
        Self::Turborepo,
        Self::Nx,
    ];

    /// The name the interface shows.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NpmWorkspaces => "npm workspaces",
            Self::PnpmWorkspaces => "pnpm workspaces",
            Self::YarnWorkspaces => "Yarn workspaces",
            Self::CargoWorkspace => "Cargo workspace",
            Self::Turborepo => "Turborepo",
            Self::Nx => "Nx",
        }
    }
}

/// One package inside a monorepo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Package {
    /// The name its own manifest gives it, or the directory name when it has none.
    ///
    /// Never invented: if neither exists, the directory is not reported at all.
    pub name: String,
    /// Where it sits, relative to the monorepo root, with `/` separators.
    pub path: String,
}

/// How the selected directory relates to the repository around it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum RepositoryLayout {
    /// One directory, one project. The ordinary case, and the default whenever
    /// nothing more specific can be established.
    Standalone,

    /// The selected directory declares a workspace.
    #[serde(rename_all = "camelCase")]
    MonorepoRoot {
        /// Every tool found here. More than one is normal — pnpm and Turborepo
        /// commonly sit together — and reporting only the first would be less true.
        tools: Vec<MonorepoTool>,
        /// The packages the tools declare and the filesystem confirms.
        packages: Vec<Package>,
    },

    /// The selected directory is one package of a workspace declared above it.
    #[serde(rename_all = "camelCase")]
    Package {
        /// The tools found at the monorepo root.
        tools: Vec<MonorepoTool>,
        /// The absolute path of the monorepo root.
        monorepo_root: String,
        /// This package's path relative to that root, with `/` separators.
        package_path: String,
        /// This package's own name.
        package_name: String,
    },
}
