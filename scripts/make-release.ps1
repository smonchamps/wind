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

param(
    [Parameter(Mandatory = $true)][string]$Version,
    # Non-interactive confirmation (the release-script fixtures); the
    # Chief Engineer types YES by hand on release day.
    [switch]$Yes
)

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
# Build identity (audit lot 4, E12a / B29): the binaries are built from
# the RELEASE COMMIT and nothing else. A dirty tree used to be baked into
# the exe while only the bump files were committed; refused before any
# write, from main only.
$rootDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $rootDir
try {
    $branch = (git branch --show-current).Trim()
    if ($branch -ne "main") { throw "Current branch '$branch': a release is made from main." }
    $dirty = @(git status --porcelain)
    if ($dirty.Count -gt 0) {
        throw "Working tree not clean ($($dirty.Count) path(s)): commit or stash first -- a release is built from its commit alone.`n$($dirty -join "`n")"
    }
}
finally { Pop-Location }
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

# The release commit comes BEFORE the builds (E12a): the lockfile follows
# the workspace bump now (never at the first build, which used to leave
# the tree dirty), and HEAD at build time IS the tagged commit. The
# commit stays local until the YES below; a failed build leaves a
# replayable local commit, nothing published.
Push-Location $rootDir
try {
    cargo update --workspace --offline
    if ($LASTEXITCODE -ne 0) { throw "cargo update --workspace failed (code $LASTEXITCODE)." }
    git add apps/desktop/tauri.conf.json Cargo.toml Cargo.lock CHANGELOG.md
    git diff --cached --quiet
    if ($LASTEXITCODE -ne 0) {
        git commit -m "release: version $Version" -m "Bump tauri.conf.json, Cargo.toml and Cargo.lock; CHANGELOG entry. Signed arm64 + x64 builds, draft Release and attestation by make-release.ps1 (ADR 0013, PLAN-RETOURS-8, audit lot 4 E12)."
        if ($LASTEXITCODE -ne 0) { throw "git commit failed (code $LASTEXITCODE)." }
    } else {
        Write-Host "Nothing to commit: the release commit already exists (resumption after a partial failure)."
    }
    $dirty = @(git status --porcelain)
    if ($dirty.Count -gt 0) { throw "Tree not clean after the release commit: $($dirty -join ', ')" }
    $sha = (git rev-parse HEAD).Trim()
}
finally { Pop-Location }
Write-Host "Release commit $($sha.Substring(0, 7)): the builds start from it."

# (2) The TWO signed builds, arm64 then x64. ALL-OR-NOTHING (D7): the
# first failure throws, nothing is published, never a channel out of
# step. The PASSWORD is deliberately NOT set as a variable (ADR 0013
# invariant kept): Tauri asks for it by hand at EACH build -- two
# entries, the price for it never appearing in an environment inherited
# by the build's child processes.
$desktop = Join-Path $PSScriptRoot "..\apps\desktop"

# The release dist is built CLEAN of the e2e seams and asserted by
# `cargo tauri build` itself: tauri.conf.json's beforeBuildCommand runs
# scripts/build-dist-clean.mjs (lot 4 E12d -- one declaration where the
# build reads it, no longer a sequence copied here and in
# release-macos.sh). A seam in the bundle fails the build before any
# artifact exists.

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

# The builds changed nothing in the tree (E12a): a lockfile or config
# rewritten by the build would mean binaries that do not match HEAD.
# CONTENT identity, not stat identity (0.21.0 release day): the tauri
# CLI rewrites apps/desktop/Cargo.toml byte-identically but with LF
# endings, and under core.autocrlf `git status --porcelain` flags the
# EOL-only difference while `git diff` and the blob hash say equal --
# a false red after both signed builds. `git diff --quiet` (worktree)
# + `--cached` (index) compare what a commit would actually store.
Push-Location $rootDir
try {
    git diff --quiet
    $worktreeChanged = $LASTEXITCODE -ne 0
    git diff --cached --quiet
    $indexChanged = $LASTEXITCODE -ne 0
    if ($worktreeChanged -or $indexChanged) {
        $dirty = @(git status --porcelain)
        throw "The builds modified the tree ($($dirty -join ', ')): the binaries would not match the release commit."
    }
    $untracked = @(git ls-files --others --exclude-standard)
    if ($untracked.Count -gt 0) {
        throw "The builds left untracked files ($($untracked -join ', ')): the binaries would not match the release commit."
    }
    if ((git rev-parse HEAD).Trim() -ne $sha) { throw "HEAD moved during the builds." }
}
finally { Pop-Location }

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

# The attestation (E12a): version, tag, commit, branch, lockfile and dist
# digests, digest of every artifact -- uploaded with the assets, checked
# again by publish-release.ps1 against the tag and the uploaded bytes.
$attestation = Join-Path $targets[0].nsis "attestation-windows.json"
$artifacts = @($targets | ForEach-Object { $_.exe; $_.sig })
& node (Join-Path $PSScriptRoot "release-lib.mjs") attest $attestation windows $Version $sha main (Join-Path $rootDir "Cargo.lock") (Join-Path $desktop "ui-v2\dist") @artifacts
if ($LASTEXITCODE -ne 0) { throw "attestation failed (code $LASTEXITCODE)." }

# (4) Publication. OUTBOUND and irreversible: once the Release is marked
# Latest with its latest.json, the installed apps auto-update. Hence the
# explicit confirmation (Chief Engineer decision), AFTER the builds -- never before.
Write-Host ""
Write-Host "Ready to stage $Version : push of the release commit (gate) + BARE tag + DRAFT GitHub Release (6 assets: 2 exe, 2 sig, latest.json, attestation)."
Write-Host "Nothing becomes public here (D1, lot 4): publish-release.ps1 promotes the draft once the whole matrix is proven."
if (-not $Yes) {
    $answer = Read-Host "Stage now? Type YES in capitals to continue"
    if ($answer -cne "YES") {
        Write-Host "Staging CANCELLED. The release commit and the artifacts stay local; rerun to resume."
        return
    }
}

Push-Location $rootDir
try {
    # Push: the pre-push hook replays the full gate. A red (sometimes a
    # local e2e flake) stops here -- the commit stays local, replayable.
    git push
    if ($LASTEXITCODE -ne 0) { throw "git push failed (code $LASTEXITCODE) -- red pre-push gate? The commit stays local." }
    # The BARE tag at the release commit, pushed explicitly: a draft
    # Release creates no tag by itself, and release-macos.sh and
    # publish-release.ps1 both verify HEAD against this tag. Resumption
    # after a partial failure: an existing tag at the SAME commit is
    # reused; at another commit it is refused. No `2>$null` here: under
    # Windows PowerShell 5.1 with $ErrorActionPreference = Stop, a
    # redirected native stderr line is a terminating error (review
    # 2026-09-07) -- `rev-parse -q --verify` answers by exit code alone.
    $existing = (git rev-parse -q --verify "refs/tags/$Version^{commit}")
    if ($LASTEXITCODE -eq 0) {
        if ("$existing".Trim() -ne $sha) { throw "tag $Version already exists at $("$existing".Trim().Substring(0, 7)), not at the release commit $($sha.Substring(0, 7)) -- delete it knowingly (git tag -d $Version; git push origin :refs/tags/$Version) and rerun." }
        Write-Host "Tag $Version already at the release commit (resumption)."
    } else {
        git tag $Version $sha
        if ($LASTEXITCODE -ne 0) { throw "git tag $Version failed (code $LASTEXITCODE)." }
    }
    git push origin $Version
    if ($LASTEXITCODE -ne 0) { throw "git push of tag $Version failed (code $LASTEXITCODE)." }
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
$assets = @($targets | ForEach-Object { $_.exe; $_.sig }) + $out + $attestation
try {
    # DRAFT (D1): invisible to every updater until publish-release.ps1
    # promotes it -- the previous Latest stays complete meanwhile (B30).
    gh release create $Version @assets --title $Version --notes-file $notesFile --draft --target $sha
    if ($LASTEXITCODE -ne 0) { throw "gh release create failed (code $LASTEXITCODE)." }
}
finally {
    Remove-Item $notesFile -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "Draft Release $Version staged at $($sha.Substring(0, 7)) with the Windows half (6 assets)."
Write-Host ""
Write-Host "Next, in order (lot 4, D1):"
Write-Host "  1. On the Mac: ./scripts/release-macos.sh $Version  (6 assets + attestation, darwin keys)."
Write-Host "  2. Here: powershell scripts\publish-release.ps1 $Version  (proves the whole matrix, then Latest)."
Write-Host "     Without the Air: publish-release.ps1 $Version -WindowsOnly -- it says what it drops."
Write-Host "  3. Here: powershell scripts\verify-release.ps1 $Version  (the public URLs, STANDARD 2.10)."
Write-Host "Then confirm the AUTO-UPDATE on the installed app (arm64: this workstation;"
Write-Host "x64: the second workstation, D5) -- the living proof (ADR 0013)."
