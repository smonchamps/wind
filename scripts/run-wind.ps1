#Requires -Version 5.1
<#
  run-wind.ps1 -- runs Wind from the sources WITH its traces
  (PLAN-KAIZEN-CLAUDE wave 2, E5). The field one-liner, committed once
  and for all -- it encodes the STANDARD section 9 trap:

    the --release exe is a *windows* subsystem app (no console); started
    BARE with `2> file`, PowerShell does not wait and the file stays
    empty FOREVER. The only launcher that traces in release is
    `cargo run` (console app, holds the stderr handle to the end).

  The trace is written at the REPOSITORY ROOT (never on the Desktop: it
  is redirected under OneDrive and the classic path does not exist --
  trap paid, PLAN-RETOURS-3), in UTF-8.

  Written in ASCII -- convention of the repository's .ps1 files
  (Windows PowerShell 5.1 encoding trap).

  Usage:
    powershell -ExecutionPolicy Bypass -File scripts\run-wind.ps1
    ... -DebugBuild     debug build (traces + fast to build, inflated CPU durations)
    ... -Trace x.log    trace file name (default trace-terrain.log)
#>
param(
    [switch]$DebugBuild,
    [string]$Trace = "trace-terrain.log"
)
$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $root

# Without the user-level OAuth credentials, connecting an account fails
# (see install-workstation.ps1, step 1). Warn BEFORE launching -- no
# stop: offline reading works without them.
foreach ($v in "GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET", "MICROSOFT_CLIENT_ID") {
    if ([string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($v, "User")) -and
        [string]::IsNullOrWhiteSpace((Get-Item "env:$v" -ErrorAction SilentlyContinue).Value)) {
        Write-Host "  !!  $v missing: account connection will fail (setx $v ...)" -ForegroundColor Yellow
    }
}

$traceAbs = Join-Path $root $Trace
$profile = if ($DebugBuild) { "debug" } else { "release" }

# Build BEFORE launching, through the single home of the rebuild traps
# (e2e/rebuild-v2.mjs): without it, `generate_context!` does not re-embed
# a changed ui-v2 dist and the Chief Engineer would validate a stale UI in the field
# (STANDARD section 9 trap, release side).
Write-Host "Building ($profile): ui-v2 + dist re-embedded if changed ..."
if ($DebugBuild) { node (Join-Path $root "scripts\build-wind.mjs") --debug }
else { node (Join-Path $root "scripts\build-wind.mjs") }
if ($LASTEXITCODE -ne 0) { throw "build failed (code $LASTEXITCODE)" }

Write-Host "Launching Wind ($profile), trace -> $traceAbs"
Write-Host "(cargo holds the handle: the window closes, the script returns)"

# PS 5.1 writes `2>` as UTF-16 through its pipeline; go through cmd for a
# raw byte redirection -- the app's trace is already UTF-8.
$args = "run -p wind-desktop" + $(if (-not $DebugBuild) { " --release" } else { "" })
cmd /c "cargo $args 2> `"$traceAbs`""
if ($LASTEXITCODE -ne 0) { Write-Host "  !!  exit code $LASTEXITCODE" -ForegroundColor Yellow }

if (Test-Path $traceAbs) {
    $size = (Get-Item $traceAbs).Length
    Write-Host "Trace: $size bytes. Last lines:"
    Get-Content $traceAbs -Tail 10 -Encoding UTF8
}
