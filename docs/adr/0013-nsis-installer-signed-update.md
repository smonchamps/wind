# ADR 0013 — NSIS installer and signed automatic update

Date: 2026-07-26 · Status: accepted — **loop validated in the field the
same day** (0.1.1 → 0.1.2 applied to the installed app, database intact)
· **Amended by [ADR 0023](0023-x64-channel-return.md)** (2026-08-22):
**bi-arch** release (arm64 + x64, 5 assets, `latest.json` with two keys)
and publication **entirely scripted** by `make-release.ps1` since
0.1.10 — the mentions "three assets", "manual publication" and the
asset name `discovery_<v>_x64-setup.exe` below are from that era (the
prefix is `Wind_`, the arches `arm64`/`x64`).

## Context

Phase 5 ([PLAN.md](../PLAN.md) §5) asks for an "**MSIX** installer +
signed automatic update". Two points to settle before coding.

### The installer format — "MSIX" is an inherited bound

The plan named MSIX **before** a trap was found in the field:
**MSIX virtualizes `%APPDATA%`**. An app packaged as MSIX does not
write to the real `%APPDATA%\Roaming\<id>`, but to a private container
under `%LOCALAPPDATA%\Packages\<PackageFamilyName>\…`. This is exactly
the mechanism that keeps the assistant from reading the real database
(the Claude app is MSIX — see handover §7.1).

Now **the entire data model of Discovery is one SQLite file** at
`%APPDATA%\dev.discovery.app\discovery.db`, resolved by
`AppHandle::app_data_dir()`. On the Chief Engineer's machine, this file
weighs **~715 MB** (256,312 messages). Packaging Discovery **itself**
as MSIX would redirect this path into the package's container: **the
existing database would become orphaned**, and the migration of
[ADR 0012] — which precisely adopts the database found at this path —
would no longer see it.

This is the lesson "an inherited bound is not a decided bound"
(handover §9): "MSIX" was a hypothesis of the plan, not a measurement.

### What "signed" covers — two distinct signatures

- **Updater signature** (minisign, built into Tauri): guarantees that
  an update cannot be forged between publication and installation.
  **Free, mandatory** — without it, the updater refuses to apply a
  package. This is not a decision.
- **Windows code signature** (publisher certificate): removes the
  SmartScreen "unknown publisher" warning. Costs money, commits an
  identity. Distinct from the previous one.

## Decisions

1. **Installer: NSIS, not MSIX.** Already built, measured at
   **4.75 MB** ([PLAN.md](../PLAN.md) §3), `installMode: currentUser` —
   this is the mode that plants the Start menu shortcut and the
   AppUserModelID that notifications need (handover §7.2). NSIS **does
   not touch `%APPDATA%`**: the database stays where it is. The Tauri
   updater targets NSIS natively; MSIX updates through App
   Installer/Store, a different, heavier mechanism. **§188 of PLAN.md
   is corrected by this ADR.**

2. **Update signed by the Tauri updater (minisign).** A key pair is
   generated **by the Chief Engineer** (`tauri signer generate`): the
   **public** key is written into `tauri.conf.json` (public by nature,
   it is committed); the **private** key is a **secret** that signs
   every publication and **never** touches the repository (§2.4: zero
   secret in the clear). Without it generated, `cargo tauri build`
   produces no update artifacts — but the pre-push gate does not bundle
   (§7.4), so the repository stays green in the meantime.

3. **Windows code signature: deferred to the public launch.** The
   closed beta (20-50 people notified) runs with the minisign-signed
   updater — update integrity is assured. SmartScreen will show
   "unknown publisher" (one "Run anyway" click); the certificate choice
   (Azure Trusted Signing, ~$10/month, or classic OV) is settled before
   the public launch, not now. Deferral assumed.

4. **Channel: GitHub Releases.** The repository is already there; the
   `…/releases/latest/download/latest.json` endpoint is free and native
   for the updater. The manifest and the signed packages are published
   there.

5. **Updater driven from Rust**, like the notifications: the webview
   never calls the updater API, only our commands. The capabilities
   stay `core:default` — least privilege preserved.

## What is done here

- `tauri-plugin-updater` added, registered in `main.rs`.
- `tauri.conf.json`: `plugins.updater` block (GitHub endpoint, public
  key **pending**), `bundle.createUpdaterArtifacts: true`.
- Commands `update_check` (returns the available version, or nothing)
  and `update_install` (downloads, applies, restarts).
- Discreet banner, **outside any `<header>`** (CSS debt, handover §8):
  "An update is available" + "Install and restart" / "Later". Checked
  at startup, once, silently offline — a check the user would have to
  request would not happen (lesson of [ADR 0007]).

**Honest test surface.** The updater is almost entirely publisher
configuration and platform I/O (download, replacing the running
binary): there is no pure decision to extract and test in RED, unlike
sync or threads. Saying so rather than faking a test that would learn
nothing (§2.4). The proof is **in the field**, as for the
notifications.

## Field validation (2026-07-26 — done, end to end)

The full loop touches signing, the network, and replacing the live
binary — it can only be proven on the installed app (as with the
notifications, §7.2). **Played and validated**: the minisign key pair
generated (private outside the repository, public in
`tauri.conf.json`), a signed `0.1.1` built, installed, launched; a
`0.1.2` published as a GitHub Release; the installed `0.1.1`
**detected, downloaded and applied** `0.1.2`, database intact. Banner,
restart, new version, zero loss.

### Two field traps, and their permanent remedy

Neither was a logic defect — both were **false assumptions about the
tooling** (the underlying lesson, handover §9):

1. **The hand-written `latest.json` got corrupted**: a multi-line
   PowerShell paste ended up writing the *command text itself* into the
   file; and `Set-Content -Encoding utf8` would have added a BOM there
   that the updater (`serde_json`) refuses.
2. **The `.exe` returned 404**: the manifest pointed to
   `releases/download/v0.1.2/…` while the Release tag is the **bare
   version** (`0.1.2`). The banner appeared — detection worked — but
   the install failed on the download.

**Remedy:** [`scripts/make-release.ps1`](../../scripts/make-release.ps1)
`<version>` — reads the built `.sig`, writes `latest.json` **without a
BOM** and with the **bare-tag URL**. Publication (attaching the three
files to the tag) stays manual. The friction is encoded once, never
repaid again.

### Publication convention (frozen)

- **Tag = bare version**: `0.1.2`, never `v0.1.2`.
- The Release must be **published**, neither draft nor pre-release, and
  marked *latest* — otherwise `releases/latest/download/…` returns 404.
- The three assets: `discovery_<version>_x64-setup.exe`, its `.sig`,
  and `latest.json`.

## Consequences and limits

- **No update rollback**: if a `0.1.1` is bad, a `0.1.2` is published.
  The updater does not go back down a version — consistent with "a
  duplicate is worse than a delay": move forward, never back.
- **SmartScreen warns** in beta: to document in the invitation.
- **The `latest.json` manifest is public**: it holds only a version, a
  URL and a signature — no user data.
