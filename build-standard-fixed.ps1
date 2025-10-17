#!/usr/bin/env pwsh
# Fixed Standard Build Script

Write-Host "`n=== Standard Build (No Custom Optimizations) ===" -ForegroundColor Cyan

# Navigate to codex-rs
$originalDir = Get-Location
Set-Location "codex-rs"

Write-Host "Current directory: $(Get-Location)" -ForegroundColor Yellow
Write-Host "Starting build..." -ForegroundColor Cyan
Write-Host ""

# Build
cargo build --release -p codex-cli 2>&1 | Tee-Object -FilePath "..\build-standard.log"

# Return to original directory
Set-Location $originalDir

Write-Host "`nBuild complete!" -ForegroundColor Green

