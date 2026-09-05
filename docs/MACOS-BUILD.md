# Building Wind on macOS — the MacBook Air, step by step

> PLAN-MACOS E4. Target machine: the **Intel** MacBook Air on macOS 13
> (Ventura) or newer — Chief-Engineer reading of 2026-09-04, decision D1. Target
> triple: `x86_64-apple-darwin`. The paths in this guide are the
> Intel ones (Homebrew under `/usr/local`; an Apple Silicon Mac puts
> it under `/opt/homebrew` and is NOT this guide's machine — D-62).
> Every command is copy-ready, one per block, in Terminal
> (Applications > Utilities > Terminal).

Before anything, confirm the machine is the Intel one (must print
`x86_64`; `arm64` means the wrong Mac for this guide):

```bash
uname -m
```

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

Put node@24 on the PATH (`/usr/local/opt` is the Intel Homebrew
prefix — matches the §0 check):

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

## 6 bis. OAuth credentials (needed to CONNECT accounts)

Field finding 2026-09-05: without these, adding a Google/Microsoft
account fails with "OAuth credentials absent from this binary". A
dev/test build embeds nothing on purpose (the
`dev_builds_embed_no_credentials` net); it reads the three values
from the environment at RUNTIME. They are the same values as the
Windows workstation's `setx` — print them THERE (PowerShell):

```text
[Environment]::GetEnvironmentVariable('GOOGLE_CLIENT_ID','User')
[Environment]::GetEnvironmentVariable('GOOGLE_CLIENT_SECRET','User')
[Environment]::GetEnvironmentVariable('MICROSOFT_CLIENT_ID','User')
```

Then, on the Mac, append them to `~/.zprofile` (fill the values) and
reload:

```bash
cat >> ~/.zprofile << 'EOF'
export GOOGLE_CLIENT_ID="..."
export GOOGLE_CLIENT_SECRET="..."
export MICROSOFT_CLIENT_ID="..."
EOF
```

```bash
source ~/.zprofile
```

Unlike Windows' `setx`, a shell export does NOT reach a double-clicked
app: to test account connection, launch Wind from a Terminal that has
these variables (`cargo tauri dev`, §7). Official release builds embed
the credentials and need none of this (§9's script checks them at
build time for exactly that reason).

## 7. Build the UI, then the app

```bash
cd ~/wind/apps/desktop/ui-v2 && npm ci && npm run build
```

(The Rust build embeds `ui-v2/dist` — the UI build always comes first.)

Dev run (a window opens; Cmd+Q quits):

```bash
cd ~/wind/apps/desktop && cargo tauri dev
```

Release-style build for LOCAL TESTING (field finding 2026-09-05: a
bare `cargo tauri build` refuses with "A public key has been found,
but no private key" — `createUpdaterArtifacts: true` in the config
makes every build want the minisign key to sign the updater tarball.
The key belongs only to the real release, on purpose; a test build
skips the updater artifact instead):

```bash
cd ~/wind/apps/desktop && cargo tauri build --target x86_64-apple-darwin --config '{"bundle":{"createUpdaterArtifacts":false}}'
```

The bundle lands in
`target/x86_64-apple-darwin/release/bundle/` — `macos/Wind.app` and
`dmg/Wind_<version>_x64.dmg` (no `.app.tar.gz`/`.sig`: those are the
updater artifacts, produced only by the signed release build of §9).
An app built ON this machine carries no quarantine flag: it opens
with a double-click, no Gatekeeper gesture.

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

The three OAuth values must be in `~/.zprofile` — already done at
§6 bis (the release script refuses to run without them: the public
build would ship unable to connect).

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
