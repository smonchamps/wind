#Requires -Version 5.1
<#
  gate.ps1 -- Wind's full gate in ONE call (PLAN-KAIZEN-CLAUDE wave 2,
  E2). Thirteen steps, the order of the pre-push hook (fail early),
  fail-fast, and NO redirection into the void: the figured verdict of
  each step (tests, pairs, warnings) must come out.

  Written in ASCII -- convention of the repository's .ps1 files
  (Windows PowerShell 5.1 encoding trap on UTF-8 sources, see
  verify-release.ps1).

  Usage:  powershell -ExecutionPolicy Bypass -File scripts\gate.ps1
#>
param(
    # The docs-only shortcut of the pre-push hook (PLAN-KAIZEN wave 2,
    # E4): a diff of docs/** + *.md (outside docs/design/**) plays only
    # the steps measured in seconds -- clippy, tests and e2e can catch
    # nothing there.
    [switch]$DocsOnly
)
$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $root

$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
$report = @()

function Step($no, $title, [scriptblock]$body) {
    Write-Host ""
    Write-Host "[$no/13] $title" -ForegroundColor Cyan
    $t = [System.Diagnostics.Stopwatch]::StartNew()
    # try/catch: a terminating exception (command absent from the PATH,
    # failed Push-Location) must produce the SAME named red verdict as a
    # non-zero exit code -- never an anonymous raw stack.
    try { & $body }
    catch {
        Write-Host $_.Exception.Message -ForegroundColor Red
        Write-Host ""
        Write-Host "GATE RED at step [$no/13] $title (exception)" -ForegroundColor Red
        exit 1
    }
    if ($LASTEXITCODE -ne 0) {
        Write-Host ""
        Write-Host "GATE RED at step [$no/13] $title (code $LASTEXITCODE)" -ForegroundColor Red
        exit 1
    }
    $t.Stop()
    $script:report += ("  [{0}/13] {1,-28} {2,7:n1} s" -f $no, $title, $t.Elapsed.TotalSeconds)
}

Step 1 "format" { cargo fmt --all -- --check }

Step 2 "build ui-v2" {
    Push-Location (Join-Path $root "apps\desktop\ui-v2")
    # "Zero warnings required" was checked by nobody: only the exit code
    # counted, and an a11y warning of vite-plugin-svelte went through two
    # green gates on 2026-09-01 (A105). A plugin warning = red, like a
    # clippy warning.
    try {
        $output = & npm run build 2>&1 | ForEach-Object { "$_" }
        $output | ForEach-Object { Write-Host $_ }
        if ($LASTEXITCODE -eq 0 -and ($output -match '\[vite-plugin-svelte\]')) {
            Write-Host "vite-plugin-svelte warning = red (zero warnings required)" -ForegroundColor Red
            $global:LASTEXITCODE = 1
        }
        # PLAN-BASCULE-ANGLAIS E1c (measured 2026-09-02): eslint no-undef,
        # 3 s, 0 pre-existing errors, catches a missed rename in .svelte and
        # .js alike; svelte-check rejected (1 059 pre-existing errors).
        if ($LASTEXITCODE -eq 0) { & npm run lint }
    } finally { Pop-Location }
}

Step 3 "WCAG contrasts (A8)" { node e2e/contraste.mjs }

Step 4 "System coherence (DC-D6)" { node e2e/coherence-systeme.mjs }

Step 5 "main-thread guard" { node e2e/garde-thread-principal.mjs }

# PLAN-AUDIT-V2 E9: no JS lint guarded the tooling scripts -- a syntax
# error in a textual gate was discovered by playing it. `node --check`
# costs 0.2 s.
Step 6 "script syntax (node --check, PowerShell parser)" {
    $scripts = @(Get-ChildItem e2e -Filter *.mjs) + @(Get-ChildItem scripts -Filter *.mjs)
    foreach ($f in $scripts) {
        & node --check $f.FullName
        if ($LASTEXITCODE -ne 0) { Write-Host "syntax: $($f.Name)" -ForegroundColor Red; return }
    }
    # Field 2026-09-02: verify-release.ps1 no longer parsed under Windows
    # PowerShell 5.1 -- an em dash in a string of a file WITHOUT BOM, read
    # as ANSI, became a closing quote. THIS PowerShell's parser (the CE's)
    # reads every .ps1 the way it will read it in the field; a non-ASCII
    # .ps1 carries a UTF-8 BOM or does not pass.
    $ps1 = @(Get-ChildItem scripts -Filter *.ps1) + @(Get-ChildItem e2e -Filter *.ps1)
    foreach ($f in $ps1) {
        $errors = $null
        $null = [System.Management.Automation.Language.Parser]::ParseFile($f.FullName, [ref]$null, [ref]$errors)
        if ($errors.Count -gt 0) {
            Write-Host "PowerShell parse: $($f.Name): $($errors[0].Message)" -ForegroundColor Red
            return
        }
    }
}

# PLAN-BASCULE-ANGLAIS E1: three textual nets, in seconds, played on the
# docs-only path too. The language ratchet refuses any RISE of French per
# file (e2e/language-baseline.json, lowered at each step with --update);
# the IPC contract compares generate_handler!, the #[tauri::command] fns
# and the appel('...') of the UI; every relative markdown link must resolve.
Step 7 "language ratchet" { node e2e/language-gate.mjs }

Step 8 "IPC contract (Tauri commands)" { node e2e/ipc-contract.mjs }

Step 9 "markdown links" { node e2e/docs-links.mjs }

if ($DocsOnly) {
    $stopwatch.Stop()
    Write-Host ""
    Write-Host "Docs-only diff: steps 10-13 skipped. Documentary gate GREEN in $([math]::Round($stopwatch.Elapsed.TotalSeconds)) s." -ForegroundColor Green
    $report | ForEach-Object { Write-Host $_ }
    exit 0
}

Step 10 "clippy (warnings = errors)" { cargo clippy --workspace --all-targets -- -D warnings }

# --all-targets is NOT decorative: without it, cargo ignores the tests of
# the EXAMPLES (field diagnostics, crates/mail-core/examples/).
Step 11 "Rust tests (--all-targets)" { cargo test --workspace --all-targets }

Step 12 "Rust tests (--doc)" { cargo test --workspace --doc }

Step 13 "e2e (CDP-driven window)" {
    Push-Location (Join-Path $root "e2e")
    try { npm test } finally { Pop-Location }
}

# The run's flaky count (D4): recorded in the verdict, never a red.
$flaky = & node e2e/flaky.mjs
$stopwatch.Stop()
Write-Host ""
Write-Host "Full gate GREEN in $([math]::Round($stopwatch.Elapsed.TotalMinutes, 1)) min ($([math]::Round($stopwatch.Elapsed.TotalSeconds)) s):" -ForegroundColor Green
$report | ForEach-Object { Write-Host $_ }
$flaky | ForEach-Object { Write-Host "  $_" }
