# Extract PNG frames from a video with ffmpeg.
# Usage: .\scripts\extract-frames.ps1 [-InputVideo path] [-OutputDir path]

param(
    [string]$InputVideo = "",
    [string]$OutputDir = ""
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot

if (-not $InputVideo) {
    $InputVideo = Join-Path $repoRoot "data\videos\sample.mp4"
}
if (-not $OutputDir) {
    $OutputDir = Join-Path $repoRoot "data\frames\run_001"
}

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$ffmpeg = Get-Command ffmpeg -ErrorAction SilentlyContinue
if (-not $ffmpeg) {
    Write-Error "ffmpeg not found on PATH. Install ffmpeg and retry."
}

if (-not (Test-Path $InputVideo)) {
    Write-Error "Input video not found: $InputVideo"
}

Write-Host "Extracting frames from $InputVideo to $OutputDir ..."
& ffmpeg -y -i $InputVideo -vsync 0 (Join-Path $OutputDir "%04d.png")

$count = (Get-ChildItem $OutputDir -Filter "*.png").Count
Write-Host "Done. $count PNG frames in $OutputDir"
