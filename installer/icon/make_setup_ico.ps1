param(
    [string]$SourcePng = "$PSScriptRoot\storage_analyser.png",
    [string]$OutIco = "$PSScriptRoot\storage_analyser_setup.ico"
)

Add-Type -AssemblyName System.Drawing

# Inno Setup's resource updater is picky about PNG-compressed ICO frames (which is what
# hand-rolled multi-size icons often use for the 256px entry). GetHicon() instead produces
# a classic uncompressed DIB-based icon, which is universally accepted — used here only for
# the Setup.exe wizard icon; the app's own embedded icon (build.rs) keeps the nicer
# multi-resolution PNG-based .ico since windres has no trouble with it.
$src = New-Object System.Drawing.Bitmap($SourcePng)
$resized = New-Object System.Drawing.Bitmap(48, 48)
$g = [System.Drawing.Graphics]::FromImage($resized)
$g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
$g.DrawImage($src, 0, 0, 48, 48)
$g.Dispose()
$src.Dispose()

$hIcon = $resized.GetHicon()
$icon = [System.Drawing.Icon]::FromHandle($hIcon)
$fs = New-Object System.IO.FileStream($OutIco, [System.IO.FileMode]::Create)
$icon.Save($fs)
$fs.Dispose()
$icon.Dispose()
$resized.Dispose()

Write-Output "Wrote $OutIco"
