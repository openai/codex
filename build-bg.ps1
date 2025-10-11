# Background Build Script
$ErrorActionPreference = "Continue"

$codexRsDir = Join-Path $PSScriptRoot "codex-rs"
$logFile = Join-Path $PSScriptRoot "build-progress.log"

Write-Host "Starting background build..." -ForegroundColor Cyan
Write-Host "Log: $logFile" -ForegroundColor Gray

Set-Location $codexRsDir
cargo build --release -p codex-cli 2>&1 | Tee-Object -FilePath $logFile

