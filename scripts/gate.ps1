#Requires -Version 5.1
<#
  gate.ps1 -- la gate complete de Wind en UN appel (PLAN-KAIZEN-CLAUDE
  vague 2, E2). Neuf etapes, l'ordre du hook pre-push (echouer tot),
  fail-fast, et AUCUNE redirection vers le neant : le verdict chiffre de
  chaque etape (tests, paires, avertissements) doit sortir.

  Ecrit SANS ACCENTS -- convention des .ps1 du depot (piege d'encodage
  PowerShell 5.1 sur les sources UTF-8, cf. faire-release.ps1).

  Usage :  powershell -ExecutionPolicy Bypass -File scripts\gate.ps1
#>
$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $root

$chrono = [System.Diagnostics.Stopwatch]::StartNew()
$rapport = @()

function Etape($no, $titre, [scriptblock]$corps) {
    Write-Host ""
    Write-Host "[$no/9] $titre" -ForegroundColor Cyan
    $t = [System.Diagnostics.Stopwatch]::StartNew()
    # try/catch : une exception terminante (commande absente du PATH,
    # Push-Location rate) doit produire le MEME verdict rouge nomme
    # qu'un code de sortie non nul -- jamais une pile brute anonyme.
    try { & $corps }
    catch {
        Write-Host $_.Exception.Message -ForegroundColor Red
        Write-Host ""
        Write-Host "GATE ROUGE a l'etape [$no/9] $titre (exception)" -ForegroundColor Red
        exit 1
    }
    if ($LASTEXITCODE -ne 0) {
        Write-Host ""
        Write-Host "GATE ROUGE a l'etape [$no/9] $titre (code $LASTEXITCODE)" -ForegroundColor Red
        exit 1
    }
    $t.Stop()
    $script:rapport += ("  [{0}/9] {1,-28} {2,7:n1} s" -f $no, $titre, $t.Elapsed.TotalSeconds)
}

Etape 1 "format" { cargo fmt --all -- --check }

Etape 2 "build ui-v2" {
    Push-Location (Join-Path $root "apps\desktop\ui-v2")
    try { npm run build } finally { Pop-Location }
}

Etape 3 "contrastes WCAG (A8)" { node e2e/contraste.mjs }

Etape 4 "coherence du Systeme (DC-D6)" { node e2e/coherence-systeme.mjs }

Etape 5 "garde du thread principal" { node e2e/garde-thread-principal.mjs }

Etape 6 "clippy (warnings = erreurs)" { cargo clippy --workspace --all-targets -- -D warnings }

# --all-targets n'est PAS decoratif : sans lui, cargo ignore les tests
# des EXEMPLES (diagnostics du terrain, crates/mail-core/examples/).
Etape 7 "tests Rust (--all-targets)" { cargo test --workspace --all-targets }

Etape 8 "tests Rust (--doc)" { cargo test --workspace --doc }

Etape 9 "e2e (fenetre pilotee CDP)" {
    Push-Location (Join-Path $root "e2e")
    try { npm test } finally { Pop-Location }
}

$chrono.Stop()
Write-Host ""
Write-Host "Gate complete VERTE en $([math]::Round($chrono.Elapsed.TotalMinutes, 1)) min ($([math]::Round($chrono.Elapsed.TotalSeconds)) s) :" -ForegroundColor Green
$rapport | ForEach-Object { Write-Host $_ }
