# Generate synthetic small-target frame sequence (bright dot on blue sky).
# Used for Phase 3 pipeline testing and future intercept demo (ADR-017).
# Usage: .\scripts\generate-dot-video.ps1 [-OutputDir path] [-FrameCount 100]

param(
    [string]$OutputDir = "",
    [int]$FrameCount = 100,
    [int]$Width = 640,
    [int]$Height = 480,
    [int]$DotDiameter = 12
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

if (-not $OutputDir) {
    $OutputDir = Join-Path $repoRoot "data\frames\dot_run_001"
}

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

Add-Type -AssemblyName System.Drawing

# Sky blue background; white dot moves diagonally (small target ~12 px).
$bg = [System.Drawing.Color]::FromArgb(26, 58, 92)

Write-Host "Generating $FrameCount frames ($Width x $Height) to $OutputDir ..."

for ($i = 0; $i -lt $FrameCount; $i++) {
    $bmp = New-Object System.Drawing.Bitmap $Width, $Height
    $graphics = [System.Drawing.Graphics]::FromImage($bmp)
    $graphics.Clear($bg)

  # Dot moves ~5 px/frame — simulates fast small mover across FOV.
    $x = 40 + ($i * 5)
    $y = 120 + [int](($i * 3) % ($Height - 240))
    $brush = [System.Drawing.Brushes]::White
    $graphics.FillEllipse($brush, $x, $y, $DotDiameter, $DotDiameter)
    $graphics.Dispose()

    $outFile = Join-Path $OutputDir ("{0:D4}.png" -f ($i + 1))
    $bmp.Save($outFile, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()

    if (($i + 1) % 25 -eq 0) {
        Write-Host "  $($i + 1) / $FrameCount"
    }
}

Write-Host "Done. $FrameCount frames in $OutputDir"
Write-Host "Run: cd crates/seeker-sim; cargo run -- process --input ../../data/frames/dot_run_001"
