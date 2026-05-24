# Download a small sample image for Phase 2 detect tests.
# Usage: .\scripts\download-sample-image.ps1

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
$samplesDir = Join-Path $repoRoot "data\samples"
$outFile = Join-Path $samplesDir "test.jpg"

New-Item -ItemType Directory -Force -Path $samplesDir | Out-Null

# COCO sample via ultralytics assets (bus.jpg — common YOLO demo image)
$url = "https://ultralytics.com/images/bus.jpg"

Write-Host "Downloading sample image to $outFile ..."
Invoke-WebRequest -Uri $url -OutFile $outFile -UseBasicParsing

Write-Host "Done."
