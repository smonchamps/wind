#Requires -Version 5.1
<#
  gate.ps1 -- la gate complete de Wind en UN appel (PLAN-KAIZEN-CLAUDE
  vague 2, E2). Dix etapes, l'ordre du hook pre-push (echouer tot),
  fail-fast, et AUCUNE redirection vers le neant : le verdict chiffre de
  chaque etape (tests, paires, avertissements) doit sortir.

  Ecrit SANS ACCENTS -- convention des .ps1 du depot (piege d'encodage
  PowerShell 5.1 sur les sources UTF-8, cf. faire-release.ps1).

  Usage :  powershell -ExecutionPolicy Bypass -File scripts\gate.ps1
#>
param(
    # Le raccourci documentaire du hook pre-push (PLAN-KAIZEN vague 2,
    # E4) : un diff docs/** + *.md (hors docs/design/**) ne joue que les
    # etapes en secondes — clippy, tests et e2e ne peuvent rien attraper.
    [switch]$DocsSeulement
)
$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $root

$chrono = [System.Diagnostics.Stopwatch]::StartNew()
$rapport = @()

function Etape($no, $titre, [scriptblock]$corps) {
    Write-Host ""
    Write-Host "[$no/10] $titre" -ForegroundColor Cyan
    $t = [System.Diagnostics.Stopwatch]::StartNew()
    # try/catch : une exception terminante (commande absente du PATH,
    # Push-Location rate) doit produire le MEME verdict rouge nomme
    # qu'un code de sortie non nul -- jamais une pile brute anonyme.
    try { & $corps }
    catch {
        Write-Host $_.Exception.Message -ForegroundColor Red
        Write-Host ""
        Write-Host "GATE ROUGE a l'etape [$no/10] $titre (exception)" -ForegroundColor Red
        exit 1
    }
    if ($LASTEXITCODE -ne 0) {
        Write-Host ""
        Write-Host "GATE ROUGE a l'etape [$no/10] $titre (code $LASTEXITCODE)" -ForegroundColor Red
        exit 1
    }
    $t.Stop()
    $script:rapport += ("  [{0}/10] {1,-28} {2,7:n1} s" -f $no, $titre, $t.Elapsed.TotalSeconds)
}

Etape 1 "format" { cargo fmt --all -- --check }

Etape 2 "build ui-v2" {
    Push-Location (Join-Path $root "apps\desktop\ui-v2")
    # « Zero avertissement exige » n'etait verifie par personne : seul le
    # code de sortie comptait, et un avertissement a11y de
    # vite-plugin-svelte a traverse deux gates vertes le 2026-09-01
    # (A105). Un avertissement du plugin = rouge, comme un warning clippy.
    try {
        $sortie = & npm run build 2>&1 | ForEach-Object { "$_" }
        $sortie | ForEach-Object { Write-Host $_ }
        if ($LASTEXITCODE -eq 0 -and ($sortie -match '\[vite-plugin-svelte\]')) {
            Write-Host "avertissement vite-plugin-svelte = rouge (zero avertissement exige)" -ForegroundColor Red
            $global:LASTEXITCODE = 1
        }
    } finally { Pop-Location }
}

Etape 3 "contrastes WCAG (A8)" { node e2e/contraste.mjs }

Etape 4 "coherence du Systeme (DC-D6)" { node e2e/coherence-systeme.mjs }

Etape 5 "garde du thread principal" { node e2e/garde-thread-principal.mjs }

# PLAN-AUDIT-V2 E9 : aucun lint JS ne gardait les scripts d'outillage —
# une faute de syntaxe dans une gate textuelle se decouvrait en la
# jouant. `node --check` coute 0,2 s.
Etape 6 "syntaxe des scripts (node --check, parser PowerShell)" {
    $scripts = @(Get-ChildItem e2e -Filter *.mjs) + @(Get-ChildItem scripts -Filter *.mjs)
    foreach ($f in $scripts) {
        & node --check $f.FullName
        if ($LASTEXITCODE -ne 0) { Write-Host "syntaxe : $($f.Name)" -ForegroundColor Red; return }
    }
    # Terrain 2026-09-02 : verifier-release.ps1 ne parsait plus sous
    # PowerShell 5.1 -- un tiret cadratin dans une chaine d'un fichier
    # SANS BOM, lu en ANSI, devient un guillemet fermant. L'analyseur de
    # CE PowerShell (celui du CE) lit chaque .ps1 tel qu'il le lira au
    # terrain ; un .ps1 non ASCII porte un BOM UTF-8 ou ne passe pas.
    $ps1 = @(Get-ChildItem scripts -Filter *.ps1) + @(Get-ChildItem e2e -Filter *.ps1)
    foreach ($f in $ps1) {
        $erreurs = $null
        $null = [System.Management.Automation.Language.Parser]::ParseFile($f.FullName, [ref]$null, [ref]$erreurs)
        if ($erreurs.Count -gt 0) {
            Write-Host "parse PowerShell : $($f.Name) : $($erreurs[0].Message)" -ForegroundColor Red
            return
        }
    }
}

if ($DocsSeulement) {
    $chrono.Stop()
    Write-Host ""
    Write-Host "Diff documentaire seul : etapes 7-10 sautees. Gate documentaire VERTE en $([math]::Round($chrono.Elapsed.TotalSeconds)) s." -ForegroundColor Green
    $rapport | ForEach-Object { Write-Host $_ }
    exit 0
}

Etape 7 "clippy (warnings = erreurs)" { cargo clippy --workspace --all-targets -- -D warnings }

# --all-targets n'est PAS decoratif : sans lui, cargo ignore les tests
# des EXEMPLES (diagnostics du terrain, crates/mail-core/examples/).
Etape 8 "tests Rust (--all-targets)" { cargo test --workspace --all-targets }

Etape 9 "tests Rust (--doc)" { cargo test --workspace --doc }

Etape 10 "e2e (fenetre pilotee CDP)" {
    Push-Location (Join-Path $root "e2e")
    try { npm test } finally { Pop-Location }
}

# Le compte des flaky du run (D4) : consigne au verdict, jamais un rouge.
$flaky = & node e2e/flaky.mjs
$chrono.Stop()
Write-Host ""
Write-Host "Gate complete VERTE en $([math]::Round($chrono.Elapsed.TotalMinutes, 1)) min ($([math]::Round($chrono.Elapsed.TotalSeconds)) s) :" -ForegroundColor Green
$rapport | ForEach-Object { Write-Host $_ }
$flaky | ForEach-Object { Write-Host "  $_" }
