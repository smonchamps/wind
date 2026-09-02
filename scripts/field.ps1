#Requires -Version 5.1
<#
  field.ps1 -- the state of the workstation for a field pass, in one call
  (PLAN-KAIZEN-CLAUDE wave 2, E5). Answers the questions asked at EVERY
  STOP 2 without regenerating one-liners:
    - where the database stands (wind.db + -wal + -shm, sizes);
    - which version of Wind is installed;
    - whether the OAuth credentials are set;
    - which field trace already exists at the repository root.

  Read-only: this script touches nothing.

  Written in ASCII -- convention of the repository's .ps1 files
  (Windows PowerShell 5.1 encoding trap).

  Usage:  powershell -ExecutionPolicy Bypass -File scripts\field.ps1
#>
$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

function Line($label, $value) { Write-Host ("  {0,-34} {1}" -f $label, $value) }
function Size($bytes) {
    if ($bytes -ge 1GB) { return ("{0:n2} GB" -f ($bytes / 1GB)) }
    if ($bytes -ge 1MB) { return ("{0:n1} MB" -f ($bytes / 1MB)) }
    return ("{0:n0} B" -f $bytes)
}

Write-Host "=== Database (%APPDATA%\dev.elements.wind) ===" -ForegroundColor Cyan
$folder = Join-Path $env:APPDATA "dev.elements.wind"
foreach ($name in "wind.db", "wind.db-wal", "wind.db-shm") {
    $f = Join-Path $folder $name
    if (Test-Path $f) { Line $name (Size (Get-Item $f).Length) }
    else { Line $name "absent" }
}

Write-Host "=== Installed application ===" -ForegroundColor Cyan
$key = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Wind"
if (Test-Path $key) {
    $p = Get-ItemProperty $key
    Line "installed version" $p.DisplayVersion
}
else {
    Line "installed version" "not found in the registry (Uninstall\Wind key)"
}
$exe = Join-Path $env:LOCALAPPDATA "Wind\wind-desktop.exe"
if (Test-Path $exe) {
    Line "wind-desktop.exe" ((Get-Item $exe).VersionInfo.ProductVersion)
}

Write-Host "=== OAuth credentials (user or session) ===" -ForegroundColor Cyan
# Same double read as run-wind.ps1: User level (setx) OR the current
# session -- a variable set in the session works for a launch from that
# session, the report must not say ABSENT.
foreach ($v in "GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET", "MICROSOFT_CLIENT_ID") {
    $persisted = [Environment]::GetEnvironmentVariable($v, "User")
    $session = (Get-Item "env:$v" -ErrorAction SilentlyContinue).Value
    if (-not [string]::IsNullOrWhiteSpace($persisted)) { Line $v "set ($($persisted.Length) chars, setx)" }
    elseif (-not [string]::IsNullOrWhiteSpace($session)) { Line $v "set ($($session.Length) chars, session only)" }
    else { Line $v "ABSENT (setx $v ...)" }
}

Write-Host "=== Field traces at the repository root ===" -ForegroundColor Cyan
$traces = Get-ChildItem (Join-Path $root "*.log") -ErrorAction SilentlyContinue
if ($traces) {
    foreach ($t in $traces) { Line $t.Name ("{0} - {1:yyyy-MM-dd HH:mm}" -f (Size $t.Length), $t.LastWriteTime) }
}
else {
    Line "(none)" "run scripts\run-wind.ps1 to produce one"
}
