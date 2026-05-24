# Download YOLOv8n ONNX weights into models/ (gitignored).
# Usage: .\scripts\download-model.ps1

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
$modelsDir = Join-Path $repoRoot "models"
$outFile = Join-Path $modelsDir "yolov8n.onnx"

New-Item -ItemType Directory -Force -Path $modelsDir | Out-Null

$url = "https://github.com/ultralytics/assets/releases/download/v8.4.0/yolov8n.onnx"

Write-Host "Downloading YOLOv8n ONNX to $outFile ..."
Invoke-WebRequest -Uri $url -OutFile $outFile -UseBasicParsing

Write-Host "Done. Size: $((Get-Item $outFile).Length) bytes"
Write-Host "License: Ultralytics YOLO is AGPL-3.0 - see docs/PROJECT_BRIEF.md"
