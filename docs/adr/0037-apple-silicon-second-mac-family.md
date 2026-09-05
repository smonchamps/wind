# ADR 0037 — Apple Silicon: a second mac family, cross-built on the Intel Air

**Date**: 2026-09-05 · **Status**: accepted (Chief-Engineer decisions D1-D5 of
2026-09-05, PLAN-APPLE-SILICON § 6) · amends [ADR 0036](0036-macos-distribution-dmg-adhoc-standard-updater.md)

## Context

An Apple Silicon tester asked to enter the beta — the reopening
condition of debt D-62 (macOS Intel-only). The fleet has one Mac, the
Intel MacBook Air; no arm64 Mac to build or run on. The updater plugin
keys the manifest on `{target}-{arch}` from `cfg!(target_arch)`
(`tauri-plugin-updater 2.10.1`, `updater.rs:1318`): an arm64 build
looks up `darwin-aarch64`, never `darwin-x86_64`.

## Decision

1. **Two triples from one machine** (D1): the Air builds
   `x86_64-apple-darwin` natively and `aarch64-apple-darwin` cross —
   `rustup target add`, Xcode's fat SDK, the `cc` crate passing
   `-arch arm64`. No second build machine, no CI release build (the
   minisign key and OAuth secrets stay off a public repo's runners —
   PLAN-MACOS § 3 unchanged).
2. **A second asset family, not a universal binary**: `Wind_<v>_aarch64.dmg`,
   `.app.tar.gz` + `.sig` (D3: Tauri's bundler word, the manifest
   key's word) under the `darwin-aarch64` key. A universal binary
   would double every download and update, and would have to be
   published under TWO keys with ONE signature — the crossing the
   anti-crossing guard exists to forbid (make-release trap 3,
   `patch-manifest.mjs`). The Windows bi-arch model (ADR 0023)
   transposed.
3. **Both families or none**: `release-macos.sh` completes BOTH builds
   before the first upload, uploads the six assets, then patches
   `latest.json` with both keys once. `verify-release.ps1` expects the
   two families and the two keys together (11 assets, 4 keys): an x64
   family without its aarch64 twin is a broken release, not a "not
   yet".
4. **The CI net proves both triples** (D2): `quality-macos` is a matrix;
   the aarch64 leg runs natively on the `macos-latest` runner (real
   runtime), the x64 leg under Rosetta 2 as before. One rust-cache per
   leg.
5. **Vehicle**: 0.19.0 (D4) — the first mac release carries both archs.

## Consequences

- The mac half of a release roughly doubles on the Air (two ~13 min
  builds, two key-password prompts).
- **The arm64 binary is never run by the fleet**: the build proof is
  the Air's exit status + `file … arm64` (E0, measured before the
  commit — D5); the RUN proof is the tester's first install, stated at
  the GO — the same status as the mac update path at the second mac
  release (ADR 0036).
- `/bin/bash` on macOS is 3.2: the script keeps to arrays and
  functions, no `declare -A`.
- Debt D-62 is closed; D-60 (notarization) and D-61 (no mac e2e)
  apply to both families unchanged.
