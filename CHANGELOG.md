# Changelog

All notable changes to Aviora Mira are documented here.

Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning: [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Product definition, product scope, PRD, information architecture, and design system
- Architecture, platform abstraction, data model, and security/privacy documents
- ADRs 0001–0009 covering the foundational technical decisions
- Locked phase plan (0.1 through 0.6+) and the fourteen-slice implementation roadmap
- Open-source project files (licence, contributing, code of conduct, security policy)
- **Foundation (slice 0).** Cargo workspace with `mira-core`, `mira-platform`, `mira-db`,
  `mira-projects`, and `mira-workspaces` behind the Tauri application boundary; runtime
  capability resolution reported as full, degraded, or unavailable with a reason; SQLite
  storage with a forward-only migration runner; typed commands with structured errors and
  TypeScript types generated from Rust; window, tray, settings surface, and global
  shortcut with a documented fallback; guard tests for the architectural rules; CI on
  macOS, Windows, and Linux.
- **Projects and Git context (slice 1).** Add a project by choosing a folder; Mira infers
  the name, detects Git by walking up to the worktree root, and records the project types
  it finds at the root. Projects persist, list most-recently-opened first, and keep
  independent state. Selecting one reads its Git context on demand: branch or detached
  HEAD, last commit with author and age, clean or changed, and ahead/behind against the
  tracked upstream — labelled as of the last fetch, because Mira does not fetch. Open a
  project's folder in the file manager, or remove a project without touching the folder.
  Read-only throughout: there is no commit, stage, checkout, push, or pull.
- macOS and Windows ask the platform for its standard window material — Liquid Glass on
  macOS 26, Mica on Windows 11 — rather than drawing an imitation. Linux stays opaque.

The first release will be `0.1.0` — **the MVP** — covering roadmap slices 1–3 and 5a: projects, Git status and history, ports and
processes, application launching, tray, and the global shortcut. Ports, processes and
application launching are not built yet. Everything from 0.2 on is post-MVP and does not
block it. See [product scope](docs/product/product-scope.md).
