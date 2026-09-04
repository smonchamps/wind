#!/bin/bash
# release-macos.sh -- the macOS half of a Wind release (PLAN-MACOS
# D3/D4/D7). Runs ON THE MAC, AFTER make-release.ps1 has published the
# Windows release (the GitHub Release and its latest.json must exist:
# this script only ADDS to them, so the manifest never points at an
# asset that is not uploaded yet -- the order is the invariant).
#
#   ./scripts/release-macos.sh 0.19.0
#
# Does, in order: (1) checks -- main, clean tree at the release commit,
# gh authenticated, signing key, OAuth credentials, the version already
# bumped by the Windows release; (2) ui-v2 build CLEAN of e2e seams
# (VITE_E2E=0 + __e2e absence assert, the make-release.ps1 poka-yoke);
# (3) one signed build, x86_64-apple-darwin (D1: the MacBook Air is
# Intel) -- dmg for first installs, app.tar.gz + minisign sig for the
# updater; (4) uploads the 3 assets under VERSIONED names; (5) patches
# latest.json: adds the darwin-x86_64 key, re-uploads with --clobber.
#
# The minisign key is THE SAME as Windows' (one pubkey in
# tauri.conf.json): copy C:\Keys\wind.key to the Mac OUTSIDE the
# repository (~/Keys/wind.key) -- never commit it, never mail it in
# clear text. Tauri asks for its password at the build.

set -euo pipefail

VERSION="${1:?usage: release-macos.sh <version>}"
REPO="smonchamps/wind"
TRIPLE="x86_64-apple-darwin"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export TAURI_SIGNING_PRIVATE_KEY="${TAURI_SIGNING_PRIVATE_KEY:-$HOME/Keys/wind.key}"

# (1) Fail fast and loud, before the long build.
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "Version '$VERSION' invalid -- MAJOR.MINOR.PATCH, without 'v'." >&2; exit 1; }
BRANCH="$(git branch --show-current)"
[[ "$BRANCH" == "main" ]] || { echo "Current branch '$BRANCH': a release is made from main." >&2; exit 1; }
git diff --quiet && git diff --cached --quiet || { echo "Working tree not clean -- pull the release commit, nothing else." >&2; exit 1; }
command -v gh >/dev/null || { echo "gh (GitHub CLI) not found -- brew install gh, then gh auth login." >&2; exit 1; }
command -v node >/dev/null || { echo "node not found -- needed for the ui-v2 build and the manifest patch." >&2; exit 1; }
cargo tauri --version >/dev/null 2>&1 || { echo "cargo tauri not found -- cargo install tauri-cli --version '^2' --locked (MACOS-BUILD.md 5)." >&2; exit 1; }
[[ -d apps/desktop/ui-v2/node_modules ]] || { echo "ui-v2/node_modules absent -- run 'npm ci' in apps/desktop/ui-v2 first (MACOS-BUILD.md 7)." >&2; exit 1; }
[[ -f "$TAURI_SIGNING_PRIVATE_KEY" ]] || { echo "Signing key not found at $TAURI_SIGNING_PRIVATE_KEY (copy C:\\Keys\\wind.key there)." >&2; exit 1; }
CONF_VERSION="$(node -e "console.log(require('./apps/desktop/tauri.conf.json').version)")"
[[ "$CONF_VERSION" == "$VERSION" ]] || { echo "tauri.conf.json says $CONF_VERSION, not $VERSION -- pull the Windows release commit first (the order is the invariant)." >&2; exit 1; }
gh api "repos/$REPO/releases/tags/$VERSION" >/dev/null 2>&1 || { echo "No GitHub release at tag $VERSION -- make-release.ps1 (Windows) publishes FIRST." >&2; exit 1; }

# OAuth credentials embedded at build time (D1, PLAN-RETOURS-9) -- the
# same three as make-release.ps1; a missing one stops the release (the
# public build would ship unable to connect). Set them in ~/.zshrc or
# inline for the run.
for VAR in GOOGLE_CLIENT_ID GOOGLE_CLIENT_SECRET MICROSOFT_CLIENT_ID; do
  [[ -n "${!VAR:-}" ]] || { echo "$VAR absent from the environment -- the release would embed a binary unable to connect." >&2; exit 1; }
done
export WIND_RELEASE_GOOGLE_CLIENT_ID="$GOOGLE_CLIENT_ID"
export WIND_RELEASE_GOOGLE_CLIENT_SECRET="$GOOGLE_CLIENT_SECRET"
export WIND_RELEASE_MICROSOFT_CLIENT_ID="$MICROSOFT_CLIENT_ID"
# The WIND_RELEASE_* die with this process: nothing to clean up, no
# poisoned later dev build (the make-release.ps1 finally, for free).

# (2) The release dist, clean of the e2e seams (PLAN-AUDIT-V3 E7) --
# the guard is ONE file shared with make-release.ps1, never two
# copies that drift.
( cd apps/desktop/ui-v2 && VITE_E2E=0 npm run build )
node scripts/assert-dist-clean.mjs

# (3) The signed build. Tauri asks for the key password here.
( cd apps/desktop && cargo tauri build --target "$TRIPLE" )

BUNDLE="$ROOT/target/$TRIPLE/release/bundle"
# The glob PINS the version: the dmg directory accumulates one file
# per version across builds -- an unpinned `ls | head -1` would ship
# the alphabetically-first (OLD) dmg under the new name (review
# 2026-09-04). The tar is unversioned by the bundler (overwritten
# each build), no pin possible there.
DMG_SRC="$(ls "$BUNDLE/dmg/"*"${VERSION}"*.dmg 2>/dev/null | head -1 || true)"
TAR_SRC="$(ls "$BUNDLE/macos/"*.app.tar.gz 2>/dev/null | head -1 || true)"
[[ -n "$DMG_SRC" && -n "$TAR_SRC" && -f "$TAR_SRC.sig" ]] || { echo "Bundle incomplete under $BUNDLE (dmg / app.tar.gz / sig) -- nothing is published." >&2; exit 1; }

# Versioned, arch-named assets (the bundler's names are not): the
# release holds several versions' history side by side.
OUT="$ROOT/target/$TRIPLE/release/bundle/upload"
mkdir -p "$OUT"
DMG="$OUT/Wind_${VERSION}_x64.dmg"
TAR="$OUT/Wind_${VERSION}_x64.app.tar.gz"
cp "$DMG_SRC" "$DMG"
cp "$TAR_SRC" "$TAR"
cp "$TAR_SRC.sig" "$TAR.sig"
SIGNATURE="$(cat "$TAR.sig")"
[[ -n "$SIGNATURE" ]] || { echo "Empty signature in $TAR.sig -- the updater would refuse the package." >&2; exit 1; }

# (4) Upload. --clobber: a rerun after a partial failure re-uploads.
gh release upload "$VERSION" "$DMG" "$TAR" "$TAR.sig" --repo "$REPO" --clobber

# (5) latest.json: download the published one, ADD the darwin key,
# re-upload. The Windows keys are never touched (trap 3 of
# make-release.ps1: a crossed signature produces NO error, only a
# silent channel).
MANIFEST="$OUT/latest.json"
gh release download "$VERSION" --repo "$REPO" --pattern latest.json --dir "$OUT" --clobber
node "$ROOT/scripts/patch-manifest.mjs" "$MANIFEST" "$VERSION" "$TAR.sig" \
  "https://github.com/$REPO/releases/download/$VERSION/Wind_${VERSION}_x64.app.tar.gz"
gh release upload "$VERSION" "$MANIFEST" --repo "$REPO" --clobber

echo ""
echo "macOS assets of $VERSION published; darwin-x86_64 added to latest.json."
echo "Verify from the Windows workstation: powershell scripts\\verify-release.ps1 $VERSION"
echo "(now checks 8 assets and 3 platform keys). The field proof: install the"
echo "dmg, then observe the n-1 -> n auto-update at the NEXT release."
