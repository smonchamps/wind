# Banc de demarrage sur DECOR REEL — exploration 2026-08-26.
#
# Meme binaire instrumente que banc-demarrage.ps1, mais WIND_DB_PATH pointe
# une COPIE de la base du terrain (12,84 Go, 251 524 enveloppes, 11,4 Go de
# corps). La base de l'utilisateur n'est jamais ouverte.
#
# Ce qu'on vient chercher ici, que le decor vide ne pouvait pas donner :
#   - la duree REELLE des commandes, lue par CHAINAGE dans le fil des
#     requetes de transport. Le front n'emet la requete suivante qu'a la
#     reponse de la precedente, donc l'ecart entre deux maillons EST la
#     latence du premier :
#       lang_get -> migration_check        (lang_get + montage)
#       migration_check -> nav_snapshot    (migration_check)
#       list_category -> category_total    (la page, avec la penalite preview)
#       backfill_status -> backfill_bodies (le balayage des 64 boites)
#       connect_accounts -> sync_inbox     (trousseau + OAuth + IMAP)
#   - l'ecart entre le decor semé et le terrain.
#
# Fenetre d'observation LONGUE : backfill_status ne part qu'a t+3 s et peut
# tenir des dizaines de secondes. On ne coupe pas avant.

param(
  [int]$N = 10,
  [int]$FenetreSecondes = 45,
  [string]$Racine = "C:\Users\smonc\OneDrive\Documents\Repositories\wind",
  [string]$Sortie = "C:\Users\smonc\AppData\Local\Temp\claude\C--Users-smonc-OneDrive-Documents-Repositories-wind\213210b5-d1a2-46bb-b5c9-00bc33779fdf\scratchpad"
)

$bin  = Join-Path $Racine "target\release\wind-desktop.exe"
$base = Join-Path $Sortie "reel\wind.db"
$logs = Join-Path $Sortie "reels"
if (-not (Test-Path $bin))  { throw "binaire absent : $bin" }
if (-not (Test-Path $base)) { throw "copie de la base absente : $base" }
New-Item -ItemType Directory -Force -Path $logs | Out-Null

$env:WIND_DB_PATH = $base

function Stop-Zombies {
  Get-Process -Name wind-desktop, msedgewebview2 -EA SilentlyContinue |
    Where-Object { $_.Path -like "*wind*" -or $_.Name -eq "wind-desktop" } |
    Stop-Process -Force -EA SilentlyContinue
  Get-Process -Name wind-desktop -EA SilentlyContinue | Stop-Process -Force -EA SilentlyContinue
}

for ($i = 1; $i -le $N; $i++) {
  Stop-Zombies
  Start-Sleep -Milliseconds 800
  $log = Join-Path $logs ("reel-{0:d2}.log" -f $i)
  $t0 = Get-Date
  $p = Start-Process -FilePath $bin -PassThru -WindowStyle Minimized `
        -RedirectStandardError $log -RedirectStandardOutput ($log + ".out")
  Start-Sleep -Seconds $FenetreSecondes
  Stop-Process -Id $p.Id -Force -EA SilentlyContinue
  Stop-Zombies
  # $lignes et NON $n : en PowerShell les variables sont insensibles a la
  # casse, donc $n EST $N — la borne de la boucle. Ecrire le nombre de
  # lignes du journal dans $n reprogrammait la boucle pour ~550 tours.
  # Defaut paye le 2026-08-26 : un -N 3 a produit 14 runs avant qu'on ne
  # l'arrete, et c'est aussi ce qui explique les 19 journaux de la
  # campagne du 26/08, lancee avec le defaut -N 10.
  $lignes = 0
  if (Test-Path $log) { $lignes = (Get-Content $log | Measure-Object -Line).Lines }
  Write-Output ("run {0:d2}/{1} : {2:n0} s observees, {3} lignes de spans" -f $i, $N, ((Get-Date)-$t0).TotalSeconds, $lignes)
  Start-Sleep -Milliseconds 500
}
Stop-Zombies
Write-Output "---"
Write-Output "logs : $logs"
