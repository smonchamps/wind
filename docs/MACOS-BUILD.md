# Building Wind on macOS — the MacBook Air, step by step

> PLAN-MACOS E4. Target machine: the Intel MacBook Air on macOS 13
> (Ventura) or newer — Chief-Engineer reading of 2026-09-04, decision D1. Target
> triple: `x86_64-apple-darwin`. Every command is copy-ready, one per
> block, in Terminal (Applications > Utilities > Terminal).

## 0. What you end up with

- a working dev build (`cargo tauri dev`) for poking at Wind on macOS;
- `Wind_<version>_x64.dmg` + `Wind_<version>_x64.app.tar.gz` + `.sig`,
  the three release assets `scripts/release-macos.sh` uploads
  (PLAN-MACOS D3/D4).

Budget the first run: ~15 min of installs, then the first Rust build
is long on an old Air (30-60 min cold; later builds are incremental).
Keep the Mac plugged in.

## 1. Xcode Command Line Tools (compiler + system SDK)

```bash
xcode-select --install
```

A dialog opens; accept, wait for the install to finish. Verify:

```bash
clang --version
```

## 2. Homebrew (package manager — for gh and node)

```bash
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
```

Follow the "Next steps" the installer prints (on Intel it asks you to
add `/usr/local/bin` to the PATH — usually already there). Verify:

```bash
brew --version
```

## 3. Node 24 and the GitHub CLI

```bash
brew install node@24 gh
```

```bash
echo 'export PATH="/usr/local/opt/node@24/bin:$PATH"' >> ~/.zprofile && source ~/.zprofile
```

Verify (wants v24.x):

```bash
node --version
```

## 4. Rust (rustup — the repo pins 1.97.1 itself)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Accept the default install, then load it into this shell:

```bash
source "$HOME/.cargo/env"
```

`rust-toolchain.toml` makes every cargo command in the repo install and
use 1.97.1 automatically — nothing to pick by hand.

## 5. The Tauri CLI

```bash
cargo install tauri-cli --version "^2" --locked
```

(~10 min: it compiles. Once per machine.)

## 6. Clone and identify

```bash
git clone https://github.com/smonchamps/wind.git ~/wind && cd ~/wind
```

The local identity, as on every fresh clone (GitHub noreply — never
the personal address, the repo is public):

```bash
git config user.name "smonchamps" && git config user.email "smonchamps@users.noreply.github.com"
```

## 7. Build the UI, then the app

```bash
cd ~/wind/apps/desktop/ui-v2 && npm ci && npm run build
```

(The Rust build embeds `ui-v2/dist` — the UI build always comes first.)

Dev run (a window opens; Cmd+Q quits):

```bash
cd ~/wind/apps/desktop && cargo tauri dev
```

Release-style build (unsigned, for local testing):

```bash
cd ~/wind/apps/desktop && cargo tauri build --target x86_64-apple-darwin
```

The bundle lands in
`target/x86_64-apple-darwin/release/bundle/` — `macos/Wind.app` and
`dmg/Wind_<version>_x64.dmg`. An app built ON this machine carries no
quarantine flag: it opens with a double-click, no Gatekeeper gesture.

## 8. First-launch gesture for DOWNLOADED builds (what testers do)

A dmg downloaded from GitHub is quarantined and Wind is not notarized
(PLAN-MACOS D2: ad-hoc for the beta). The gesture, once per version:

1. Open the dmg, drag Wind to Applications.
2. Double-click Wind; macOS refuses. System Settings > Privacy &
   Security, scroll to *"Wind" was blocked*, click **Open Anyway**,
   launch again. (On macOS 14 and older, right-click > Open > Open is
   a shortcut; macOS 15 removed it.)

Terminal alternative (same effect):

```bash
xattr -d com.apple.quarantine /Applications/Wind.app
```

## 9. Releasing the macOS assets (after the Windows release)

One-time setup: authenticate gh, copy the signing key, set the OAuth
credentials.

```bash
gh auth login
```

Copy `C:\Keys\wind.key` from the Windows workstation to `~/Keys/wind.key`
on the Mac — by USB stick or another private channel, never mail in
clear text, never into the repository.

Append the three OAuth values (same values as the Windows
workstation's `setx`) to `~/.zprofile`, then reload:

```bash
source ~/.zprofile
```

```text
export GOOGLE_CLIENT_ID="..."
export GOOGLE_CLIENT_SECRET="..."
export MICROSOFT_CLIENT_ID="..."
```

Per release — pull the release commit that make-release.ps1 pushed,
then run the mac half (order is the invariant: **Windows first, mac
second** — the manifest never points at an absent asset). The mac
half is part of **every** release, even a Windows-motivated hotfix:
until it runs, the fresh `latest.json` has no darwin key and mac
clients find no update (ADR 0036):

```bash
cd ~/wind && git pull && ./scripts/release-macos.sh <version>
```

Tauri asks for the key password at the build. Then, from the Windows
workstation, `powershell scripts\verify-release.ps1 <version>` — it
now expects 8 assets and 3 platform keys.

## Known limits (stated, PLAN-MACOS §2)

- No e2e suite on macOS (WKWebView has no CDP) — the mac gate is the
  `quality-macos` CI job + the field checklist (D5).
- One arch only (`x86_64`); Apple Silicon joins when a tester needs it.
- Unsigned/ad-hoc: the §8 gesture is the price until notarization (D2
  debt).
