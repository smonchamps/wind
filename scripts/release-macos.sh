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
# (3) one signed build PER TRIPLE -- x86_64-apple-darwin (the Air's
# own) and aarch64-apple-darwin (cross-built on the same Intel Air,
# PLAN-APPLE-SILICON D1) -- dmg for first installs, app.tar.gz +
# minisign sig for the updater; (4) uploads the 6 assets under
# VERSIONED, arch-named names; (5) patches latest.json: adds the
# darwin-x86_64 and darwin-aarch64 keys, re-uploads with --clobber.
#
# The minisign key is THE SAME as Windows' (one pubkey in
# tauri.conf.json): copy C:\Keys\wind.key to the Mac OUTSIDE the
# repository (~/Keys/wind.key) -- never commit it, never mail it in
# clear text. Tauri asks for its password at the build.

set -euo pipefail

VERSION="${1:?usage: release-macos.sh <version>}"
REPO="smonchamps/wind"
# Two triples from ONE Intel machine (PLAN-APPLE-SILICON D1): the
# asset word and the manifest key word per triple. x64 is the
# PLAN-MACOS name kept; aarch64 is Tauri's bundler word (D3).
# Functions, not `declare -A`: /bin/bash on macOS is 3.2, which has
# no associative arrays -- the script would die at release day.
TRIPLES=(x86_64-apple-darwin aarch64-apple-darwin)
arch_of() { case "$1" in x86_64-*) echo x64 ;; aarch64-*) echo aarch64 ;; *) echo "unknown triple $1" >&2; exit 1 ;; esac; }
key_of() { echo "darwin-${1%%-*}"; }
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

export TAURI_SIGNING_PRIVATE_KEY="${TAURI_SIGNING_PRIVATE_KEY:-$HOME/Keys/wind.key}"

# (1) Fail fast and loud, before the long build.
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "Version '$VERSION' invalid -- MAJOR.MINOR.PATCH, without 'v'." >&2; exit 1; }
git diff --quiet && git diff --cached --quiet || { echo "Working tree not clean -- check out the release tag, nothing else." >&2; exit 1; }
command -v gh >/dev/null || { echo "gh (GitHub CLI) not found -- brew install gh, then gh auth login." >&2; exit 1; }
command -v node >/dev/null || { echo "node not found -- needed for the ui-v2 build and the manifest patch." >&2; exit 1; }
for TRIPLE in "${TRIPLES[@]}"; do
  rustup target list --installed | grep -qx "$TRIPLE" || { echo "Rust target $TRIPLE not installed -- rustup target add $TRIPLE (MACOS-BUILD.md 4)." >&2; exit 1; }
done
cargo tauri --version >/dev/null 2>&1 || { echo "cargo tauri not found -- cargo install tauri-cli --version '^2' --locked (MACOS-BUILD.md 5)." >&2; exit 1; }
[[ -d apps/desktop/ui-v2/node_modules ]] || { echo "ui-v2/node_modules absent -- run 'npm ci' in apps/desktop/ui-v2 first (MACOS-BUILD.md 7)." >&2; exit 1; }
[[ -f "$TAURI_SIGNING_PRIVATE_KEY" ]] || { echo "Signing key not found at $TAURI_SIGNING_PRIVATE_KEY (copy C:\\Keys\\wind.key there)." >&2; exit 1; }
CONF_VERSION="$(node -e "console.log(require('./apps/desktop/tauri.conf.json').version)")"
[[ "$CONF_VERSION" == "$VERSION" ]] || { echo "tauri.conf.json says $CONF_VERSION, not $VERSION -- pull the Windows release commit first (the order is the invariant)." >&2; exit 1; }
# Auth first, then the release: an unauthenticated gh made the next
# line say "no release" while 0.19.0 was published (field 2026-09-06).
gh auth status >/dev/null 2>&1 || { echo "gh is not authenticated on this Mac -- gh auth login (GitHub.com, HTTPS, web browser), then rerun." >&2; exit 1; }
# The DRAFT staged by make-release.ps1 (lot 4, D1): `gh release view`
# reads drafts, the tags API does not. HEAD must BE the tagged release
# commit (E12a / B29): a clean tree was never enough.
TARGET="$(gh release view "$VERSION" --repo "$REPO" --json targetCommitish,isDraft --jq '.targetCommitish' 2>/dev/null || true)"
[[ -n "$TARGET" ]] || { echo "No GitHub release (draft or not) at tag $VERSION -- make-release.ps1 (Windows) stages the draft FIRST." >&2; exit 1; }
TAG_COMMIT="$(gh api "repos/$REPO/git/ref/tags/$VERSION" --jq '.object.sha' 2>/dev/null || true)"
[[ -n "$TAG_COMMIT" ]] || { echo "Tag $VERSION absent from GitHub -- make-release.ps1 pushes it with the draft." >&2; exit 1; }
HEAD_COMMIT="$(git rev-parse HEAD)"
[[ "$HEAD_COMMIT" == "$TAG_COMMIT" ]] || { echo "HEAD $HEAD_COMMIT is not the release commit $TAG_COMMIT (tag $VERSION) -- git fetch --tags && git checkout $VERSION, then rerun." >&2; exit 1; }
[[ "$TARGET" == "$TAG_COMMIT" ]] || { echo "The draft targets $TARGET but the tag points at $TAG_COMMIT." >&2; exit 1; }
# The identity is the TAG's commit, checked out detached (the documented
# path); "from main" means that commit is on main's history, not that a
# branch is checked out (review 2026-09-07: a detached HEAD has no
# current branch, and main may have moved since the Windows half).
git fetch -q origin main
git merge-base --is-ancestor "$TAG_COMMIT" origin/main || { echo "The release commit $TAG_COMMIT is not on origin/main -- a release is made from main." >&2; exit 1; }
BRANCH="main"

# A mounted "Wind" image (a dmg opened to install/test) or a leftover
# "dmg.*" temp volume makes bundle_dmg.sh fail AFTER the build (field
# 2026-09-06, 5 min lost): refuse before building.
for VOL in /Volumes/Wind /Volumes/dmg.*; do
  [[ -d "$VOL" ]] && { echo "Volume $VOL is mounted -- hdiutil detach \"$VOL\" -force, then rerun (bundle_dmg.sh cannot build the dmg over it)." >&2; exit 1; }
done

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

# (2) The release dist, clean of the e2e seams: built and asserted by
# `cargo tauri build` itself through tauri.conf.json's beforeBuildCommand
# (scripts/build-dist-clean.mjs, lot 4 E12d) -- one declaration, no copy.

# (3) One signed build PER TRIPLE (PLAN-APPLE-SILICON D1, 2026-09-05:
# the Intel Air cross-builds arm64 -- Xcode's SDK is fat, `cc` passes
# -arch arm64). BOTH builds complete before anything is uploaded: a
# release with one mac family and not the other is a broken release
# (verify-release checks the two families and their keys together),
# so a failed second build leaves the GitHub release untouched.
# Tauri asks for the key password at each build (two prompts).
OUT="$ROOT/target/upload-macos-$VERSION"
rm -rf "$OUT" && mkdir -p "$OUT"
ASSETS=()
for TRIPLE in "${TRIPLES[@]}"; do
  ARCH="$(arch_of "$TRIPLE")"
  ( cd apps/desktop && cargo tauri build --target "$TRIPLE" )

  BUNDLE="$ROOT/target/$TRIPLE/release/bundle"
  # The glob PINS the version: the dmg directory accumulates one file
  # per version across builds -- an unpinned `ls | head -1` would ship
  # the alphabetically-first (OLD) dmg under the new name (review
  # 2026-09-04). The tar is unversioned by the bundler (overwritten
  # each build), no pin possible there. The arch needs no pin: the
  # bundle directory is per triple.
  DMG_SRC="$(ls "$BUNDLE/dmg/"*"${VERSION}"*.dmg 2>/dev/null | head -1 || true)"
  TAR_SRC="$(ls "$BUNDLE/macos/"*.app.tar.gz 2>/dev/null | head -1 || true)"
  [[ -n "$DMG_SRC" && -n "$TAR_SRC" && -f "$TAR_SRC.sig" ]] || { echo "Bundle incomplete under $BUNDLE (dmg / app.tar.gz / sig) -- nothing is published." >&2; exit 1; }

  # Versioned, arch-named assets (the bundler's names are not): the
  # release holds several versions' history side by side. The arch
  # word is the manifest key's (x64 kept from PLAN-MACOS, aarch64 =
  # Tauri's bundler word, D3).
  DMG="$OUT/Wind_${VERSION}_${ARCH}.dmg"
  TAR="$OUT/Wind_${VERSION}_${ARCH}.app.tar.gz"
  cp "$DMG_SRC" "$DMG"
  cp "$TAR_SRC" "$TAR"
  cp "$TAR_SRC.sig" "$TAR.sig"
  # Content, not size (-s): a whitespace-only .sig must fail HERE,
  # before the upload -- patch-manifest would catch it, one step late.
  [[ -n "$(cat "$TAR.sig")" ]] || { echo "Empty signature in $TAR.sig -- the updater would refuse the package." >&2; exit 1; }
  ASSETS+=("$DMG" "$TAR" "$TAR.sig")
done

# The builds changed nothing in the tree (E12a).
git diff --quiet && git diff --cached --quiet || { echo "The builds modified the tree -- the binaries would not match the release commit." >&2; exit 1; }
# The attestation (E12a): the same shape as Windows', checked by
# publish-release.ps1 against the tag and the uploaded bytes.
ATTESTATION="$OUT/attestation-macos.json"
node "$ROOT/scripts/release-lib.mjs" attest "$ATTESTATION" macos "$VERSION" "$HEAD_COMMIT" "$BRANCH" "$ROOT/Cargo.lock" "$ROOT/apps/desktop/ui-v2/dist" "${ASSETS[@]}"
ASSETS+=("$ATTESTATION")

# (4) Upload, all seven at once. --clobber: a rerun after a partial
# failure re-uploads.
gh release upload "$VERSION" "${ASSETS[@]}" --repo "$REPO" --clobber

# (5) latest.json: download the published one, ADD one darwin key per
# triple, re-upload ONCE. The Windows keys are never touched (trap 3
# of make-release.ps1: a crossed signature produces NO error, only a
# silent channel); patch-manifest refuses a signature already present
# under another key -- the second call is where two identical mac
# signatures would be caught.
MANIFEST="$OUT/latest.json"
gh release download "$VERSION" --repo "$REPO" --pattern latest.json --dir "$OUT" --clobber
for TRIPLE in "${TRIPLES[@]}"; do
  ARCH="$(arch_of "$TRIPLE")"
  node "$ROOT/scripts/patch-manifest.mjs" "$MANIFEST" "$VERSION" "$OUT/Wind_${VERSION}_${ARCH}.app.tar.gz.sig" \
    "https://github.com/$REPO/releases/download/$VERSION/Wind_${VERSION}_${ARCH}.app.tar.gz" \
    "$(key_of "$TRIPLE")"
done
gh release upload "$VERSION" "$MANIFEST" --repo "$REPO" --clobber

echo ""
echo "macOS half of $VERSION staged on the draft (${#ASSETS[@]} files, ${TRIPLES[*]});"
echo "${#TRIPLES[@]} darwin keys added to latest.json. Nothing is public yet (D1)."
echo "From the Windows workstation: powershell scripts\publish-release.ps1 $VERSION"
echo "(proves 11 assets, 4 keys, 4 signatures, both attestations, then Latest),"
echo "then scripts\verify-release.ps1 $VERSION. The field proof: install the dmg,"
echo "then observe the n-1 -> n auto-update at the NEXT release."
