param(
    [string]$SourcePng = "$PSScriptRoot\storage_analyser.png",
    [string]$OutIco = "$PSScriptRoot\storage_analyser.ico"
)

Add-Type -AssemblyName System.Drawing

$sizes = @(16, 32, 48, 256)
$src = [System.Drawing.Image]::FromFile($SourcePng)

$entries = @()
foreach ($size in $sizes) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.DrawImage($src, 0, 0, $size, $size)
    $g.Dispose()

    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bytes = $ms.ToArray()
    $ms.Dispose()
    $bmp.Dispose()

    $entries += [PSCustomObject]@{ Size = $size; Bytes = $bytes }
}
$src.Dispose()

$fs = New-Object System.IO.FileStream($OutIco, [System.IO.FileMode]::Create)
$bw = New-Object System.IO.BinaryWriter($fs)

# ICONDIR
$bw.Write([UInt16]0)          # reserved
$bw.Write([UInt16]1)          # type: icon
$bw.Write([UInt16]$entries.Count)

$dataOffset = 6 + (16 * $entries.Count)
foreach ($e in $entries) {
    $dim = if ($e.Size -ge 256) { 0 } else { $e.Size }
    $bw.Write([byte]$dim)      # width
    $bw.Write([byte]$dim)      # height
    $bw.Write([byte]0)         # color count
    $bw.Write([byte]0)         # reserved
    $bw.Write([UInt16]1)       # color planes
    $bw.Write([UInt16]32)      # bits per pixel
    $bw.Write([UInt32]$e.Bytes.Length)
    $bw.Write([UInt32]$dataOffset)
    $dataOffset += $e.Bytes.Length
}
foreach ($e in $entries) {
    $bw.Write($e.Bytes)
}

$bw.Flush()
$bw.Dispose()
$fs.Dispose()

Write-Output "Wrote $OutIco"
