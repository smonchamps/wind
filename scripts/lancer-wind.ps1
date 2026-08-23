#Requires -Version 5.1
<#
  lancer-wind.ps1 -- lance Wind depuis les sources AVEC ses traces
  (PLAN-KAIZEN-CLAUDE vague 2, E5). Le one-liner du terrain, verse une
  fois pour toutes -- il encode le piege STANDARD section 9 :

    l'exe --release est sous-systeme *windows* (aucune console) ; lance
    NU avec `2> fichier`, PowerShell n'attend pas et le fichier reste
    vide A JAMAIS. Le seul lanceur qui trace en release est `cargo run`
    (appli console, tient le handle stderr jusqu'au bout).

  La trace s'ecrit a la RACINE DU DEPOT (jamais sur le Bureau : il est
  redirige sous OneDrive et le chemin classique n'existe pas -- piege
  paye, PLAN-RETOURS-3), en UTF-8.

  Ecrit SANS ACCENTS -- convention des .ps1 du depot (piege d'encodage
  PowerShell 5.1).

  Usage :
    powershell -ExecutionPolicy Bypass -File scripts\lancer-wind.ps1
    ... -DebugBuild     build debug (traces + rapide a batir, durees CPU gonflees)
    ... -Trace x.log    nom du fichier de trace (defaut trace-terrain.log)
#>
param(
    [switch]$DebugBuild,
    [string]$Trace = "trace-terrain.log"
)
$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $root

# Sans les identifiants OAuth au niveau utilisateur, la connexion d'un
# compte echoue (voir installer-poste.ps1, etape 1). On previent AVANT
# le lancement -- pas d'arret : la lecture hors-ligne marche sans eux.
foreach ($v in "GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET", "MICROSOFT_CLIENT_ID") {
    if ([string]::IsNullOrWhiteSpace([Environment]::GetEnvironmentVariable($v, "User")) -and
        [string]::IsNullOrWhiteSpace((Get-Item "env:$v" -ErrorAction SilentlyContinue).Value)) {
        Write-Host "  !!  $v absent : la connexion de compte echouera (setx $v ...)" -ForegroundColor Yellow
    }
}

$traceAbs = Join-Path $root $Trace
$profil = if ($DebugBuild) { "debug" } else { "release" }

# Construire AVANT de lancer, par la maison unique des pieges du rebuild
# (e2e/rebuild-v2.mjs) : sans elle, `generate_context!` ne re-embarque
# pas un dist ui-v2 change et le CE validerait au terrain une UI perimee
# (piege STANDARD section 9, cote release).
Write-Host "Construction ($profil) : ui-v2 + dist re-embarque si change ..."
if ($DebugBuild) { node (Join-Path $root "scripts\construire-wind.mjs") --debug }
else { node (Join-Path $root "scripts\construire-wind.mjs") }
if ($LASTEXITCODE -ne 0) { throw "construction echouee (code $LASTEXITCODE)" }

Write-Host "Lancement de Wind ($profil), trace -> $traceAbs"
Write-Host "(cargo tient le handle : la fenetre se ferme, le script rend la main)"

# PS 5.1 ecrit `2>` en UTF-16 via son pipeline ; on passe par cmd pour
# une redirection d'octets brute -- la trace de l'app est deja UTF-8.
$args = "run -p wind-desktop" + $(if (-not $DebugBuild) { " --release" } else { "" })
cmd /c "cargo $args 2> `"$traceAbs`""
if ($LASTEXITCODE -ne 0) { Write-Host "  !!  sortie code $LASTEXITCODE" -ForegroundColor Yellow }

if (Test-Path $traceAbs) {
    $taille = (Get-Item $traceAbs).Length
    Write-Host "Trace : $taille octets. Dernieres lignes :"
    Get-Content $traceAbs -Tail 10 -Encoding UTF8
}
