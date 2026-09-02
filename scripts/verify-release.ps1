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
# the 5 named assets (2 exe, 2 sig, latest.json); latest.json without
# BOM, matching version, BOTH platform keys; per platform: manifest
# signature == .sig file, URL at the bare tag that resolves (200,
# Content-Length == asset size); distinct signatures (anti-crossing
# guard). What this script does NOT prove (STANDARD 2.10): the minisign
# crypto unless minisign is on the PATH -- the definitive proof remains
# the auto-update <n-1> -> <n> observed in the field, PER CHANNEL
# (arm64: this workstation; x64: the second workstation, D5).

param([Parameter(Mandatory = $true)][string]$Version)

$ErrorActionPreference = "Stop"
$repo = "smonchamps/wind"
$failures = 0
function Say($ok, $text) {
    if ($ok) { Write-Host "PASS  $text" }
    else { Write-Host "FAIL  $text"; $script:failures += 1 }
}

# 1. The Latest release is the version's, at the BARE tag.
$latest = gh api "repos/$repo/releases/latest" | ConvertFrom-Json
Say ($latest.tag_name -eq $Version) "Latest at bare tag '$Version' (seen: '$($latest.tag_name)')"

# The asset checks target the release OF THE VERSION, never Latest's
# (review 2026-08-22: re-verifying an n-1 after an n compared the assets
# of another release).
$release = $null
try {
    $release = gh api "repos/$repo/releases/tags/$Version" 2>$null | ConvertFrom-Json
}
catch { }
if ($null -eq $release -or $null -eq $release.assets) {
    Say $false "release at tag '$Version' not found on GitHub"
    Write-Host ""
    Write-Host "$failures check(s) failed -- the release is NOT declared verified."
    exit 1
}

# 2. The 5 assets, named exactly.
$expected = @(
    "Wind_${Version}_arm64-setup.exe",
    "Wind_${Version}_arm64-setup.exe.sig",
    "Wind_${Version}_x64-setup.exe",
    "Wind_${Version}_x64-setup.exe.sig",
    "latest.json"
)
$names = @($release.assets | ForEach-Object { $_.name })
Say ($names.Count -eq 5) "5 assets ($($names.Count) seen)"
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
    gh release download $Version --repo $repo --pattern "latest.json" --pattern "*.sig" --dir $tmp --clobber
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

    $platforms = @(
        @{ key = "windows-aarch64"; exe = "Wind_${Version}_arm64-setup.exe" },
        @{ key = "windows-x86_64"; exe = "Wind_${Version}_x64-setup.exe" }
    )
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
        # PLAN-AUDIT-V2 E9: the minisign crypto, if the tool is on the PATH --
        # the installer is downloaded (once per platform) and verified
        # against the public key of the Tauri manifest. Without the tool:
        # SAYS "not proven", never PASS.
        $minisign = Get-Command minisign -ErrorAction SilentlyContinue
        if ($minisign) {
            $conf = Get-Content (Join-Path $PSScriptRoot "..\apps\desktop\tauri.conf.json") -Raw | ConvertFrom-Json
            $pub = $conf.plugins.updater.pubkey
            $pubFile = Join-Path $tmp "minisign.pub"
            [IO.File]::WriteAllText($pubFile, [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($pub)))
            gh release download $Version --repo $repo --pattern $p.exe --dir $tmp --clobber | Out-Null
            & minisign -Vm (Join-Path $tmp $p.exe) -x $sigFile -p $pubFile | Out-Null
            Say ($LASTEXITCODE -eq 0) "$($p.key): minisign signature VALID on $($p.exe)"
        } else {
            Write-Host "NOT PROVEN  $($p.key): minisign crypto (minisign absent from the PATH -- winget install minisign)"
        }

        # URL at the BARE tag, name of the right architecture.
        $expectedUrl = "https://github.com/$repo/releases/download/$Version/$($p.exe)"
        Say ($entry.url -eq $expectedUrl) "$($p.key): URL at the bare tag to $($p.exe)"

        # The URL resolves (302 then 200), Content-Length == asset size.
        $asset = $release.assets | Where-Object { $_.name -eq $p.exe }
        try {
            # -UseBasicParsing: PowerShell 5.1 would otherwise go through IE.
            # Headers['Content-Length'] is an array under pwsh, a string
            # under 5.1 -- both forms are accepted.
            $response = Invoke-WebRequest -Uri $entry.url -Method Head -MaximumRedirection 5 -UseBasicParsing
            $cl = $response.Headers['Content-Length']
            if ($cl -is [array]) { $cl = $cl[0] }
            $size = [int64]$cl
            Say ($response.StatusCode -eq 200 -and $size -eq $asset.size) "$($p.key): the exe resolves 200 / $size bytes (asset: $($asset.size))"
        }
        catch {
            Say $false "$($p.key): the URL does not resolve -- $($_.Exception.Message)"
        }
    }
    # Anti-crossing guard: two channels, two DISTINCT signatures.
    Say ($signatures.Count -eq 2 -and $signatures[0] -ne $signatures[1]) "arm64 and x64 signatures distinct"
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Host ""
if ($failures -eq 0) {
    Write-Host "Verification 2.10: everything passes. The field proof remains -- the auto-update"
    Write-Host "observed PER CHANNEL (arm64: this workstation; x64: the second workstation, D5)."
}
else {
    Write-Host "$failures check(s) failed -- the release is NOT declared verified."
}
# No ternary: the script must run under Windows PowerShell 5.1.
if ($failures -eq 0) { exit 0 } else { exit 1 }
