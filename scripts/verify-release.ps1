# verify-release.ps1 -- the STANDARD 2.10 verification of a published
# release, scripted (PLAN-RETOURS-8: with 5 assets and 2 platforms the
# manual checks double -- the friction is encoded once). ASCII only:
# field 2026-09-02, an em dash in a string, read as ANSI by Windows
# PowerShell 5.1, closed the string -- the script no longer parsed; the
# gate now parses every .ps1.
#
#   powershell scripts\verify-release.ps1 0.6.0
#
# Checks, for the given version: Release marked Latest at the BARE tag;
# the 5 named Windows assets (2 exe, 2 sig, latest.json) and, once
# release-macos.sh has uploaded them (PLAN-MACOS), the 6 macOS assets
# (dmg, app.tar.gz, sig -- x64 and aarch64, PLAN-APPLE-SILICON: 11
# assets, 4 keys); latest.json without BOM, matching version,
# every expected platform key; per platform: manifest
# signature == .sig file, URL at the bare tag that resolves (200,
# Content-Length == asset size); distinct signatures (anti-crossing
# guard). What this script does NOT prove (STANDARD 2.10): the minisign
# crypto unless minisign is on the PATH -- the definitive proof remains
# the auto-update <n-1> -> <n> observed in the field, PER CHANNEL
# (arm64: this workstation; x64: the second workstation, D5).

param(
    [Parameter(Mandatory = $true)][string]$Version,
    # Explicitly incomplete mode (audit lot 4, E12c / B31): the structural
    # checks only, and the verdict SAYS it is not a verification.
    [switch]$Structural,
    # The D1 exception: a release promoted with the Windows half alone.
    [switch]$WindowsOnly,
    # A release published BEFORE audit lot 4 (0.19.0 and earlier) carries
    # no attestation: said explicitly, never assumed.
    [switch]$Unattested
)

$ErrorActionPreference = "Stop"
$repo = "smonchamps/wind"
$failures = 0
$proofs = 0
function Say($ok, $text) {
    if ($ok) { Write-Host "PASS  $text" }
    else { Write-Host "FAIL  $text"; $script:failures += 1 }
}
# HEAD the URL: 302 then 200, Content-Length == the asset's size on the
# release (a truncated --clobber re-upload passed once, review
# 2026-09-04). One copy for the updater artifacts AND the dmgs.
function ResolvesWhole($url, $expectedSize, $label) {
    try {
        # -UseBasicParsing: PowerShell 5.1 would otherwise go through IE.
        # Headers['Content-Length'] is an array under pwsh, a string
        # under 5.1 -- both forms are accepted.
        $response = Invoke-WebRequest -Uri $url -Method Head -MaximumRedirection 5 -UseBasicParsing
        $cl = $response.Headers['Content-Length']
        if ($cl -is [array]) { $cl = $cl[0] }
        $size = [int64]$cl
        Say ($response.StatusCode -eq 200 -and $size -eq $expectedSize) "$label resolves 200 / $size bytes (asset: $expectedSize)"
    }
    catch {
        Say $false "$label URL does not resolve -- $($_.Exception.Message)"
    }
}

# 1. The Latest release is the version's, at the BARE tag.
$latest = gh api "repos/$repo/releases/latest" | ConvertFrom-Json
Say ($latest.tag_name -eq $Version) "Latest at bare tag '$Version' (seen: '$($latest.tag_name)')"

# The asset checks target the release OF THE VERSION, never Latest's
# (review 2026-08-22: re-verifying an n-1 after an n compared the assets
# of another release).
# Native stderr is kept out of PowerShell with `cmd /c … 2>nul`: under
# Windows PowerShell 5.1 with Stop, a redirected `2>$null` is a
# terminating error and the verdict below is never printed (review
# 2026-09-07).
$release = $null
$view = cmd /c "gh release view $Version --repo $repo --json isDraft,assets,targetCommitish 2>nul"
if ($LASTEXITCODE -eq 0 -and $view) {
    try { $release = ($view -join "") | ConvertFrom-Json } catch { }
}
if ($null -eq $release -or $null -eq $release.assets) {
    Say $false "release at tag '$Version' not found on GitHub"
    Write-Host ""
    Write-Host "$failures check(s) failed -- the release is NOT declared verified."
    exit 1
}
if ($release.isDraft) {
    Say $false "release '$Version' is still a DRAFT -- publish-release.ps1 proves and promotes it; this script checks the public release"
    Write-Host ""
    Write-Host "$failures check(s) failed -- the release is NOT declared verified."
    exit 1
}
# The tag and the attestations (E12a): every platform built from the
# tagged commit, from main.
$tagCommit = cmd /c "gh api repos/$repo/git/ref/tags/$Version --jq .object.sha 2>nul"
if ($LASTEXITCODE -eq 0 -and -not [string]::IsNullOrWhiteSpace("$tagCommit")) {
    $tagCommit = "$tagCommit".Trim()
    Say ($release.targetCommitish -eq $tagCommit) "release anchored on the tag's commit $($tagCommit.Substring(0, 7))"
} else {
    Say $false "tag '$Version' absent from GitHub"
    $tagCommit = ""
}

# 2. The assets, named exactly. 5 Windows assets always; the 3 macOS
# assets (dmg + updater tar.gz + sig, PLAN-MACOS D4/D7) are uploaded
# LATER by release-macos.sh from the MacBook -- before that upload the
# script says NOT PRESENT (never PASS, never FAIL: the Windows half is
# verifiable on publication day, the mac half on its own upload).
# The matrix comes from scripts/release-lib.mjs -- the ONE tested copy
# (audit lot 4 review: it lived in three files). Eleven assets, four
# keys and two attestations; five, two and one under -WindowsOnly.
$matrixFlags = @()
if ($WindowsOnly) { $matrixFlags += "--windows-only" }
$matrix = (& node (Join-Path $PSScriptRoot "release-lib.mjs") channels $Version @matrixFlags) -join "" | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or $null -eq $matrix) {
    Say $false "release-lib channels failed"
    Write-Host ""
    Write-Host "$failures check(s) failed -- the release is NOT declared verified."
    exit 1
}
$expected = @($matrix.assets)
if ($Unattested) {
    $expected = @($expected | Where-Object { $_ -notlike "attestation-*.json" })
    Write-Host "UNATTESTED  release published before lot 4: no attestation expected (explicit -Unattested)"
}
$macArchs = @("x64", "aarch64")
$names = @($release.assets | ForEach-Object { $_.name })
$macPresent = ($names -contains "Wind_${Version}_x64.app.tar.gz")
if ($WindowsOnly) {
    Write-Host "WINDOWS-ONLY  macOS assets absent by explicit decision (mac clients find no update)"
}
# The count DERIVES from the name list: they can never disagree.
Say ($names.Count -eq $expected.Count) "$($expected.Count) assets ($($names.Count) seen)"
foreach ($n in $expected) {
    Say ($names -contains $n) "asset '$n' present"
}

# 3. latest.json: no BOM, matching version, both platforms.
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) "wind-verify-$Version"
New-Item -ItemType Directory -Force $tmp | Out-Null
try {
    # A failed download (missing asset, network) is a FAILED verdict,
    # never an exception that swallows the report (review 2026-08-22) --
    # it is the very scenario this script exists to catch.
    gh release download $Version --repo $repo --pattern "latest.json" --pattern "*.sig" --pattern "attestation-*.json" --dir $tmp --clobber
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path (Join-Path $tmp "latest.json"))) {
        Say $false "download of the assets (latest.json + .sig) -- gh release download code $LASTEXITCODE"
        Write-Host ""
        Write-Host "$failures check(s) failed -- the release is NOT declared verified."
        exit 1
    }

    $bytes = [System.IO.File]::ReadAllBytes((Join-Path $tmp "latest.json"))
    $bom = ($bytes.Length -ge 3 -and $bytes[0] -eq 0xEF -and $bytes[1] -eq 0xBB -and $bytes[2] -eq 0xBF)
    Say (-not $bom) "latest.json without BOM ($($bytes.Length) bytes)"
    $manifest = [System.Text.Encoding]::UTF8.GetString($bytes) | ConvertFrom-Json
    Say ($manifest.version -eq $Version) "manifest version '$($manifest.version)'"

    # The manifest key and the mac assets stand or fall TOGETHER: a
    # darwin key whose tar.gz was deleted (or a rerun that uploaded
    # assets but died before the manifest) is a broken release, not a
    # "not yet" (review 2026-09-04 -- keying the checks on asset
    # presence alone let a manifest-orphan darwin key pass silently
    # while every mac updater 404'd).
    $macKey = ($null -ne $manifest.platforms.'darwin-x86_64')
    Say ($macKey -eq $macPresent) "darwin-x86_64 key and mac assets consistent (key: $macKey, assets: $macPresent)"
    $armKey = ($null -ne $manifest.platforms.'darwin-aarch64')
    Say ($armKey -eq $macPresent) "darwin-aarch64 key and mac assets consistent (key: $armKey, assets: $macPresent)"

    # `exe` names the downloadable updater artifact of the channel --
    # the bare NSIS exe on Windows, the .app.tar.gz on macOS.
    $platforms = @($matrix.channels)
    # Attestations: at the tag's commit, from main (E12a).
    foreach ($attName in @("attestation-windows.json", "attestation-macos.json")) {
        $attPath = Join-Path $tmp $attName
        if (-not (Test-Path $attPath)) {
            if (-not $Unattested -and ($expected -contains $attName)) { Say $false "$attName absent from the release" }
            continue
        }
        $att = Get-Content -Raw -Encoding UTF8 $attPath | ConvertFrom-Json
        Say ($att.version -eq $Version -and $att.branch -eq "main" -and $att.commit -eq $tagCommit) "$attName at $Version, main, commit $(if ($att.commit) { $att.commit.Substring(0, 7) })"
    }
    $conf = Get-Content (Join-Path $PSScriptRoot "..\apps\desktop\tauri.conf.json") -Raw -Encoding UTF8 | ConvertFrom-Json
    $pubkey = $conf.plugins.updater.pubkey
    $rootDir = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
    # The verifier, built ONCE and loudly: a build error must never read
    # as "signature NOT verified".
    $verifier = Join-Path $rootDir "target\debug\release-verify.exe"
    if (-not $Structural) {
        Push-Location $rootDir
        try { cargo build -q -p release-verify; Say ($LASTEXITCODE -eq 0) "release-verify built" }
        finally { Pop-Location }
    }
    $signatures = @()
    foreach ($p in $platforms) {
        $entry = $manifest.platforms.($p.key)
        Say ($null -ne $entry) "platform '$($p.key)' present in the manifest"
        if ($null -eq $entry) { continue }

        # Manifest signature == .sig file of the SAME architecture.
        $sigFile = Join-Path $tmp "$($p.exe).sig"
        if (Test-Path $sigFile) {
            $sig = (Get-Content -Raw $sigFile).Trim()
            Say ($entry.signature -eq $sig) "$($p.key): signature == $($p.exe).sig"
        }
        else {
            Say $false "$($p.key): file $($p.exe).sig absent from the release"
        }
        $signatures += $entry.signature
        # The cryptographic proof (audit lot 4, E12c / B31): the artifact
        # is downloaded and verified by tools/release-verify -- the
        # updater's own minisign implementation, the Tauri base64 wrapper
        # decoded -- against the pubkey of tauri.conf.json. Absent proof
        # is a FAILURE; only -Structural skips it, and says so.
        if ($Structural) {
            Write-Host "STRUCTURAL  $($p.key): signature not verified (explicit -Structural mode)"
        } elseif ((Test-Path $sigFile) -and (Test-Path $verifier)) {
            gh release download $Version --repo $repo --pattern $p.exe --dir $tmp --clobber | Out-Null
            & $verifier verify --pubkey $pubkey --signature $sigFile --artifact (Join-Path $tmp $p.exe) | Out-Null
            $valid = ($LASTEXITCODE -eq 0)
            if ($valid) { $script:proofs += 1 }
            Say $valid "$($p.key): signature VERIFIED on $($p.exe) (release-verify)"
        }

        # URL at the BARE tag, name of the right architecture.
        $expectedUrl = "https://github.com/$repo/releases/download/$Version/$($p.exe)"
        Say ($entry.url -eq $expectedUrl) "$($p.key): URL at the bare tag to $($p.exe)"

        # The URL resolves (302 then 200), Content-Length == asset size.
        $asset = $release.assets | Where-Object { $_.name -eq $p.exe }
        ResolvesWhole $entry.url $asset.size "$($p.key): the exe"
    }
    # The dmg is the mac FIRST-INSTALL artifact (no .sig by design --
    # the updater never touches it) and the only downloadable with no
    # integrity check otherwise: at least prove it resolves whole
    # (review 2026-09-04: a truncated --clobber re-upload passed).
    # Both dmgs (review 2026-09-05: the aarch64 dmg was the one asset
    # no check covered when the family was added).
    if ($macPresent) {
        foreach ($a in $macArchs) {
            $dmgName = "Wind_${Version}_$a.dmg"
            $dmgAsset = $release.assets | Where-Object { $_.name -eq $dmgName }
            ResolvesWhole "https://github.com/$repo/releases/download/$Version/$dmgName" $dmgAsset.size "${a}: the dmg"
        }
    }

    # Anti-crossing guard: one DISTINCT signature per channel, whatever
    # their number (2 Windows, +2 macOS once uploaded).
    $uniqueSignatures = @($signatures | Select-Object -Unique)
    Say ($signatures.Count -eq $platforms.Count -and $uniqueSignatures.Count -eq $signatures.Count) "$($signatures.Count) channel signatures, pairwise distinct"
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Host ""
if ($Structural) {
    if ($failures -eq 0) { Write-Host "STRUCTURAL ONLY: the form checks pass, the signatures were NOT verified -- this is not a verification (rerun without -Structural)." }
    else { Write-Host "$failures check(s) failed in structural mode -- the release is NOT declared verified." }
}
elseif ($failures -eq 0 -and $proofs -eq $platforms.Count) {
    Write-Host "Verification 2.10: everything passes, $proofs channel signature(s) verified. The field proof remains -- the auto-update"
    Write-Host "observed PER CHANNEL (arm64: this workstation; x64: the second workstation, D5)."
}
else {
    Write-Host "$failures check(s) failed, $proofs/$($platforms.Count) signature(s) proven -- the release is NOT declared verified."
}
# No ternary: the script must run under Windows PowerShell 5.1.
if ($failures -eq 0 -and ($Structural -or $proofs -eq $platforms.Count)) { exit 0 } else { exit 1 }
