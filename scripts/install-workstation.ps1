#Requires -Version 5.1
<#
  install-workstation.ps1 -- prepares a Windows workstation to develop Wind.

  Idempotent: replayable without breakage (every tool is checked before
  being installed). Touches NO secret: OAuth tokens and the signing key
  stay in the user's hands (recalled at the end of the script).

  Written in ASCII -- convention of the repository's .ps1 files (see
  make-release.ps1), which avoids the encoding trap of Windows
  PowerShell 5.1 on UTF-8 sources.

  On a fresh machine (nothing installed):
     winget install -e --id Git.Git
     git clone https://github.com/smonchamps/wind.git C:\dev\wind
     cd C:\dev\wind
     powershell -ExecutionPolicy Bypass -File scripts\install-workstation.ps1

  (Clone OUTSIDE OneDrive: it disturbs the measurements and makes the e2e
   flake -- STANDARD section 7.3.)

  Options:
     -SkipBuildTools    does not touch Visual Studio Build Tools
     -WithCargoAudit    also installs cargo-audit (played by the CI, not the local gate)
     -CrossArm64Check   compiles wind-desktop for arm64 (proof of the cross-build, ~minutes)

  This script prepares an x64 workstation. The arm64 target is installed
  IN ADDITION (cross-compilation) to be able to produce both release
  channels. The bi-arch release is DONE (PLAN-RETOURS-8, ADR 0023):
  make-release.ps1 builds arm64 + x64 and publishes 5 assets; the mirror
  of this script on the ARM64 workstation is the rustup target
  x86_64-pc-windows-msvc + the MSVC x64 component (present in Build Tools
  by default) + the lld-link override of the x64 triple (.cargo/config.toml).
#>
param(
    [switch]$SkipBuildTools,
    [switch]$WithCargoAudit,
    [switch]$CrossArm64Check
)

$ErrorActionPreference = "Stop"

function Section($t) { Write-Host ""; Write-Host "=== $t ===" -ForegroundColor Cyan }
function Info($t) { Write-Host "  $t" }
function Ok($t) { Write-Host "  OK   $t" -ForegroundColor Green }
function Warn($t) { Write-Host "  !!   $t" -ForegroundColor Yellow }

function Update-SessionPath {
    # winget/rustup write the persistent PATH; reload it in THIS session so
    # that node/cargo/git work without reopening the shell.
    $m = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $u = [Environment]::GetEnvironmentVariable("Path", "User")
    $env:Path = ($m, $u -join ";")
    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    if ((Test-Path $cargoBin) -and ($env:Path -notmatch [regex]::Escape($cargoBin))) {
        $env:Path += ";$cargoBin"
    }
}

function Winget-Present($id) {
    winget list --id $id -e *> $null
    return ($LASTEXITCODE -eq 0)
}

function Ensure-Winget($id, $override) {
    if (Winget-Present $id) { Ok "$id already installed"; return }
    Info "Installing $id ..."
    $a = @("install", "-e", "--id", $id, "--accept-source-agreements", "--accept-package-agreements")
    if ($override) { $a += @("--override", $override) }
    winget @a
    if ($LASTEXITCODE -ne 0) { throw "winget failed for $id (code $LASTEXITCODE)" }
    Ok "$id installed"
}

function Npm-Bootstrap($dir) {
    Push-Location $dir
    try {
        if (Test-Path "package-lock.json") { npm ci } else { npm install }
        if ($LASTEXITCODE -ne 0) { throw "npm failed in $dir (code $LASTEXITCODE)" }
    }
    finally { Pop-Location }
}

# --------------------------------------------------------------------------

Section "Prerequisites"
if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    throw "winget not found. Install 'App Installer' from the Microsoft Store, then rerun."
}
Ok "winget present"

Section "System tools (winget)"
Ensure-Winget "Git.Git"
Ensure-Winget "Rustlang.Rustup"
Ensure-Winget "Microsoft.EdgeWebView2Runtime"   # the app's webview + the window driven by the e2e
Ensure-Winget "OpenJS.NodeJS.LTS"               # CI pins Node 24 (LTS)
Ensure-Winget "Python.Python.3.13"              # e2e/freeze-probe.py
Ensure-Winget "GitHub.cli"                      # gh run list / gh release

Section "Visual Studio Build Tools (C++ x64 + cross arm64)"
if ($SkipBuildTools) {
    Warn "Skipped (-SkipBuildTools)."
}
elseif (Winget-Present "Microsoft.VisualStudio.2022.BuildTools") {
    Warn "Build Tools already present: I do not modify an existing install."
    Warn "To LINK arm64, add the component 'MSVC v143 - VS 2022 C++ ARM64"
    Warn "build tools' through the 'Visual Studio Installer' app (Modify > Individual"
    Warn "components). Without it, the arm64 target downloads but does not LINK."
}
else {
    # VCTools = MSVC x64/x86 compiler + (with includeRecommended) Windows SDK.
    # VC.Tools.ARM64 = arm64 cross-compiler, required for the second channel.
    $ovr = "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools " +
    "--add Microsoft.VisualStudio.Component.VC.Tools.ARM64 --includeRecommended"
    Ensure-Winget "Microsoft.VisualStudio.2022.BuildTools" $ovr
}

Update-SessionPath

Section "Rust (rustup)"
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    throw "rustup unreachable after installation. Reopen PowerShell and rerun the script (idempotent)."
}
# The VERSION is pinned in rust-toolchain.toml (1.97.1): rustup switches to
# it by itself at the first cargo in the repository. We only guarantee a
# stable default for commands outside the repository.
rustup default stable | Out-Null
Ok "rustup: default stable (the repository forces 1.97.1 through rust-toolchain.toml)"

Section "cargo tools"
$cargoList = (cargo install --list) 2>$null
if ($cargoList -match "tauri-cli") {
    Ok "tauri-cli already installed"
}
else {
    Info "Installing tauri-cli (compiled from source, several minutes) ..."
    cargo install tauri-cli --version "^2.0" --locked
    Ok "tauri-cli installed (cargo tauri build)"
}
if ($WithCargoAudit) {
    if ($cargoList -match "cargo-audit") { Ok "cargo-audit already installed" }
    else { cargo install cargo-audit --locked; Ok "cargo-audit installed" }
}

Section "Wind repository"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $root
if (-not (Test-Path (Join-Path $root "Cargo.toml"))) {
    throw "This script must live in scripts\ of the Wind repository (Cargo.toml not found at the root)."
}
Ok "Repository root: $root"

# Repository hooks: a fresh clone does not enable them by itself.
git config core.hooksPath .githooks
Ok "core.hooksPath = .githooks (pre-push gate + commit-msg co-author strip armed)"

# Local git identity: NEVER guessed (personal data). Warn if absent.
$mail = ((git config --local user.email) 2>$null)
if ([string]::IsNullOrWhiteSpace($mail)) {
    Warn "Git identity not set on this clone. Take it from the current machine:"
    Warn "  git config user.name  ""smonchamps"""
    Warn "  git config user.email ""<your-noreply-address@users.noreply.github.com>"""
}
else {
    Ok "Local git identity: $mail"
}

Section "JS dependencies + first build"
Info "ui-v2: npm ci + build (MUST precede cargo -- generate_context! requires ui-v2/dist)"
Npm-Bootstrap (Join-Path $root "apps\desktop\ui-v2")
Push-Location (Join-Path $root "apps\desktop\ui-v2")
try { npm run build; if ($LASTEXITCODE -ne 0) { throw "ui-v2 build failed" } } finally { Pop-Location }
Ok "ui-v2 built"

Info "e2e: npm ci"
Npm-Bootstrap (Join-Path $root "e2e")
Ok "e2e ready"

Info "cargo build --workspace (materializes the repository's 1.97.1 toolchain)"
cargo build --workspace
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
Ok "workspace compiled (native x64)"

Section "arm64 target (cross-build from this x64 workstation)"
# Added TO the repository's toolchain: we are in $root, the 1.97.1 override is active.
if (((rustup target list --installed) -join "`n") -match "aarch64-pc-windows-msvc") {
    Ok "aarch64-pc-windows-msvc already present"
}
else {
    rustup target add aarch64-pc-windows-msvc
    Ok "arm64 target added to the repository's toolchain"
}
if ($CrossArm64Check) {
    Info "Proof of the arm64 cross-build (cargo build -p wind-desktop --target aarch64-pc-windows-msvc) ..."
    cargo build -p wind-desktop --target aarch64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw "arm64 cross-build failed (C++ ARM64 component missing?)" }
    Ok "arm64 cross-build OK"
}
else {
    Info "Cross-build proof not played (option -CrossArm64Check)."
    Info "LINKING arm64 requires the VS component 'MSVC v143 C++ ARM64 build tools'."
}

Section "Verification"
Update-SessionPath
function Show($label, $cmd) {
    try {
        $v = (& ([scriptblock]::Create($cmd)) 2>$null | Select-Object -First 1)
        Ok ("{0,-8} {1}" -f $label, $v)
    }
    catch { Warn "$label not found" }
}
Show "git" "git --version"
Show "rustc" "rustc --version"
Show "cargo" "cargo --version"
Show "node" "node --version"
Show "npm" "npm --version"
Show "python" "python --version"
Show "gh" "gh --version"
Show "tauri" "cargo tauri --version"

$node = ((node --version) 2>$null)
if ($node -notmatch "^v24\.") {
    Warn "Node = $node (the CI pins v24). If an e2e journey diverges, install 24."
}

Section "Done"
Write-Host "Workstation ready for the cargo + e2e cycle." -ForegroundColor Green
Write-Host ""
Write-Host "In your hands (the script touches no secret):"
Write-Host "  1. User-level OAuth credentials -- a DEV WORKSTATION gesture only"
Write-Host "     (a public release embeds its own at build time, D1 PLAN-RETOURS-9;"
Write-Host "     these variables remain needed for a dev build and for make-release.ps1"
Write-Host "     which maps them to WIND_RELEASE_*) -- values to take from the current machine:"
Write-Host "       setx GOOGLE_CLIENT_ID     ..."
Write-Host "       setx GOOGLE_CLIENT_SECRET ..."
Write-Host "       setx MICROSOFT_CLIENT_ID  ..."
Write-Host "  2. Replay the full gate to validate the toolchain:"
Write-Host "       powershell -ExecutionPolicy Bypass -File scripts\gate.ps1"
Write-Host ""
Write-Host "Bi-arch release (x64 + arm64): DONE (PLAN-RETOURS-8, ADR 0023)."
Write-Host "make-release.ps1 builds both channels and latest.json carries both"
Write-Host "platform keys; scripted verification by verify-release.ps1."
