# Pre-push security scan (run before git push)
# Usage: .\scripts\pre-push-check.ps1

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

Write-Host "=== SeekerSim pre-push check ===" -ForegroundColor Cyan

# 1. Staged files
$staged = git diff --cached --name-only
if (-not $staged) {
    Write-Host "No staged files. Stage commits first, or this checks the index before push." -ForegroundColor Yellow
} else {
    Write-Host "`nStaged files:" -ForegroundColor White
    $staged | ForEach-Object { Write-Host "  $_" }
}

# 2. Forbidden paths in index
$forbiddenPatterns = @(
    '\.env',
    '\.env\.',
    '\.pem$',
    '\.pfx$',
    '\.key$',
    'id_rsa',
    'credentials',
    'secrets',
    '\\target\\',
    '\.onnx$',
    '\.pdb$'
)

$blocked = @()
foreach ($file in $staged) {
    foreach ($pat in $forbiddenPatterns) {
        if ($file -match $pat) {
            $blocked += $file
        }
    }
}

if ($blocked.Count -gt 0) {
    Write-Host "`nBLOCKED: staged files match forbidden patterns:" -ForegroundColor Red
    $blocked | Select-Object -Unique | ForEach-Object { Write-Host "  $_" -ForegroundColor Red }
    exit 1
}

# 3. Secret-like content in staged diff
$diff = git diff --cached 2>$null
$secretPatterns = @(
    'ghp_[A-Za-z0-9]{20,}',
    'github_pat_[A-Za-z0-9_]{20,}',
    'sk-[A-Za-z0-9]{20,}',
    'AKIA[0-9A-Z]{16}',
    '(?i)(api[_-]?key|secret|password|token)\s*=\s*["''][^"'']+["'']',
    'Bearer\s+[A-Za-z0-9\-_.]{20,}'
)

$foundSecrets = $false
foreach ($pat in $secretPatterns) {
    if ($diff -match $pat) {
        Write-Host "`nBLOCKED: staged diff may contain a secret (pattern: $pat)" -ForegroundColor Red
        $foundSecrets = $true
    }
}

if ($foundSecrets) {
    Write-Host "Remove secrets before pushing. See rules.md" -ForegroundColor Red
    exit 1
}

Write-Host "`nOK: no obvious blocked paths or secret patterns in staged diff." -ForegroundColor Green
Write-Host "Review rules.md and git diff manually before push." -ForegroundColor Gray
exit 0
