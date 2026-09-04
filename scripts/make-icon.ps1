# Regenerates apps/desktop/icons/icon.ico AND icon.icns from the geometry
# of the Elements brand (System, "Brand" section, TILE regime -- V1/V11,
# W-D3: frozen outside the themes). GDI+ draws the same shapes as
# Marque.svelte: tile #F2EDE3 at the platform radius 15/64 (the ONLY
# rounded shape, V14), envelope #141414 with sharp corners (M4 8h16v9H4z,
# stroke 2.3 on a viewBox of 24), flap as a half-disc #1F8A8A (center
# 12;9.15 r 3.25).
#
# ICO sizes: 256, 48, 32, 16. At 16 px the stroke goes to 2 units (the
# Marque.svelte regime). The pixel floor of the old "W-badge" rendering is
# dead: in the 24 geometry the computed stroke exceeds it at every size
# (1.33 px at worst, at 16).
#
# ICNS (PLAN-MACOS E2): the SAME drawing, packed as PNG-payload icns
# entries (icp4..ic10 -- the format macOS reads since 10.7; no iconutil,
# so the file is produced and committed from the Windows workstation).
# One macOS convention applies: the tile is inset to 824/1024 of the
# canvas (Apple's icon grid -- a full-bleed tile reads oversized in the
# Dock), margins transparent.
#
#   pwsh scripts/make-icon.ps1                   # writes icon.ico + icon.icns
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

function New-Rendering([int]$size, [double]$scale = 1.0) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.PixelOffsetMode = 'HighQuality'
    $g.Clear([System.Drawing.Color]::Transparent)

    # The tile fills `scale` of the canvas (1.0 on Windows, 824/1024 on
    # macOS -- Apple's icon grid), centered; every dimension is in
    # units of the brand's viewBox of 24.
    $side = $size * $scale
    $pad = ($size - $side) / 2.0
    $u = $side / 24.0

    $brush = New-Object System.Drawing.SolidBrush($tileBg)
    $g.FillPath($brush, (New-RoundedPath $pad $pad $side $side ($side * 15.0 / 64.0)))

    # The envelope: rectangle 4;8 -> 20;17, sharp corners, centered stroke --
    # 2.3 units (2 at 16 px and below, the Marque.svelte regime).
    $strokeUnits = if ($size -le 16) { 2.0 } else { 2.3 }
    $stroke = $strokeUnits * $u
    $pen = New-Object System.Drawing.Pen($structure, $stroke)
    $pen.LineJoin = 'Miter'
    $g.DrawRectangle($pen, ($pad + 4.0 * $u), ($pad + 8.0 * $u), (16.0 * $u), (9.0 * $u))

    # The flap: teal half-disc under the chord y = 9.15, tangent to the
    # inner top edge (center 12;9.15, radius 3.25).
    $r = 3.25 * $u
    $tealBrush = New-Object System.Drawing.SolidBrush($teal)
    $g.FillPie($tealBrush, ($pad + 12.0 * $u - $r), ($pad + 9.15 * $u - $r), (2 * $r), (2 * $r), 0, 180)

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

# --- icon.icns (PLAN-MACOS E2) ---------------------------------------
# PNG-payload entries; lengths are BIG-endian (the icns format), which
# BinaryWriter does not speak -- written byte by byte. Entry types per
# Apple's table: icp4/icp5 (16/32), ic07..ic10 (128/256/512/1024),
# ic11..ic14 (the @2x renditions macOS actually favors in the Dock).

function Write-BigEndian([System.IO.BinaryWriter]$w, [uint32]$value) {
    $w.Write([byte](($value -shr 24) -band 0xFF))
    $w.Write([byte](($value -shr 16) -band 0xFF))
    $w.Write([byte](($value -shr 8) -band 0xFF))
    $w.Write([byte]($value -band 0xFF))
}

$macScale = 824.0 / 1024.0
$icnsEntries = @(
    @{ type = 'icp4'; px = 16 },
    @{ type = 'icp5'; px = 32 },
    @{ type = 'ic11'; px = 32 },
    @{ type = 'ic12'; px = 64 },
    @{ type = 'ic07'; px = 128 },
    @{ type = 'ic08'; px = 256 },
    @{ type = 'ic13'; px = 256 },
    @{ type = 'ic09'; px = 512 },
    @{ type = 'ic14'; px = 512 },
    @{ type = 'ic10'; px = 1024 }
)
$rendered = @{}
foreach ($e in $icnsEntries) {
    if (-not $rendered.ContainsKey($e.px)) {
        $bmp = New-Rendering $e.px $macScale
        $ms = New-Object System.IO.MemoryStream
        $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
        if ($Preview) {
            [System.IO.File]::WriteAllBytes((Join-Path $Preview "preview-mac-$($e.px).png"), $ms.ToArray())
        }
        $bmp.Dispose()
        $rendered[$e.px] = $ms.ToArray()
    }
}
$total = 8
foreach ($e in $icnsEntries) { $total += 8 + $rendered[$e.px].Length }
$outIcns = Join-Path $PSScriptRoot "..\apps\desktop\icons\icon.icns"
$icnsStream = [System.IO.File]::Create($outIcns)
$icnsWriter = New-Object System.IO.BinaryWriter($icnsStream)
$icnsWriter.Write([System.Text.Encoding]::ASCII.GetBytes('icns'))
Write-BigEndian $icnsWriter ([uint32]$total)
foreach ($e in $icnsEntries) {
    $bytes = $rendered[$e.px]
    $icnsWriter.Write([System.Text.Encoding]::ASCII.GetBytes($e.type))
    Write-BigEndian $icnsWriter ([uint32](8 + $bytes.Length))
    $icnsWriter.Write($bytes)
}
$icnsWriter.Dispose()

Write-Host "icon.icns written ($($icnsEntries.Count) entries, tile at 824/1024)."
