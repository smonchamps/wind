# publish-release.ps1 -- the LAST gesture of a release (audit lot 4,
# E12b, Chief Engineer decision D1 of 2026-09-07): promotes the DRAFT
# created by make-release.ps1 and completed by release-macos.sh to the
# public Latest -- only once the whole matrix is proven.
#
#   powershell scripts\publish-release.ps1 0.20.0
#   powershell scripts\publish-release.ps1 0.20.0 -WindowsOnly   (the Air unavailable: SAYS what it drops)
#
# Proven before promotion, from the draft's own assets (downloaded through
# gh, which reads drafts): eleven assets and four keys (five and two under
# -WindowsOnly), latest.json without BOM at the version, every channel's
# manifest signature equal to its .sig, every signature VERIFIED
# cryptographically by tools/release-verify (the updater's own minisign
# implementation) against the pubkey of tauri.conf.json, pairwise distinct
# signatures, both attestations at the tag's commit with the uploaded
# bytes equal to the attested ones. One failure = no promotion, the
# previous Latest stays complete for every updater. After promotion the
# public URLs are checked by verify-release.ps1.
#
# Written in ASCII -- convention of the repository's .ps1 files.

param(
    [Parameter(Mandatory = $true)][string]$Version,
    [switch]$WindowsOnly
)

$ErrorActionPreference = "Stop"
$repo = "smonchamps/wind"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

if ($Version -notmatch '^[0-9]+\.[0-9]+\.[0-9]+$') { throw "Version '$Version' invalid -- MAJOR.MINOR.PATCH, without 'v'." }

# 1. The draft, its commit, its tag.
# Native stderr is never redirected in this script: under Windows
# PowerShell 5.1 with $ErrorActionPreference = Stop, `2>$null` turns a
# gh "not found" into a terminating error instead of the verdict below
# (review 2026-09-07). `cmd /c` keeps gh's stderr out of PowerShell.
$release = $null
$view = cmd /c "gh release view $Version --repo $repo --json isDraft,targetCommitish,tagName,assets 2>nul"
if ($LASTEXITCODE -eq 0 -and $view) { $release = ($view -join "") | ConvertFrom-Json }
if ($null -eq $release) { throw "no release (draft or not) at tag '$Version' on GitHub -- make-release.ps1 stages the draft first." }
if (-not $release.isDraft) { throw "release '$Version' is already published -- nothing to promote (verify it: scripts\verify-release.ps1 $Version)." }
$tagCommit = cmd /c "gh api repos/$repo/git/ref/tags/$Version --jq .object.sha 2>nul"
if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace("$tagCommit")) { throw "tag '$Version' does not exist on GitHub -- make-release.ps1 pushes it before creating the draft." }
$tagCommit = "$tagCommit".Trim()
if ($release.targetCommitish -ne $tagCommit) { throw "the draft targets $($release.targetCommitish) but the tag points at $tagCommit." }
Write-Host "Draft $Version at $($tagCommit.Substring(0, 7)), $($release.assets.Count) asset(s)."
if ($WindowsOnly) {
    Write-Host "WINDOWS-ONLY promotion (D1 exception): the darwin keys are ABSENT from latest.json --"
    Write-Host "mac clients will find no update until release-macos.sh and a new promotion."
}

# 2. Every asset of the draft, in a scratch folder.
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) "wind-publish-$Version"
if (Test-Path $tmp) { Remove-Item -Recurse -Force $tmp }
New-Item -ItemType Directory -Force $tmp | Out-Null
try {
    gh release download $Version --repo $repo --dir $tmp --clobber
    if ($LASTEXITCODE -ne 0) { throw "gh release download failed (code $LASTEXITCODE)." }

    # 3. The cryptographic proof per channel: the workspace tool, never an
    # optional external binary (B31). Built ONCE, loudly, then called as
    # an exe: a build error can never pass for "signature NOT verified".
    # The matrix comes from release-lib.mjs, the one tested copy.
    $conf = Get-Content (Join-Path $root "apps\desktop\tauri.conf.json") -Raw -Encoding UTF8 | ConvertFrom-Json
    $pubkey = $conf.plugins.updater.pubkey
    $matrixFlags = @()
    if ($WindowsOnly) { $matrixFlags += "--windows-only" }
    $matrix = (& node (Join-Path $PSScriptRoot "release-lib.mjs") channels $Version @matrixFlags) -join "" | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0) { throw "release-lib channels failed (code $LASTEXITCODE)." }
    Push-Location $root
    try {
        cargo build -q -p release-verify
        if ($LASTEXITCODE -ne 0) { throw "cargo build -p release-verify failed (code $LASTEXITCODE) -- no proof possible, no promotion." }
    }
    finally { Pop-Location }
    $verifier = Join-Path $root "target\debug\release-verify.exe"
    $proven = @()
    foreach ($c in $matrix.channels) {
        $artifact = Join-Path $tmp $c.exe
        $sig = "$artifact.sig"
        if (-not (Test-Path $artifact) -or -not (Test-Path $sig)) {
            Write-Host "FAIL  $($c.key): artifact or .sig absent from the draft"
            continue
        }
        & $verifier verify --pubkey $pubkey --signature $sig --artifact $artifact
        if ($LASTEXITCODE -eq 0) { Write-Host "PASS  $($c.key): signature verified"; $proven += "--proven=$($c.key)" }
        else { Write-Host "FAIL  $($c.key): signature NOT verified" }
    }

    # 4. The promotion rule (scripts/release-lib.mjs, proven by node --test).
    $flags = @($proven)
    if ($WindowsOnly) { $flags += "--windows-only" }
    & node (Join-Path $PSScriptRoot "release-lib.mjs") publishable $tmp $Version $tagCommit @flags
    if ($LASTEXITCODE -ne 0) {
        throw "the draft is NOT publishable (failures above) -- the previous Latest stays in place."
    }
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

# 5. Promotion: outbound and irreversible -- the installed apps update from
# here. The typed YES is the point of this script (D1); no switch skips it.
$answer = Read-Host "Promote $Version to Latest now? Type YES in capitals to continue"
if ($answer -cne "YES") { Write-Host "Promotion CANCELLED. The draft stays as it is."; return }
gh release edit $Version --repo $repo --draft=false --latest
if ($LASTEXITCODE -ne 0) { throw "gh release edit failed (code $LASTEXITCODE) -- the draft is unchanged." }
Write-Host "Release $Version published and marked Latest."
Write-Host "Public check: powershell scripts\verify-release.ps1 $Version$(if ($WindowsOnly) { ' -WindowsOnly' })"
