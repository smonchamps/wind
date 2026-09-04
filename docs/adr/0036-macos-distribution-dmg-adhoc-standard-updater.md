# 0036 — macOS distribution: dmg, ad-hoc signature, the plugin's standard updater

- **Status**: accepted (Chief-Engineer decisions D2/D3/D4, 2026-09-04, PLAN-MACOS)
- **Context**: the beta needs macOS testers; the build machine is an
  Intel MacBook Air on macOS 13 (D1). ADR 0013's whole rationale
  (NSIS over MSIX, our own installer launch with `/P /R /UPDATE`) is
  Windows-shaped and transfers nothing.

## Decision

1. **Artifacts**: `Wind_<v>_x64.dmg` for first installs,
   `Wind_<v>_x64.app.tar.gz` + minisign `.sig` for the updater —
   Tauri's own macOS artifact shapes, one triple only
   (`x86_64-apple-darwin`).
2. **Gatekeeper**: no Apple Developer account for the beta (D2) —
   the app is ad-hoc signed by the bundler; the tester's gesture
   (right-click Open / "Open Anyway") is documented in BETA.md. The
   99 USD/yr notarization is debt D-60.
3. **Updates**: the standard `tauri-plugin-updater` install path (D3)
   — unlike Windows, where the launch is ours (the plugin swallowed
   Smart App Control refusals, ADR 0013). No macOS equivalent of that
   failure mode is known; if the field finds one, the same
   take-over lever exists (the plugin stays pinned `=2.10.1`).
4. **Release order** (D4): Windows first (`make-release.ps1` publishes
   the release and a 2-key `latest.json`), the Air second
   (`release-macos.sh` uploads the 3 mac assets, then re-uploads
   `latest.json` with the `darwin-x86_64` key). The manifest never
   names an asset that is not uploaded — the 404 trap of ADR 0013,
   generalized.
5. **Quality bar** (D5): no macOS e2e — WKWebView exposes no CDP, the
   Playwright harness cannot attach. The mac gate is the
   `quality-macos` CI job (clippy + full unit tests) plus the manual
   field checklist. Porting e2e is debt D-61.

## Consequences

- One minisign keypair serves all channels; the private key lives on
  BOTH build machines, outside the repositories.
- The anti-crossing guard becomes N-way (verify-release: every
  channel signature pairwise distinct), and the `darwin-x86_64` key
  and the mac assets are verified to stand or fall together.
- The mac update path is only field-proven at the SECOND mac release
  (the first proves install) — stated at the GO.
- **The release order has a stated gap**: every `make-release.ps1`
  run regenerates `latest.json` with the Windows keys only — darwin
  clients find no update (their check errors on the missing key)
  until `release-macos.sh` runs on the Mac. The mac half is part of
  EVERY release, even a Windows-motivated hotfix; make-release prints
  the reminder.
- **Ad-hoc signing pins the CDHash, which changes every build**: the
  macOS Keychain ACL binds stored secrets to the writing app's
  signature, so after an auto-update the system may prompt ("Wind
  wants to use your confidential information") once per stored
  credential — "Always Allow" re-binds. A friction cost of D2,
  recorded under D-60; Developer ID signing (stable Team ID) is what
  removes it. To watch at the second mac release's field pass.
