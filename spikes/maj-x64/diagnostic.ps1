#Requires -Version 5.1
<#
  diagnostic.ps1 -- pourquoi "Installer" du bandeau de MAJ ne fait rien.

  Constat terrain 2026-08-26 (poste x64), precise par le Chef Ingenieur :
  au clic sur Installer, LA FENETRE WIND SE FERME, ET AUCUN INSTALLATEUR
  NE SE LANCE.

  Ce symptome est SANS AMBIGUITE. Il dit que tout a REUSSI jusqu'au bout :
  le telechargement, la verification de signature, l'ecriture du temoin
  en %TEMP%. Puis, dans tauri-plugin-updater 2.10.1, updater.rs:854-865 :

      unsafe { ShellExecuteW(..., w!("open"), file, parameters, ...) };
      std::process::exit(0);

  Le retour de ShellExecuteW n'est JAMAIS TESTE, et le processus sort
  quand meme. Si Windows refuse de lancer le temoin, l'application se
  ferme sans un mot et rien ne s'installe. C'est exactement ce qui se voit.

  Reste UNE question, et ce script y repond : pourquoi Windows refuse-t-il
  de lancer ce temoin-la, sur ce poste-la.

  Ce script ne repare rien. Il RAPPORTE des faits, dans l'ordre meme ou
  l'updater les rencontre :
    1. quelle version est installee, ou, et depuis quel chemin elle tourne
    2. le manifeste latest.json est-il joignable, et que dit-il
    3. l'installateur se telecharge-t-il VRAIMENT en entier
    4. le temoin s'ecrit-il, et pourrait-il se lancer
    5. le repertoire d'installation est-il accessible en ecriture
    6. qu'est-ce qui, sur ce poste, peut bloquer un .exe frais

  Ecrit SANS ACCENTS -- convention des .ps1 du depot (faire-release.ps1).
  N'ecrit rien hors de %TEMP%. Ne touche a aucun secret.

  Usage :
     powershell -ExecutionPolicy Bypass -File spikes\maj-x64\diagnostic.ps1
#>
param(
    [string]$Version = "0.10.1",
    # Par defaut on NE LANCE PAS l'installateur : on prouve seulement qu'il
    # POURRAIT se lancer. -AvecLancement va jusqu'au bout (il installera).
    [switch]$AvecLancement
)

$ErrorActionPreference = "Continue"
$ProgressPreference = "SilentlyContinue"

function Section($t) { Write-Host ""; Write-Host "=== $t ===" -ForegroundColor Cyan }
function Ok($t) { Write-Host "  OK    $t" -ForegroundColor Green }
function Ko($t) { Write-Host "  ECHEC $t" -ForegroundColor Red }
function Info($t) { Write-Host "  ..    $t" }
function Warn($t) { Write-Host "  !!    $t" -ForegroundColor Yellow }

$verdicts = @()
function Verdict($cle, $etat, $detail) {
    $script:verdicts += [pscustomobject]@{ Point = $cle; Etat = $etat; Detail = $detail }
}

$guillemet = [char]34

Write-Host ""
Write-Host "Diagnostic auto-update Wind -- cible $Version" -ForegroundColor White
Write-Host "Poste : $env:COMPUTERNAME / $env:PROCESSOR_ARCHITECTURE / $([Environment]::OSVersion.Version)"

# --------------------------------------------------------------------------
Section "1. Quelle version tourne, et d'ou"

$reg = @('HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*',
         'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\*')
$pose = Get-ItemProperty $reg -ErrorAction SilentlyContinue |
        Where-Object { $_.DisplayName -eq 'Wind' } |
        Select-Object -First 1
$dirPose = $null
if ($pose) {
    $dirPose = $pose.InstallLocation
    if ($dirPose) { $dirPose = $dirPose.Trim($guillemet) }
    Ok "installee : $($pose.DisplayVersion)  ->  $dirPose"
    Verdict "version installee" "OK" $pose.DisplayVersion
} else {
    Ko "aucune installation de Wind en base de registre"
    Verdict "version installee" "ABSENTE" "ni HKLM ni HKCU"
}

$procs = Get-Process -Name Wind -ErrorAction SilentlyContinue
if ($procs) {
    $attendu = if ($dirPose) { Join-Path $dirPose "Wind.exe" } else { $null }
    foreach ($p in $procs) {
        $chemin = try { $p.Path } catch { $null }
        $v = if ($chemin) { (Get-Item $chemin).VersionInfo.FileVersion } else { "?" }
        Ok "en cours  : pid $($p.Id)  v$v  <- $chemin"
        Verdict "processus en cours" "OK" "$v depuis $chemin"
    }
    $hors = $procs | Where-Object { $attendu -and $_.Path -and ($_.Path -ne $attendu) }
    if ($hors) {
        Warn "Wind tourne HORS de son repertoire d'installation."
        Warn "Attendu : $attendu"
        Warn "Or      : $($hors[0].Path)"
        Warn "Une mise a jour poserait la version neuve dans le repertoire"
        Warn "d'installation, pas la ou tourne CE binaire : le clic"
        Warn "semblerait alors sans effet."
        Verdict "tourne depuis l'installation" "NON" $hors[0].Path
    } elseif ($attendu) {
        Ok "le binaire en cours EST celui de l'installation"
        Verdict "tourne depuis l'installation" "OUI" $attendu
    }
} else {
    Info "Wind n'est pas lance en ce moment (lance-le pour ce point)."
    Verdict "processus en cours" "ABSENT" "Wind n'est pas lance"
}

# --------------------------------------------------------------------------
Section "2. Le manifeste"

$urlManifeste = "https://github.com/smonchamps/wind/releases/latest/download/latest.json"
$urlExe = $null
try {
    $t0 = Get-Date
    $m = Invoke-RestMethod -Uri $urlManifeste -TimeoutSec 30
    $ms = [int]((Get-Date) - $t0).TotalMilliseconds
    Ok "latest.json lu en $ms ms -- version annoncee : $($m.version)"
    Verdict "manifeste joignable" "OK" "$($m.version) en $ms ms"
    foreach ($k in $m.platforms.PSObject.Properties.Name) {
        Info "$k -> $($m.platforms.$k.url)"
    }
    $cle = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') { 'windows-aarch64' } else { 'windows-x86_64' }
    if ($m.platforms.PSObject.Properties.Name -contains $cle) {
        Ok "cle de plateforme '$cle' presente"
        $urlExe = $m.platforms.$cle.url
        Verdict "cle de plateforme" "OK" $cle
    } else {
        Ko "cle '$cle' ABSENTE du manifeste"
        Verdict "cle de plateforme" "ABSENTE" $cle
    }
} catch {
    Ko "latest.json injoignable : $($_.Exception.Message)"
    Verdict "manifeste joignable" "ECHEC" $_.Exception.Message
}

# --------------------------------------------------------------------------
Section "3. Le telechargement, en entier"
# C'est LE point aveugle : le plugin updater n'a AUCUN timeout
# (updater.rs:176 -- timeout: None) et Wind n'en pose pas. Un transfert
# qui cale ne rend JAMAIS la main : le bandeau resterait fige sur
# "Installation..." pour toujours, ce qui se lit "rien ne se passe".

$tmp = Join-Path $env:TEMP ("wind-diag-" + $Version)
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
$nom = if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') {
    "Wind_${Version}_arm64-setup.exe"
} else {
    "Wind_${Version}_x64-setup.exe"
}
$exe = Join-Path $tmp $nom
if (-not $urlExe) {
    $urlExe = "https://github.com/smonchamps/wind/releases/download/$Version/$nom"
    Warn "URL deduite du tag (le manifeste n'a pas repondu) : $urlExe"
}

try {
    $t0 = Get-Date
    Invoke-WebRequest -Uri $urlExe -OutFile $exe -TimeoutSec 300
    $taille = (Get-Item $exe).Length
    $sec = [math]::Round(((Get-Date) - $t0).TotalSeconds, 1)
    $debit = if ($sec -gt 0) { [math]::Round($taille / 1MB / $sec, 2) } else { 0 }
    Ok "telecharge : $taille octets en $sec s ($debit Mo/s)"
    Verdict "telechargement" "OK" "$taille octets en $sec s"
    if ($taille -lt 1MB) {
        Ko "taille suspecte -- ce n'est probablement pas l'installateur"
        Verdict "telechargement" "SUSPECT" "$taille octets"
    }
} catch {
    Ko "telechargement impossible : $($_.Exception.Message)"
    Warn "C'est le point le plus probable : sans timeout, l'application"
    Warn "attendrait ici SANS FIN, bandeau fige sur Installation..."
    Verdict "telechargement" "ECHEC" $_.Exception.Message
}

# --------------------------------------------------------------------------
Section "4. Le temoin s'ecrit-il, et pourrait-il se lancer"

if (Test-Path $exe) {
    $marque = Get-Content -Path $exe -Stream Zone.Identifier -ErrorAction SilentlyContinue
    if ($marque) {
        Warn "le fichier porte une MARQUE DU WEB (Zone.Identifier) :"
        $marque | ForEach-Object { Warn "    $_" }
        Verdict "marque du web" "PRESENTE" "SmartScreen peut interposer une invite"
    } else {
        Ok "aucune marque du web sur le temoin"
        Verdict "marque du web" "ABSENTE" ""
    }

    $sig = Get-AuthenticodeSignature $exe
    Info "signature Authenticode : $($sig.Status)"
    Verdict "authenticode" "$($sig.Status)" ""

    if ($AvecLancement) {
        # LES MEMES ARGUMENTS que l'updater (updater.rs:801-816 --
        # install_mode.nsis_args() = /P /R en mode passive par defaut,
        # puis /UPDATE /ARGS). On reproduit, on n'improvise pas.
        $argsMaj = @('/P', '/R', '/UPDATE', '/ARGS')
        Warn "lancement REEL de l'installateur : il va installer $Version."
        Warn "arguments : $($argsMaj -join ' ')"
        try {
            $p = Start-Process -FilePath $exe -ArgumentList $argsMaj -PassThru -ErrorAction Stop
            Ok "installateur lance (pid $($p.Id))"
            Start-Sleep -Seconds 3
            if ($p.HasExited) {
                Warn "il a deja QUITTE, code $($p.ExitCode) -- il demarre mais ne fait rien"
                Verdict "lancement" "SORTIE IMMEDIATE" "code $($p.ExitCode)"
            } else {
                Ok "toujours en cours apres 3 s -- il installe"
                Verdict "lancement" "OK" "pid $($p.Id)"
            }
        } catch {
            Ko "Windows a REFUSE de lancer le temoin : $($_.Exception.Message)"
            Warn "C'est exactement ce que l'application vit : le plugin appelle"
            Warn "ShellExecuteW SANS TESTER son retour, puis sort par exit(0)."
            Verdict "lancement" "REFUSE" $_.Exception.Message
        }
    } else {
        Info "lancement non joue -- ajoute -AvecLancement pour aller au bout."
        Info "Si rien ne s'ouvre alors, double-clique le temoin a la main :"
        Info "  $exe"
        Info "(sans argument, il montre son interface complete et ses erreurs)"
        Verdict "lancement" "NON JOUE" "option -AvecLancement"
    }
} else {
    Ko "pas de temoin a examiner (le telechargement a echoue)"
    Verdict "temoin" "ABSENT" "telechargement echoue"
}

# --------------------------------------------------------------------------
Section "5. Le repertoire d'installation"

if ($dirPose) {
    if (Test-Path $dirPose) {
        $essai = Join-Path $dirPose ".wind-diag-ecriture"
        try {
            Set-Content -Path $essai -Value "x" -ErrorAction Stop
            Remove-Item $essai -Force
            Ok "$dirPose accessible en ecriture"
            Verdict "ecriture dans l'installation" "OK" $dirPose
        } catch {
            Ko "$dirPose NON accessible en ecriture : $($_.Exception.Message)"
            Verdict "ecriture dans l'installation" "REFUSEE" $_.Exception.Message
        }
    } else {
        Ko "$dirPose n'existe pas"
        Verdict "ecriture dans l'installation" "ABSENT" $dirPose
    }
}

# --------------------------------------------------------------------------
Section "6. Ce qui peut bloquer un .exe frais"

try {
    $d = Get-MpComputerStatus -ErrorAction Stop
    Info "Defender temps reel : $($d.RealTimeProtectionEnabled) -- moteur $($d.AMEngineVersion)"
    Verdict "defender temps reel" "$($d.RealTimeProtectionEnabled)" ""
} catch { Info "Get-MpComputerStatus indisponible sur ce poste" }

try {
    $menaces = Get-MpThreatDetection -ErrorAction Stop |
               Where-Object { $_.InitialDetectionTime -gt (Get-Date).AddDays(-2) }
    if ($menaces) {
        Warn "Defender a signale quelque chose dans les 2 derniers jours :"
        $menaces | ForEach-Object { Warn "    $($_.InitialDetectionTime)  $($_.Resources -join ', ')" }
        Verdict "detection defender recente" "OUI" "$($menaces.Count) evenement(s)"
    } else {
        Ok "aucune detection Defender dans les 2 derniers jours"
        Verdict "detection defender recente" "NON" ""
    }
} catch { Info "Get-MpThreatDetection indisponible sur ce poste" }

$sm = Get-ItemProperty 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Explorer' `
      -Name SmartScreenEnabled -ErrorAction SilentlyContinue
if ($sm) { Info "SmartScreen (Explorer) : $($sm.SmartScreenEnabled)" }

$proxy = Get-ItemProperty 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Internet Settings' `
         -ErrorAction SilentlyContinue
if ($proxy -and $proxy.ProxyEnable -eq 1) {
    Warn "un PROXY WinINET est actif : $($proxy.ProxyServer)"
    Verdict "proxy" "ACTIF" $proxy.ProxyServer
} else {
    Ok "aucun proxy WinINET actif"
    Verdict "proxy" "AUCUN" ""
}

# --------------------------------------------------------------------------
Section "Releve"

$verdicts | Format-Table -AutoSize | Out-String -Width 200 | Write-Host
Write-Host "Temoin conserve : $tmp" -ForegroundColor DarkGray
Write-Host "Colle ce releve entier dans la conversation." -ForegroundColor White
