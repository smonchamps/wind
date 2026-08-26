# Banc de demarrage — exploration 2026-08-26.
#
# Lance N fois le binaire compile avec la feature `mesure` et recolte les
# spans d'amont (wry::window::create, wry::webview::create, wry::window::draw)
# ecrits sur stderr par le collecteur tracing.
#
# Precautions (pieges nommes au rapport) :
#   - zombies : aucun wind-desktop ni msedgewebview2 ne doit survivre d'un
#     run a l'autre, sinon l'environnement WebView2 se cree en quasi-zero ;
#   - base isolee : WIND_DB_PATH pointe une base de banc, la vraie base de
#     12,8 Go n'est jamais ouverte, et aucun compte n'est connecte (pas de
#     reseau, pas de synchro) ;
#   - chauffe : un run jete AVANT les runs mesures.
#
# Pas de CDP, pas de Playwright : les chiffres ne portent donc pas le
# surcout inconnu de --remote-debugging-port.

param(
  [int]$N = 10,
  [string]$Racine = "C:\Users\smonc\OneDrive\Documents\Repositories\wind",
  [string]$Sortie = "C:\Users\smonc\AppData\Local\Temp\claude\C--Users-smonc-OneDrive-Documents-Repositories-wind\213210b5-d1a2-46bb-b5c9-00bc33779fdf\scratchpad\runs"
)

$bin = Join-Path $Racine "target\release\wind-desktop.exe"
if (-not (Test-Path $bin)) { throw "binaire absent : $bin" }

New-Item -ItemType Directory -Force -Path $Sortie | Out-Null
Get-ChildItem $Sortie -Filter "run-*.log" -ErrorAction SilentlyContinue | Remove-Item -Force

$env:WIND_DB_PATH = Join-Path $Sortie "banc.db"

function Stop-Zombies {
  Get-Process -Name wind-desktop, msedgewebview2 -ErrorAction SilentlyContinue |
    Stop-Process -Force -ErrorAction SilentlyContinue
}

function Invoke-Run {
  param([string]$Log)
  Stop-Zombies
  Start-Sleep -Milliseconds 700
  if (Test-Path $Log) { Remove-Item $Log -Force }
  $p = Start-Process -FilePath $bin -PassThru -WindowStyle Minimized `
        -RedirectStandardError $Log -RedirectStandardOutput "$Log.out"
  # On attend la FERMETURE du span de premiere trame : tout ce qu'on
  # mesure est alors ecrit. Sondage sur le fichier, pas sur une duree.
  $limite = (Get-Date).AddSeconds(90)
  $vu = $false
  while ((Get-Date) -lt $limite) {
    if (Test-Path $Log) {
      $texte = Get-Content $Log -Raw -ErrorAction SilentlyContinue
      if ($texte -and $texte -match 'window::draw.*close') { $vu = $true; break }
    }
    Start-Sleep -Milliseconds 100
  }
  # Laisser retomber la premiere salve d'IPC (lang_get, migration_check,
  # la rafale de l'onMount) : ce sont des `info_span!` de wry, ils donnent
  # la file du verrou global sans rien coder.
  Start-Sleep -Milliseconds 2500
  Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
  Stop-Zombies
  Start-Sleep -Milliseconds 400
  return $vu
}

Write-Output "chauffe (run jete, cree la base de banc)..."
$null = Invoke-Run (Join-Path $Sortie "chauffe.log")

for ($i = 1; $i -le $N; $i++) {
  $log = Join-Path $Sortie ("run-{0:d2}.log" -f $i)
  $ok = Invoke-Run $log
  Write-Output ("run {0:d2} : {1}" -f $i, $(if ($ok) { "spans recoltes" } else { "TIMEOUT — span de trame jamais vu" }))
}

Stop-Zombies
Write-Output "---"
Write-Output "logs : $Sortie"
