# Renders text pages to PNG with Windows' own text shaping (GDI+ / Uniscribe), because Pillow in this environment has no complex-text
# layout (no libraqm) and would draw Devanagari matras and conjuncts wrongly. Windows only.
# Usage: powershell -File render_pages.ps1 -Spec pages.json -OutDir dir
# Spec: [{ "id": "...", "font": "Nirmala UI", "bold": false, "size_px": 38, "lines": ["...", ...] }, ...]
param([Parameter(Mandatory = $true)][string]$Spec, [Parameter(Mandatory = $true)][string]$OutDir)
Add-Type -AssemblyName System.Drawing
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $OutDir | Out-Null
$pages = Get-Content -Raw -Encoding UTF8 $Spec | ConvertFrom-Json
foreach ($p in $pages) {
    $w = 1654; $h = 2339
    $bmp = New-Object System.Drawing.Bitmap $w, $h, ([System.Drawing.Imaging.PixelFormat]::Format24bppRgb)
    $bmp.SetResolution(200, 200)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.Clear([System.Drawing.Color]::White)
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAlias
    $style = if ($p.bold) { [System.Drawing.FontStyle]::Bold } else { [System.Drawing.FontStyle]::Regular }
    $font = New-Object System.Drawing.Font($p.font, [single]$p.size_px, $style, ([System.Drawing.GraphicsUnit]::Pixel))
    $fmt = [System.Drawing.StringFormat]::GenericTypographic
    $fmt.FormatFlags = $fmt.FormatFlags -bor [System.Drawing.StringFormatFlags]::NoWrap -bor [System.Drawing.StringFormatFlags]::MeasureTrailingSpaces
    $brush = [System.Drawing.Brushes]::Black
    $y = 150.0
    foreach ($line in $p.lines) {
        if ($line.Length -gt 0) { $g.DrawString([string]$line, $font, $brush, [single]130, [single]$y, $fmt) }
        $y += [double]$p.size_px * 1.9
    }
    $g.Dispose()
    $bmp.Save((Join-Path $OutDir ($p.id + '.png')), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}
Write-Output "rendered $($pages.Count) pages"
