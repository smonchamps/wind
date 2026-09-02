# Regenerates apps/desktop/icons/icon.ico from the geometry of the Elements
# brand (System, "Brand" section, TILE regime -- V1/V11, W-D3: frozen
# outside the themes). GDI+ draws the same shapes as Marque.svelte: tile
# #F2EDE3 at the platform radius 15/64 (the ONLY rounded shape, V14),
# envelope #141414 with sharp corners (M4 8h16v9H4z, stroke 2.3 on a
# viewBox of 24), flap as a half-disc #1F8A8A (center 12;9.15 r 3.25).
#
# Sizes: 256, 48, 32, 16. At 16 px the stroke goes to 2 units (the
# Marque.svelte regime). The pixel floor of the old "W-badge" rendering is
# dead: in the 24 geometry the computed stroke exceeds it at every size
# (1.33 px at worst, at 16).
#
#   pwsh scripts/make-icon.ps1                   # writes icon.ico
#   pwsh scripts/make-icon.ps1 -Preview folder   # + control PNGs

param([string]$Preview = "")

Add-Type -AssemblyName System.Drawing

$tileBg    = [System.Drawing.Color]::FromArgb(0xFF, 0xF2, 0xED, 0xE3)
$structure = [System.Drawing.Color]::FromArgb(0xFF, 0x14, 0x14, 0x14)
$teal      = [System.Drawing.Color]::FromArgb(0xFF, 0x1F, 0x8A, 0x8A)

function New-RoundedPath([single]$x, [single]$y, [single]$w, [single]$h, [single]$r) {
    $p = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = 2 * $r
    $p.AddArc($x, $y, $d, $d, 180, 90)
    $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
    $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
    $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
    $p.CloseFigure()
    return $p
}

function New-Rendering([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.PixelOffsetMode = 'HighQuality'
    $g.Clear([System.Drawing.Color]::Transparent)

    # The tile fills the canvas; every dimension is in units of the
    # brand's viewBox of 24.
    $u = $size / 24.0

    $brush = New-Object System.Drawing.SolidBrush($tileBg)
    $g.FillPath($brush, (New-RoundedPath 0 0 $size $size ($size * 15.0 / 64.0)))

    # The envelope: rectangle 4;8 -> 20;17, sharp corners, centered stroke --
    # 2.3 units (2 at 16 px and below, the Marque.svelte regime).
    $strokeUnits = if ($size -le 16) { 2.0 } else { 2.3 }
    $stroke = $strokeUnits * $u
    $pen = New-Object System.Drawing.Pen($structure, $stroke)
    $pen.LineJoin = 'Miter'
    $g.DrawRectangle($pen, (4.0 * $u), (8.0 * $u), (16.0 * $u), (9.0 * $u))

    # The flap: teal half-disc under the chord y = 9.15, tangent to the
    # inner top edge (center 12;9.15, radius 3.25).
    $r = 3.25 * $u
    $tealBrush = New-Object System.Drawing.SolidBrush($teal)
    $g.FillPie($tealBrush, (12.0 * $u - $r), (9.15 * $u - $r), (2 * $r), (2 * $r), 0, 180)

    $g.Dispose()
    return $bmp
}

$sizes = @(256, 48, 32, 16)

$pngs = foreach ($s in $sizes) {
    $bmp = New-Rendering $s
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    if ($Preview) {
        New-Item -ItemType Directory -Force $Preview | Out-Null
        [System.IO.File]::WriteAllBytes((Join-Path $Preview "preview-$s.png"), $ms.ToArray())
    }
    $bmp.Dispose()
    @{ size = $s; bytes = $ms.ToArray() }
}

$out = Join-Path $PSScriptRoot "..\apps\desktop\icons\icon.ico"
$stream = [System.IO.File]::Create($out)
$writer = New-Object System.IO.BinaryWriter($stream)
$writer.Write([uint16]0)               # reserved
$writer.Write([uint16]1)               # icon type
$writer.Write([uint16]$pngs.Count)
$offset = 6 + 16 * $pngs.Count
foreach ($p in $pngs) {
    $dim = if ($p.size -ge 256) { 0 } else { $p.size }
    $writer.Write([byte]$dim); $writer.Write([byte]$dim)
    $writer.Write([byte]0); $writer.Write([byte]0)   # palette, reserved
    $writer.Write([uint16]1); $writer.Write([uint16]32)
    $writer.Write([uint32]$p.bytes.Length)
    $writer.Write([uint32]$offset)
    $offset += $p.bytes.Length
}
foreach ($p in $pngs) { $writer.Write($p.bytes) }
$writer.Dispose()

Write-Host "icon.ico written ($($pngs.Count) sizes: $($sizes -join ', '))."
