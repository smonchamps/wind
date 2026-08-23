#Requires -Version 5.1
<#
  terrain.ps1 -- l'etat du poste pour une passe terrain, en un appel
  (PLAN-KAIZEN-CLAUDE vague 2, E5). Repond aux questions posees a
  CHAQUE STOP 2 sans regenerer de one-liners :
    - ou en est la base (wind.db + -wal + -shm, tailles) ;
    - quelle version de Wind est installee ;
    - les identifiants OAuth sont-ils poses ;
    - quelle trace terrain existe deja a la racine du depot.

  Lecture seule : ce script ne touche a rien.

  Ecrit SANS ACCENTS -- convention des .ps1 du depot (piege d'encodage
  PowerShell 5.1).

  Usage :  powershell -ExecutionPolicy Bypass -File scripts\terrain.ps1
#>
$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path

function Ligne($label, $valeur) { Write-Host ("  {0,-34} {1}" -f $label, $valeur) }
function Poids($octets) {
    if ($octets -ge 1GB) { return ("{0:n2} Go" -f ($octets / 1GB)) }
    if ($octets -ge 1MB) { return ("{0:n1} Mo" -f ($octets / 1MB)) }
    return ("{0:n0} o" -f $octets)
}

Write-Host "=== Base de donnees (%APPDATA%\dev.elements.wind) ===" -ForegroundColor Cyan
$dossier = Join-Path $env:APPDATA "dev.elements.wind"
foreach ($nom in "wind.db", "wind.db-wal", "wind.db-shm") {
    $f = Join-Path $dossier $nom
    if (Test-Path $f) { Ligne $nom (Poids (Get-Item $f).Length) }
    else { Ligne $nom "absent" }
}

Write-Host "=== Application installee ===" -ForegroundColor Cyan
$cle = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Wind"
if (Test-Path $cle) {
    $p = Get-ItemProperty $cle
    Ligne "version installee" $p.DisplayVersion
}
else {
    Ligne "version installee" "introuvable au registre (cle Uninstall\Wind)"
}
$exe = Join-Path $env:LOCALAPPDATA "Wind\wind-desktop.exe"
if (Test-Path $exe) {
    Ligne "wind-desktop.exe" ((Get-Item $exe).VersionInfo.ProductVersion)
}

Write-Host "=== Identifiants OAuth (utilisateur ou session) ===" -ForegroundColor Cyan
# Meme double lecture que lancer-wind.ps1 : niveau User (setx) OU la
# session courante -- une variable posee en session marche pour un
# lancement depuis cette session, le rapport ne doit pas dire ABSENT.
foreach ($v in "GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET", "MICROSOFT_CLIENT_ID") {
    $persistee = [Environment]::GetEnvironmentVariable($v, "User")
    $session = (Get-Item "env:$v" -ErrorAction SilentlyContinue).Value
    if (-not [string]::IsNullOrWhiteSpace($persistee)) { Ligne $v "pose ($($persistee.Length) car., setx)" }
    elseif (-not [string]::IsNullOrWhiteSpace($session)) { Ligne $v "pose ($($session.Length) car., session seule)" }
    else { Ligne $v "ABSENT (setx $v ...)" }
}

Write-Host "=== Traces terrain a la racine du depot ===" -ForegroundColor Cyan
$traces = Get-ChildItem (Join-Path $root "*.log") -ErrorAction SilentlyContinue
if ($traces) {
    foreach ($t in $traces) { Ligne $t.Name ("{0} - {1:yyyy-MM-dd HH:mm}" -f (Poids $t.Length), $t.LastWriteTime) }
}
else {
    Ligne "(aucune)" "lancer scripts\lancer-wind.ps1 pour en produire une"
}
