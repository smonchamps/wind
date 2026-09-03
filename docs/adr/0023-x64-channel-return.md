# ADR 0023 — Return of the x64 channel: bi-arch release (arm64 + x64)

**Date**: 2026-08-22 · **Status**: accepted (CE decisions D5-D8 of
2026-08-22, PLAN-RETOURS-8 § Decisions)

## Context

The x64 channel was **removed in 0.1.3** (PLAN-WIND E4): the only user
workstation was ARM64 and ran the x64 app under emulation. Since then,
`make-release.ps1` only builds the host (arm64), the Release carries 3
assets and `latest.json` a single `windows-aarch64` key. The CE
directive of 2026-08-22 (PLAN-RETOURS-8 R3) reopens the channel:
**every release ships x64 AND arm64**.

Investigation facts:

- **The Tauri updater picks its own platform**: the `{os}-{arch}` key
  comes from the installed binary's compile-time constants. One
  `latest.json` therefore serves both channels; it is enough to add
  `windows-x86_64` to it. **Nothing to change on the Rust side.**
- **The version is global to the manifest**: both architectures ship
  at the same version, or not at all.
- **Cross-building x64 from the ARM64 workstation is proven** (E1,
  2026-08-22): MSVC 14.50 toolset with x64 libs already installed, the
  rustup target added, the `lld-link` override extended to the x64
  triple (the `link.exe` trap from Git Bash, already paid on arm64,
  replayed identically); `cargo tauri build --target
  x86_64-pc-windows-msvc` links and bundles in 1 min 45 s →
  `target/x86_64-pc-windows-msvc/release/bundle/nsis/
  Wind_<v>_x64-setup.exe`. The `quality` CI (windows-latest, x64) was
  already proving compile + tests continuously.
- **A silent failure specific to bi-arch**: a missing platform key or
  crossed signatures produce NO error whatsoever — the updater of the
  mute channel concludes "no update." Third member of the ADR 0013
  trap family (BOM, `v` tag).

## Decision

1. **Local cross-build on the ARM64 workstation** (D6) — two
   `cargo tauri build --target <triple>` in `make-release.ps1`, the
   signing key never leaves the workstation (one same key signs both
   channels), password asked once. CI stays a gate, never a release
   builder.
2. **All-or-nothing** (D7) — a failed build blocks the whole release:
   never a lagging channel, never a partial manifest.
3. **`latest.json` with two keys**, built per platform from ITS
   target's directory; **cross-signing guard encoded** (the two
   signatures must be distinct) — never left to vigilance.
4. **`verify-release.ps1`** scripts the §2.10 verification (5 named
   assets, BOM, two keys, signatures == `.sig` and distinct, URLs that
   resolve) — with two platforms the manual checks were doubling.
5. **Field proof per channel** (D5): arm64 on this workstation; x64 on
   a **second x64 workstation** — never under emulation (the reason
   for the 0.1.3 removal). The first x64 auto-update is only
   observable at the release following the first bi-arch one; the x64
   install is observable as of that one.
6. **The §2.9 MAJOR criterion is evaluated PER CHANNEL**: a break in
   auto-update on a single channel is enough to trigger MAJOR. Adding
   the x64 channel is not one (arm64 workstations keep reading their
   key) → the first bi-arch release is MINOR (D8).

## Consequences

- Release time ~doubled (two builds, ~4 min each) — accepted, the
  `YES` confirmation still comes after the builds.
- `install-workstation.ps1` (preparing an x64 workstation) described
  bi-arch as a "separate job" — reversed.
- The six "3 assets at the bare tag" mentions in ETAT's history stay
  true for THEIR versions; the current norm is "5 assets" (§2.10
  amended).

## Set aside — x64 build in GitHub CI

The `windows-latest` runner is x64-native (no cross-build), but the
signing key would become a GitHub Actions secret and the release a
two-place process. Set aside (D6) as long as the local cross-build
holds — to reopen if a local x64 build fails persistently.
