# PLAN-MACOS — a macOS build of Wind, to widen the beta

> Statement (Chief Engineer, 2026-09-04): *"In order to extend the number of
> testers, we need to create a MacOS version. I just got an old
> MacBook Air which can be used to build the MacOS version, but I
> need everything prepped up in the code and a step by step guide to
> install everything that is needed on MacOS in order to succeed."*

Status: **STOP 1 played on 2026-09-04 — D1-D7 settled (§6), GO from the
Chief Engineer the same day** (flag accepted: the mac update path is only provable
at the SECOND mac release — the first proves install).

---

## 1. Finding (sweep of 2026-09-04, full report in session)

Wind is Windows-only end to end, but the Windows-ness is **shallow in
the product code and deep in the tooling**:

**Code — four real blockers, all small:**

- `Cargo.toml:29` — `keyring` built with `windows-native` only; on
  macOS `mail-auth` has **no secure-storage backend** (Keychain needs
  the `apple-native` feature). Every credential path goes through it.
- `apps/desktop/src/commands.rs:5710-5866` — the update flow is NSIS
  by construction: `MZ` magic-byte check on the download, NSIS
  `/P /R /UPDATE` flags, `installer_command` hardcoded. Meaningless on
  macOS; must be platform-gated.
- `apps/desktop/src/telemetry.rs:274` — `Command::new("explorer")` to
  open the crash-report folder; fails on macOS (needs `open`).
- `apps/desktop/src/instance.rs:53-61` — the pre-Tauri
  `database_folder()` reads `%APPDATA%` directly; on macOS it returns
  `None`, so **the single-instance lock and the early trace file are
  silently skipped** (audit wave 1's E1 undone on the new platform).

**Degrades gracefully, verify only:** `relocation.rs` (Windows-only
migration by construction — no Discovery ever existed on macOS),
`safe_file_name` (stricter than macOS needs, harmless),
`additionalBrowserArgs` (WebView2 flags; must confirm WKWebView
ignores them), `open` crate (uses `open(1)` on macOS), `fs4`, `rfd`,
`rustls-platform-verifier` (Security.framework — ADR 0032's one-stack
rule holds on macOS).

**Bundle/config:** `tauri.conf.json` targets `["nsis"]` only; no
`.icns` (only `icon.ico`; `make-icon.ps1` is GDI+, Windows-only); no
`bundle.macOS` block; updater `latest.json` has only `windows-*` keys.

**Tooling:** CI has no macOS job — a mac-breaking change is invisible
from the Windows workstation. `make-release.ps1`/`verify-release.ps1`
know only the two Windows triples and five Windows assets. The e2e
harness drives WebView2 over CDP (`launch.mjs:276`); **WKWebView
exposes no CDP** — the suite cannot run on macOS as designed.

**Docs:** BETA.md §1-§2 are NSIS + SmartScreen/SAC; "Windows only" is
stated at line 131. No Gatekeeper guidance exists.

**Hard fact not yet measured:** the MacBook Air's chip and macOS
version (About This Mac). This decides the target triple
(`x86_64-apple-darwin` vs `aarch64-apple-darwin`) and whether the
current toolchain (Rust 1.97.1, Tauri 2, recent Xcode CLT) installs at
all. **Genchi genbutsu: D1 asks for the reading before anything is
built.**

## 2. Scope

**In:** the code compiles and runs on macOS (the four blockers), a
`.dmg` the testers can install, a `.icns`, a macOS CI compile net, a
step-by-step build guide for the MacBook Air, a BETA.md macOS section,
release tooling for mac assets per D3/D4.

**Refusals (§2.6):**

- **No macOS e2e harness.** WKWebView has no CDP; porting the 205-spec
  suite means a different automation stack (WebDriver/`tauri-driver`)
  — a job of its own, not a rider. The mac quality bar is D5's call.
- **No universal binary.** One triple — the Air's — for the beta.
  The second darwin arch joins when a tester needs it.
- **No notarization infrastructure** unless D2 chooses the paid path.
- **No port of the PowerShell workstation tooling** (field.ps1,
  gate.ps1, make-icon.ps1). The Mac gets one small build script and a
  guide; the gate stays on Windows, where the e2e proof lives.
- **No relocation migration on macOS** — nothing to migrate.

## 3. Options weighed

- **Gatekeeper**: (a) ad-hoc signed, testers right-click-Open or
  `xattr -d com.apple.quarantine` — free, friction at first launch,
  documented in BETA.md; (b) Apple Developer Program (99 USD/yr),
  Developer ID + notarization — deterministic installs, the exact
  lesson Authenticode taught on SAC (memory: per-binary cloud
  verdicts). → D2.
- **Auto-update**: (a) none for the beta — macOS checks the manifest,
  shows the banner, the button opens the release page (manual dmg);
  (b) full Tauri `.app.tar.gz` + `.sig` updater path. (b) doubles the
  release surface before a single mac tester exists. → D3.
- **Where mac assets are built**: the Air itself (only option — no
  cross-compile of WKWebView from Windows; CI release builds refused
  for now, secrets on a runner + a second signing path).

## 4. Steps

- **E1 — code compiles and behaves on macOS** (buildable from Windows
  CI even before the Mac is ready): keyring per-platform features
  (`windows-native` / `apple-native` via target-specific dependency
  tables), `telemetry_open_folder` platform branch, `database_folder()`
  platform-neutral (Application Support on macOS) so the
  single-instance lock and early trace hold, the NSIS update flow
  behind `#[cfg(windows)]`; on macOS the updater plugin's standard
  path (verify + in-place install + relaunch — D3b), the custom `MZ`
  check and `/P /R /UPDATE` flags never compiled there. TDD where the
  decision is pure; RED shown.
- **E2 — bundle**: `tauri.macos.conf.json` (targets `["app","dmg"]`,
  `minimumSystemVersion` per D1's reading), `.icns` produced on the
  Mac from the icon master (`scripts/make-icns.sh`, `iconutil`),
  committed like `icon.ico` is.
- **E3 — CI net**: a `macos-latest` job — `cargo clippy` +
  `cargo test --workspace` (compile + unit proof; no e2e). Proven by
  breaking it once (a deliberate mac-only error).
- **E4 — Mac build guide**: `docs/MACOS-BUILD.md` — step-by-step for
  the Air: Xcode CLT, rustup (pinned 1.97.1 via rust-toolchain.toml),
  Node LTS, clone + git identity (memory: smonchamps, noreply), npm
  install, `cargo tauri build`, where the `.dmg` lands, the D2 launch
  gesture. One command per block, copy-ready.
- **E5 — release surface (D3b + D4)**: `scripts/release-macos.sh` on
  the Air — builds `x86_64-apple-darwin`, produces `.dmg` +
  `.app.tar.gz` + `.sig` (minisign, same key), uploads them to the
  existing GitHub release; `make-release.ps1` (or the mac script)
  writes the `darwin-x86_64` key into `latest.json`;
  `verify-release.ps1` extended to the darwin assets and key. Order
  engraved in the guide: **Windows release first, mac assets second,
  `latest.json` last** — the manifest never points at an asset that
  is not yet uploaded.
- **E6 — docs**: BETA.md macOS section (install + Gatekeeper gesture),
  line 131 amended; BETA.fr.md mirrored (D11 of ENGLISH-SWITCH keeps
  it alive); README platform line; ADR for the macOS
  distribution/update decision (model 0013's counterpart).

Gate: full `/gate` on Windows at each commit (the ratchet, e2e, IPC
nets all still bind); the macOS CI job green; field on the Air at
STOP 2.

## 4 bis. Delivery record

**E1-E6 implemented on 2026-09-04/05, one commit.** E1: keyring
`apple-native` (workspace features), the update flow split into
per-platform `install_downloaded` (Windows keeps the MZ net and the
NSIS launch; macOS uses the plugin's `install` + `restart`, D3b),
`telemetry_open_folder` through `open::that_detached` (one doorway,
same as `open_link`), `database_folder` on `dirs::data_dir()` (the
source `app_data_dir()` itself reads), `lock_patiently` (the update
relaunch races the lock — 10×200 ms grace, D1 held, proven RED-first),
`compile_error!` naming the platform boundary. E2:
`tauri.macos.conf.json` (app+dmg, `signingIdentity: "-"`,
minimumSystemVersion 10.15), `icon.icns` from the same GDI+ drawing
(824/1024 inset; `icon.ico` regenerated byte-identical — proof of no
drift), System A118. E3: `quality-macos` CI job pinned like `quality`,
`--target x86_64-apple-darwin` on the arm64 runner (Rosetta) so it
proves the SHIPPING triple. E4: `docs/MACOS-BUILD.md`. E5:
`release-macos.sh` (version-pinned dmg glob, preflight incl.
tauri-cli/node_modules), `patch-manifest.mjs` (the manifest patch as a
checked file, not inline JS), `assert-dist-clean.mjs` (the seam
poka-yoke in ONE copy, shared with make-release.ps1, proven by
breaking it), gate step 6 now parses `.sh`, `verify-release.ps1`
(darwin key ↔ assets stand or fall together, dmg resolve check,
derived asset count, N-way signature guard), make-release.ps1 prints
the mac-half reminder. E6: BETA.md/BETA.fr.md (§1 mac install,
Gatekeeper section ordered for macOS 15 — Sequoia removed
right-click-Open), README, ADR 0036, debts D-60/D-61/D-62.

**Fresh-eyes review (5 finder angles): 12 findings retained, 10
fixed** — among them the stale-dmg glob, the missing ad-hoc
`signingIdentity` (an unsigned quarantined app shows "damaged" with no
recourse), the manifest-orphan darwin key passing verification, the
relaunch/lock race, the CI job proving the wrong triple. 2 recorded,
not code-fixed: the Keychain-ACL prompt after an ad-hoc update (ADR
0036 consequence, watch at the SECOND mac release) and the CI job
duplication (kept: renaming check contexts risks branch protection; a
cross-reference comment binds the pins).

**The E3 net proved itself unstaged**: `quality-macos`'s FIRST run
(33924763712) went red on a real mac-only break every other job missed
— on a macOS target `generate_context!` resolves a `.png` window icon
(fallback `icons/icon.png`, absent; proc-macro panic). Fixed in
`3f9cc14`: make-icon.ps1 also writes `icon.png` (the 512 macOS
rendition), listed in `tauri.macos.conf.json`. The deliberate-break
branch planned for the proof became unnecessary. Two other reds paid
on the way to the push: the language ratchet flagged the new tracked
files (the two-letter Chief-Engineer abbreviation reads as a French
function word, and guillemet quote marks count as accents — reworded
to the full title, straight quotes).

**Stated limits**: a Windows-only release drops the darwin key from
`latest.json` until release-macos.sh reruns — the mac half is part of
EVERY release (ADR 0036, make-release reminder); the mac update path
is only provable at the second mac release.

## 5. Field checklist (STOP 2, on the MacBook Air)

NB: the field REQUIRES the commit pushed (the Air clones from
GitHub) — for this job the push precedes STOP 2, and a field finding
is fixed the same day on main.

1. Follow [MACOS-BUILD.md](MACOS-BUILD.md) top to bottom on the Air —
   every friction in the guide is itself a finding.
2. `cargo tauri dev`: window opens, onboarding runs.
3. Add a real account — Keychain stores the credential; quit,
   relaunch: the account reconnects without re-entering anything.
4. Receive, read, send; click a body link (system browser opens —
   invariant S1); open an attachment.
5. Second launch while running: refused with the message
   (single instance).
6. `cargo tauri build --target x86_64-apple-darwin`: dmg + .app.tar.gz
   + .sig land in the bundle; the built app double-clicks open (no
   quarantine on a local build).
7. Report the figures: cold build minutes, launch feel, anything
   WKWebView renders differently from Windows (fonts, scrollbars,
   theme).

## 6. Chief-Engineer decisions

| # | Question | Answer (date) |
|---|----------|---------------|
| D1 | **Measurement**: About This Mac — chip (Intel/Apple Silicon) and macOS version? | **"Intel, macOS 13 (Ventura) or newer"** (Chief Engineer, 2026-09-04) → target `x86_64-apple-darwin`, toolchain fine, `minimumSystemVersion` set from the exact reading at E2. |
| D2 | Gatekeeper: (a) ad-hoc + documented gesture, free; (b) Apple Developer 99 USD/yr + notarization? | **(a) "Ad-hoc, documented gesture"** (Chief Engineer, 2026-09-04). Notarization logged as debt; reopening condition: Gatekeeper friction reported from the field. |
| D3 | Auto-update on macOS: (a) banner → release page, manual dmg; (b) full updater path now? | **(b) "Full updater now"** (Chief Engineer, 2026-09-04): `.app.tar.gz` + `.sig` assets, darwin key in `latest.json`, in-place replace + relaunch via the updater plugin's standard macOS path. |
| D4 | Mac assets on the same GitHub release (next MINOR), built on the Air? Or hand the first testers a dmg outside the release while it stabilizes? | **"On the GitHub release"** (Chief Engineer, 2026-09-04); `verify-release.ps1` extended to the darwin assets. |
| D5 | Quality bar accepted: no macOS e2e — macOS CI (clippy+tests) + manual field pass on the Air stand as the mac gate? | **"Accepted"** (Chief Engineer, 2026-09-04). e2e-on-mac logged as debt; reopening condition: first mac-only regression found by a tester. |
| D6 | Add the `macos-latest` CI job (E3)? | **"Yes"** (Chief Engineer, 2026-09-04). |
| D7 | Release vehicle: the macOS debut ships in the next MINOR (0.19.0) alongside whatever else lands? | **"Yes, 0.19.0"** (Chief Engineer, 2026-09-04) — win + mac assets on one release, one `latest.json` with all platform keys. |
