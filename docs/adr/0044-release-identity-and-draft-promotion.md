# ADR 0044 — Release identity, draft-then-Latest and local signature proof

Date: 2026-09-07. Status: implemented for audit lot 4 (E12); field proof at
the next actual release. Chief Engineer decision D1 of 2026-09-07.

## Finding

The release chain had three gaps the 2026-09-06 audit named B29, B30 and B31.
The Windows script built from the working tree and committed only its bump
files, so any local edit was baked into the binaries and never committed; the
mac script checked a clean tree but not that HEAD was the tagged commit. The
GitHub Release was created public and Latest with the two Windows keys, the
six macOS assets and their keys arriving later from the Air: between the two,
every mac updater read a manifest without its key. The verification script
handed Tauri's `.sig` asset — the base64 of the minisign text — to a minisign
parser expecting the text, and when the tool was absent it printed "NOT
PROVEN" without counting a failure, so its final verdict passed with no proof.

## Decision

1. **Build identity.** `make-release.ps1` refuses a branch other than `main`
   and a non-empty `git status --porcelain` before any write. It bumps the
   version files, refreshes the lockfile (`cargo update --workspace`), commits
   the release commit, and only then builds both channels from that HEAD; a
   tree modified by the builds is refused. `release-macos.sh` refuses a HEAD
   that is not the tag's commit. Each platform writes an attestation (version,
   tag, commit, branch, `Cargo.lock` and dist digests, digest of every
   artifact) and uploads it with its assets.
2. **Draft, then Latest.** The Release is created as a draft with the Windows
   half; the mac half is added to the draft; `publish-release.ps1` downloads
   every asset of the draft, proves the whole matrix and promotes it with
   `gh release edit --draft=false --latest`. Until then the previous Latest
   stays complete for every updater. A day without the Air is an explicit
   `-WindowsOnly` promotion that prints what it drops. The bare tag is pushed
   by the Windows script (a draft creates none) so both scripts and the
   promotion verify HEAD against it.
3. **Local cryptographic proof.** `tools/release-verify` decodes the Tauri
   wrappers of the public key and of every `.sig` and verifies with
   `minisign-verify`, the crate the shipped updater itself uses; fixtures
   prove valid, tampered, wrong-key, broken-wrapper and missing cases. Both
   `publish-release.ps1` and `verify-release.ps1` call it for every channel;
   an absent proof is a failure. `verify-release.ps1 -Structural` is the
   explicitly incomplete mode and says so in its verdict.
4. **One decision module.** The promotion rule and the attestation live in
   `scripts/release-lib.mjs`, proven by `node --test` on every push and in
   CI; the PowerShell and bash scripts do the I/O. The Windows script's
   guards and ordering are proven against fake `git`/`gh`/`cargo`/`rustup`
   stubs (`e2e/release-scripts.test.mjs`), a net shown red on the previous
   script before it went green.

## Consequences and limits

The Windows channel is no longer public before the mac half (same day, two
machines). The eleven-asset, four-key matrix plus two attestations is the
complete release; five assets, two keys and one attestation only under the
stated exception. The mac script's own guards are covered by `bash -n` and by
the shared module, not by a stub harness (its prerequisites are macOS tools).
Draft assets are not publicly downloadable, so the public URL checks run after
promotion, by `verify-release.ps1`. The field proof of the whole chain is the
next actual release, observed per channel as before (ADR 0013).
