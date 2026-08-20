# Security Policy

Mira is a local desktop application that reads sensitive parts of a developer's machine —
project paths, running processes, SSH configuration names, and the Docker socket. We take
reports seriously and would rather hear about a maybe-issue than miss a real one.

## Reporting a vulnerability

**Do not open a public issue for a security problem.**

Report privately, either way:

1. **GitHub Security Advisories** — the "Report a vulnerability" button on the repository's
   Security tab (preferred; it gives us a private thread and a CVE path).
2. **Email** — **security@aviora.dev**.

Please include: what the issue is, how to reproduce it, the affected version and platform,
and what an attacker could achieve. A proof of concept helps; a rough description is still
welcome.

## What to expect

| Stage | Target |
|---|---|
| Acknowledgement | within 72 hours |
| Initial assessment | within 7 days |
| Fix for confirmed high severity | prioritised over feature work |
| Public disclosure | after a fix ships, coordinated with you |

We will credit you in the advisory and the changelog unless you prefer otherwise.

## Supported versions

Mira is pre-1.0. **Only the latest release receives security fixes.** There are no
backports to older `0.x` versions.

## Scope

**In scope** — anything that lets an attacker, or hostile content, do something the user
did not ask for:

- Command or argument injection through project configuration, paths, or app templates
- Path traversal or symlink escape allowing reads outside registered project roots
- Rendering untrusted file content (Peek), filenames, branch names, or commit messages in
  a way that executes code
- Exposure of SSH key material, credentials, or environment values
- Privilege escalation, or terminating a process the user should not be able to terminate
- Weaknesses in update verification
- Any unexpected network connection (Mira should make none by default)
- Bypassing the frontend's lack of filesystem, shell, or network capability

**Out of scope:**

- Attacks requiring an attacker who already controls the user's account or root — Mira
  cannot defend against that and does not claim to
- Missing hardening with no demonstrated impact
- Vulnerabilities in third-party applications Mira launches
- Social engineering, physical access, denial of service against the local machine
- Reports from automated scanners with no working proof of concept

## Security design

Mira's security properties are documented in
[docs/architecture/security-and-privacy.md](docs/architecture/security-and-privacy.md).
The load-bearing ones:

- No shell is ever invoked; child processes get argv arrays
- The frontend has no filesystem, shell, or network capability — only named commands
- File reads are confined to registered project roots and shelf entries
- SSH private keys are never opened; Docker access is read-only
- No telemetry, no account, and no outbound connection unless the user asks

Several of these are enforced by guard tests that run on every commit. If you find a way
around one, that is a valid report even if you cannot yet show impact.
