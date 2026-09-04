# Wind

**Wind** — the mail client of the **Elements** suite
("what the wind carries, the rhythm of the days").

Desktop application: Rust core (IMAP, SMTP, OAuth, message rendering)
and Svelte interface, packaged by Tauri. Targets: Windows **arm64 and
x64** (bi-arch release, ADR 0023; NSIS installer, ADR 0013) and
**macOS x64** (dmg, ADR 0036) — automatic signed update on all
channels (minisign, ADR 0013).

The last shipped version and the current state live in
[docs/STATE.md](docs/STATE.md). Preparing for the closed beta.

## Changelog

Versions and their changes are recorded in
[CHANGELOG.md](CHANGELOG.md). The signed packages live in the
[GitHub Releases](https://github.com/smonchamps/wind/releases).

## Documentation

- [docs/STANDARD.md](docs/STANDARD.md) — the method (standing
  instruction), the frozen decisions and the invariants.
- [docs/STATE.md](docs/STATE.md) — the current state: shipped version,
  next job, field figures.
- [docs/WORKFLOW.md](docs/WORKFLOW.md) — the standardized workflows
  (`/job`, `/field`, `/gate`, `/close`).
- [CHANGELOG.md](CHANGELOG.md) — the changelog.

## Build and verify

The commands (test set, installer build, e2e) are described in §7.3 of
[docs/STANDARD.md](docs/STANDARD.md). The full gate — format, UI build,
contrasts, System coherence, clippy, Rust tests and real e2e — is
replayed at pre-push (`.githooks/pre-push`): nothing leaves the machine
without it.
