# make-release.ps1 -- signed BI-ARCH build + latest.json of a version
# (ADR 0013, bi-arch since PLAN-RETOURS-8). ASCII only (PowerShell 5.1
# encoding trap: the gate parses every .ps1).
#
#   powershell scripts\make-release.ps1 0.6.0
#
# Does the WHOLE release, in order: (1) checks version + CHANGELOG entry,
# BUMPS the single line of tauri.conf.json; (2) TWO signed builds --
# native arm64 then x64 as a cross-build (D6), key defined below,
# password asked by Tauri AT EACH build (twice -- assumed: it NEVER goes
# through a variable, a build's environment is inherited by all its
# child processes, review 2026-08-22); ALL-OR-NOTHING (D7): a failed
# build stops everything, nothing is published; (3) latest.json manifest
# with TWO platform keys (one manifest serves both channels: the updater
# reads the {os}-{arch} key of ITS binary); (4) AFTER an explicit
# CONFIRMATION (the Latest auto-update is irreversible): release commit,
# push (the pre-push gate replays), BARE VERSION tag + GitHub Release
# with the 5 assets, marked Latest, notes from the CHANGELOG.
#
# Traps paid and encoded here (never left to vigilance):
#   1. a UTF-8 BOM that the updater (serde_json) silently refuses;
#   2. a URL pointing at `v<version>` while the tag is the BARE VERSION
#      -- hence a "404 Not Found" at download;
#   3. (bi-arch) a missing platform key or CROSSED signatures produce NO
#      error -- the updater of the silent channel concludes "no update".
#      The manifest is therefore built per platform from the directory
#      of ITS target, and the two signatures are required distinct.

param([Parameter(Mandatory = $true)][string]$Version)

$ErrorActionPreference = "Stop"

# Updater signing key (ADR 0013): the file lives outside the repository,
# at C:\Keys\wind.key. `cargo tauri build` reads it from this variable.
$env:TAURI_SIGNING_PRIVATE_KEY = "C:\Keys\wind.key"
$repo = "smonchamps/wind"

# The TWO channels (D5/D6, PLAN-RETOURS-8): native arm64 (the workstation),
# x64 as a local cross-build (MSVC x64 toolset + rustup target, proven at
# E1). With `--target`, Tauri writes under target/<triple>/release/bundle/nsis
# -- the path without triple no longer exists in this script.
$targets = @(
    [ordered]@{
        triple   = "aarch64-pc-windows-msvc"
        platform = "windows-aarch64"
        exeName  = "Wind_${Version}_arm64-setup.exe"
    },
    [ordered]@{
        triple   = "x86_64-pc-windows-msvc"
        platform = "windows-x86_64"
        exeName  = "Wind_${Version}_x64-setup.exe"
    }
)
foreach ($t in $targets) {
    $t.nsis = Join-Path $PSScriptRoot "..\target\$($t.triple)\release\bundle\nsis"
    $t.exe = Join-Path $t.nsis $t.exeName
    $t.sig = "$($t.exe).sig"
}

# (1) Preparation, BEFORE the long builds (fail fast and loud): well-formed
# version, user notes written, then bump of tauri.conf.json.
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "Version '$Version' invalid -- expected MAJOR.MINOR.PATCH (e.g. 0.1.10), without 'v'."
}
# The release is tagged and pushed from `main` ONLY (audit 2026-09-01):
# the script pushes the CURRENT branch and the Latest tag targets its
# commit; from a working branch, the (irreversible) auto-update would
# ship a commit outside main. Refuse before the bump.
$branch = (git branch --show-current).Trim()
if ($branch -ne 'main') {
    throw "Current branch '$branch': a release is made from main."
}
# The final publication goes through gh: refuse early, not after 8 min of build.
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw "gh (GitHub CLI) not found -- install it (winget install GitHub.cli) and 'gh auth login', or publish the Release by hand."
}
# The x64 target must be installed (rustup): fail before the build.
$installedTargets = rustup target list --installed
if ($installedTargets -notcontains "x86_64-pc-windows-msvc") {
    throw "rustup target x86_64-pc-windows-msvc absent -- 'rustup target add x86_64-pc-windows-msvc' (PLAN-RETOURS-8 E1)."
}
# OAuth credentials EMBEDDED at build time (D1, PLAN-RETOURS-9): the
# public release connects without any user setx. The values come from
# the maintainer workstation's environment (the same as the setx of
# install-workstation.ps1) and are mapped to WIND_RELEASE_* names that
# ONLY this script sets -- a dev/test build therefore never embeds
# anything (the e2e isolation keeps its lever, and the
# dev_builds_embed_no_credentials test shouts otherwise). ALL-OR-NOTHING
# (D7): a missing value stops the release before the builds.
$oauth = @(
    @{ target = "WIND_RELEASE_GOOGLE_CLIENT_ID";     source = "GOOGLE_CLIENT_ID" },
    @{ target = "WIND_RELEASE_GOOGLE_CLIENT_SECRET"; source = "GOOGLE_CLIENT_SECRET" },
    @{ target = "WIND_RELEASE_MICROSOFT_CLIENT_ID";  source = "MICROSOFT_CLIENT_ID" }
)
# NB: the table above duplicates the option_env! of
# crates/mail-auth/src/provider.rs -- a provider ADDED there is added
# HERE, otherwise its release ships without a credential (the
# all-or-nothing only checks its own list).
foreach ($o in $oauth) {
    # Process scope first, fallback on the User scope (the run-wind.ps1
    # pattern): a shell opened BEFORE the setx must not fail the release
    # wrongly (field 2026-08-23).
    $o.value = [Environment]::GetEnvironmentVariable($o.source)
    if ([string]::IsNullOrWhiteSpace($o.value)) {
        $o.value = [Environment]::GetEnvironmentVariable($o.source, "User")
    }
    if ([string]::IsNullOrWhiteSpace($o.value)) {
        throw "$($o.source) absent from the workstation (process AND user scopes) -- the release would embed a binary unable to connect (D1, PLAN-RETOURS-9). Remedy: setx $($o.source) `"<value>`" then rerun (no new shell needed, the User scope is read)."
    }
}
Write-Host "OAuth credentials present on the workstation (3/3) -- set for the duration of the builds only."

$changelog = Join-Path $PSScriptRoot "..\CHANGELOG.md"
if ((Get-Content -Raw -Encoding UTF8 $changelog) -notmatch [regex]::Escape("## [$Version]")) {
    throw "CHANGELOG.md has no '## [$Version]' entry -- write the user notes first."
}
# Bump of the SINGLE version line (targeted regex: the rest of the file,
# its formatting and key order, does not move; never a BOM that the
# updater refuses). Exactly one 'version' key is required.
$conf = Join-Path $PSScriptRoot "..\apps\desktop\tauri.conf.json"
$json = Get-Content -Raw -Encoding UTF8 $conf
$pattern = '("version"\s*:\s*")[^"]*(")'
if (([regex]::Matches($json, $pattern)).Count -ne 1) {
    throw "tauri.conf.json: 'version' key not found or multiple -- automatic bump refused, do it by hand."
}
# The Cargo workspace version FOLLOWS the product version (PLAN-RETOURS-12
# R4, decision D3): one number everywhere -- the crates state the version
# of the app that embeds them. Regex anchored on the [workspace.package]
# section: the `version = "..."` of the dependencies never move.
# Cargo.lock updates itself at the builds below and goes into the release
# commit. BOTH validations pass BEFORE the first write (review): a throw
# never leaves the tree half bumped.
$cargoToml = Join-Path $PSScriptRoot "..\Cargo.toml"
$toml = Get-Content -Raw -Encoding UTF8 $cargoToml
$patternCargo = '(?ms)(\[workspace\.package\][^\[]*?^version\s*=\s*")[^"]*(")'
if (([regex]::Matches($toml, $patternCargo)).Count -ne 1) {
    throw "Cargo.toml: [workspace.package] version not found or multiple -- automatic bump refused, do it by hand."
}

$json = [regex]::Replace($json, $pattern, "`${1}$Version`${2}")
[System.IO.File]::WriteAllText($conf, $json, (New-Object System.Text.UTF8Encoding $false))
Write-Host "tauri.conf.json bumped to $Version."
$toml = [regex]::Replace($toml, $patternCargo, "`${1}$Version`${2}")
[System.IO.File]::WriteAllText($cargoToml, $toml, (New-Object System.Text.UTF8Encoding $false))
Write-Host "Cargo.toml (workspace.package) bumped to $Version."

# (2) The TWO signed builds, arm64 then x64. ALL-OR-NOTHING (D7): the
# first failure throws, nothing is published, never a channel out of
# step. The PASSWORD is deliberately NOT set as a variable (ADR 0013
# invariant kept): Tauri asks for it by hand at EACH build -- two
# entries, the price for it never appearing in an environment inherited
# by the build's child processes.
$desktop = Join-Path $PSScriptRoot "..\apps\desktop"

# The release dist is built CLEAN of the e2e seams (PLAN-AUDIT-V3 E7,
# D-52 item 8): `cargo tauri build` embeds WHATEVER dist sits on disk,
# and a gate or an e2e run leaves a seam-flavored one behind. Rebuild
# without the flag, then ASSERT no `__e2e` survives in the bundle --
# the poka-yoke, not the build, is what makes the release deterministic.
Push-Location (Join-Path $desktop "ui-v2")
$env:VITE_E2E = "0"
try {
    & npm run build
    if ($LASTEXITCODE -ne 0) { throw "vite build failed -- release interrupted, NOTHING is published." }
} finally {
    Remove-Item Env:VITE_E2E -ErrorAction SilentlyContinue
    Pop-Location
}
$assets = Get-ChildItem (Join-Path $desktop "ui-v2\dist") -Recurse -File |
    Where-Object { $_.Extension -in ".js", ".mjs", ".css", ".html", ".map" }
$leaks = $assets | Where-Object { Select-String -Path $_.FullName -Pattern "__e2e" -Quiet }
if ($leaks) {
    throw "e2e seams found in the release bundle ($($leaks.Name -join ', ')) -- release interrupted, NOTHING is published."
}
Write-Host "dist rebuilt clean: no __e2e in the bundle"

Push-Location $desktop
# The WIND_RELEASE_* live only for the TWO builds, and the finally removes
# them even on failure or interruption: left in the environment, they
# would poison the pre-push of the final git push (cargo test recompiles
# mail-auth with the values, the dev_builds_embed_no_credentials test
# turns red and the release blocks itself) and any later dev build of
# the same shell (review 2026-08-23).
foreach ($o in $oauth) {
    Set-Item -Path "Env:$($o.target)" -Value $o.value
}
try {
    foreach ($t in $targets) {
        Write-Host ""
        Write-Host "=== Build $($t.triple) ==="
        cargo tauri build --target $t.triple
        if ($LASTEXITCODE -ne 0) {
            throw "cargo tauri build --target $($t.triple) failed (code $LASTEXITCODE) -- release interrupted, NOTHING is published (D7)."
        }
    }
}
finally {
    foreach ($o in $oauth) {
        Remove-Item -Path "Env:$($o.target)" -ErrorAction SilentlyContinue
    }
    Pop-Location
}

# Presence check PER CHANNEL: both exe and both signatures.
foreach ($t in $targets) {
    foreach ($f in @($t.exe, $t.sig)) {
        if (-not (Test-Path $f)) {
            throw "Not found after the build: $f`nDoes version '$Version' match the one in tauri.conf.json?"
        }
    }
    $t.signature = (Get-Content -Raw $t.sig).Trim()
    if ([string]::IsNullOrWhiteSpace($t.signature)) {
        throw "Empty signature in $($t.sig) -- the updater would refuse the package."
    }
}
# Anti-crossing guard (trap 3): two identical signatures sign an
# accidental copy -- one channel would serve the other's binary. Pairwise
# distinct, whatever the number of targets (review 2026-08-22: never a
# hard-coded index that would miss a 3rd target).
$uniqueSignatures = @($targets | ForEach-Object { $_.signature } | Select-Object -Unique)
if ($uniqueSignatures.Count -ne $targets.Count) {
    throw "Signatures of different targets are IDENTICAL -- crossing or accidental copy, release interrupted."
}

# (3) latest.json manifest (no BOM, URL at the BARE tag, ONE key per
# channel -- each entry is built from the directory of ITS target).
$platforms = [ordered]@{}
foreach ($t in $targets) {
    $platforms[$t.platform] = [ordered]@{
        signature = $t.signature
        # Tag = BARE VERSION, never `v$Version`: that is the 404 trap.
        url       = "https://github.com/$repo/releases/download/$Version/$($t.exeName)"
    }
}
$manifest = [ordered]@{
    version   = $Version
    notes     = "Signed update (ADR 0013)"
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = $platforms
}

$out = Join-Path $targets[0].nsis "latest.json"
# WriteAllText with a BOM-less encoder: Set-Content -Encoding utf8 would
# add one, and the updater refuses it.
[System.IO.File]::WriteAllText($out, ($manifest | ConvertTo-Json -Depth 5), (New-Object System.Text.UTF8Encoding $false))

Write-Host "latest.json written without BOM ($($targets.Count) platforms): $out"

# (4) Publication. OUTBOUND and irreversible: once the Release is marked
# Latest with its latest.json, the installed apps auto-update. Hence the
# explicit confirmation (Chief Engineer decision), AFTER the builds -- never before.
Write-Host ""
Write-Host "Ready to publish $Version : release commit + push (gate) + BARE tag + GitHub Release Latest (5 assets: 2 exe, 2 sig, latest.json)."
$answer = Read-Host "Publish now? Type YES in capitals to continue"
if ($answer -cne "YES") {
    Write-Host "Publication CANCELLED. The artifacts stay ready; rerun or publish by hand."
    return
}

$rootDir = Join-Path $PSScriptRoot ".."
Push-Location $rootDir
try {
    # Release commit: the bump files only (never `git add -A`, which
    # would carry neighbouring work).
    git add apps/desktop/tauri.conf.json Cargo.toml Cargo.lock CHANGELOG.md scripts/make-release.ps1
    # Resumption after a partial failure (field finding 2026-08-23): if a
    # previous run already committed and pushed the bump then died before
    # the tag, the index is empty here -- `git commit` would fail on
    # "nothing to commit" and block the resumption. Skip the commit, the
    # publication goes on.
    git diff --cached --quiet
    if ($LASTEXITCODE -ne 0) {
        git commit -m "release: version $Version" -m "Bump tauri.conf.json and CHANGELOG entry; signed arm64 + x64 builds and Release published by make-release.ps1 (ADR 0013, bi-arch PLAN-RETOURS-8)."
        if ($LASTEXITCODE -ne 0) { throw "git commit failed (code $LASTEXITCODE)." }
    } else {
        Write-Host "Nothing to commit: the release commit already exists (resumption after a partial failure)."
    }
    # Push: the pre-push hook replays the full gate. A red (sometimes a
    # local e2e flake) stops here -- the commit stays local, replayable.
    git push
    if ($LASTEXITCODE -ne 0) { throw "git push failed (code $LASTEXITCODE) -- red pre-push gate? The commit stays local." }
    $sha = (git rev-parse HEAD).Trim()
}
finally {
    Pop-Location
}

# Release notes = the CHANGELOG section of the version. Sober fallback if
# the extraction fails. -Encoding UTF8 is IMPERATIVE: without it, Windows
# PowerShell 5.1 (invoked by `powershell scripts\make-release.ps1`) reads
# the UTF-8 CHANGELOG as cp1252, then WriteAllText re-encodes it in
# UTF-8 -- double encoding, non-ASCII characters turn into mojibake in
# the Release notes. (Field finding 2026-08-22: 0.1.10 to 0.6.0 repaired
# by hand.)
$clText = Get-Content -Raw -Encoding UTF8 $changelog
$rxSection = "(?sm)^## \[" + [regex]::Escape($Version) + "\].*?(?=^## \[|\z)"
$section = [regex]::Match($clText, $rxSection)
$notes = if ($section.Success) { $section.Value.Trim() } else { "Signed update (ADR 0013)." }
$notesFile = [System.IO.Path]::GetTempFileName()
[System.IO.File]::WriteAllText($notesFile, $notes, (New-Object System.Text.UTF8Encoding $false))

# GitHub Release: tag = BARE VERSION (never v$Version, the 404 trap), the
# assets DERIVED from $targets (review 2026-08-22: a target added to the
# table is published by construction, never forgotten), marked Latest,
# anchored on the release commit just pushed.
$assets = @($targets | ForEach-Object { $_.exe; $_.sig }) + $out
try {
    gh release create $Version @assets --title $Version --notes-file $notesFile --latest --target $sha
    if ($LASTEXITCODE -ne 0) { throw "gh release create failed (code $LASTEXITCODE)." }
}
finally {
    Remove-Item $notesFile -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "Release $Version published and marked Latest."
Write-Host "Verify it: powershell scripts\verify-release.ps1 $Version (STANDARD 2.10,"
Write-Host "BOTH platforms). Then confirm the AUTO-UPDATE on the installed app"
Write-Host "(arm64: this workstation; x64: the second workstation, decision D5) -- the only"
Write-Host "living proof of the signature (ADR 0013)."
