# Régénère apps/desktop/icons/icon.ico depuis la géométrie de la marque
# Elements (Système, section « Marque », régime TUILE — V1/V11, W-D3 :
# figée hors thèmes). GDI+ trace les mêmes formes que Marque.svelte :
# tuile #F2EDE3 au rayon de plateforme 15/64 (la SEULE forme arrondie,
# V14), enveloppe #141414 à coins vifs (M4 8h16v9H4z, trait 2,3 sur
# viewBox 24), rabat en demi-disque #1F8A8A (centre 12;9,15 r 3,25).
#
# Tailles : 256, 48, 32, 16. À 16 px le trait passe à 2 unités (le
# régime de Marque.svelte). Le plancher en pixels de l'ancien rendu
# « W-pastille » est mort : dans la géométrie 24, le trait calculé le
# dépasse à toutes les tailles (1,33 px au pire, à 16).
#
#   pwsh scripts/faire-icone.ps1                  # écrit icon.ico
#   pwsh scripts/faire-icone.ps1 -Apercu dossier  # + PNG de contrôle

param([string]$Apercu = "")

Add-Type -AssemblyName System.Drawing

$fondTuile = [System.Drawing.Color]::FromArgb(0xFF, 0xF2, 0xED, 0xE3)
$structure = [System.Drawing.Color]::FromArgb(0xFF, 0x14, 0x14, 0x14)
$teal      = [System.Drawing.Color]::FromArgb(0xFF, 0x1F, 0x8A, 0x8A)

function New-CheminArrondi([single]$x, [single]$y, [single]$w, [single]$h, [single]$r) {
    $p = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = 2 * $r
    $p.AddArc($x, $y, $d, $d, 180, 90)
    $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
    $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
    $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
    $p.CloseFigure()
    return $p
}

function New-Rendu([int]$taille) {
    $bmp = New-Object System.Drawing.Bitmap($taille, $taille, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.PixelOffsetMode = 'HighQuality'
    $g.Clear([System.Drawing.Color]::Transparent)

    # La tuile emplit le canevas ; toutes les cotes sont en unités du
    # viewBox 24 de la marque.
    $u = $taille / 24.0

    $pinceau = New-Object System.Drawing.SolidBrush($fondTuile)
    $g.FillPath($pinceau, (New-CheminArrondi 0 0 $taille $taille ($taille * 15.0 / 64.0)))

    # L'enveloppe : rectangle 4;8 → 20;17, coins vifs, trait centré —
    # 2,3 unités (2 à 16 px et moins, le régime de Marque.svelte).
    $unitesTrait = if ($taille -le 16) { 2.0 } else { 2.3 }
    $trait = $unitesTrait * $u
    $plume = New-Object System.Drawing.Pen($structure, $trait)
    $plume.LineJoin = 'Miter'
    $g.DrawRectangle($plume, (4.0 * $u), (8.0 * $u), (16.0 * $u), (9.0 * $u))

    # Le rabat : demi-disque teal sous la corde y = 9,15, tangent au
    # bord intérieur haut (centre 12 ; 9,15, rayon 3,25).
    $r = 3.25 * $u
    $pinceauT = New-Object System.Drawing.SolidBrush($teal)
    $g.FillPie($pinceauT, (12.0 * $u - $r), (9.15 * $u - $r), (2 * $r), (2 * $r), 0, 180)

    $g.Dispose()
    return $bmp
}

$tailles = @(256, 48, 32, 16)

$pngs = foreach ($t in $tailles) {
    $bmp = New-Rendu $t
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    if ($Apercu) {
        New-Item -ItemType Directory -Force $Apercu | Out-Null
        [System.IO.File]::WriteAllBytes((Join-Path $Apercu "apercu-$t.png"), $ms.ToArray())
    }
    $bmp.Dispose()
    @{ taille = $t; octets = $ms.ToArray() }
}

$sortie = Join-Path $PSScriptRoot "..\apps\desktop\icons\icon.ico"
$flux = [System.IO.File]::Create($sortie)
$ecrivain = New-Object System.IO.BinaryWriter($flux)
$ecrivain.Write([uint16]0)               # réservé
$ecrivain.Write([uint16]1)               # type icône
$ecrivain.Write([uint16]$pngs.Count)
$decalage = 6 + 16 * $pngs.Count
foreach ($p in $pngs) {
    $dim = if ($p.taille -ge 256) { 0 } else { $p.taille }
    $ecrivain.Write([byte]$dim); $ecrivain.Write([byte]$dim)
    $ecrivain.Write([byte]0); $ecrivain.Write([byte]0)   # palette, réservé
    $ecrivain.Write([uint16]1); $ecrivain.Write([uint16]32)
    $ecrivain.Write([uint32]$p.octets.Length)
    $ecrivain.Write([uint32]$decalage)
    $decalage += $p.octets.Length
}
foreach ($p in $pngs) { $ecrivain.Write($p.octets) }
$ecrivain.Dispose()

Write-Host "icon.ico écrit ($($pngs.Count) tailles : $($tailles -join ', '))."
