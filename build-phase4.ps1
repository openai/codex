# Phase 4 Build Script

Write-Host "Starting Phase 4 build..." -ForegroundColor Cyan

if (-not (Test-Path "codex-rs")) {
    Write-Host "Error: codex-rs directory not found" -ForegroundColor Red
    exit 1
}

Set-Location -Path "codex-rs"
Write-Host "Building codex-cli..." -ForegroundColor Yellow
cargo build --release -p codex-cli

if ($LASTEXITCODE -eq 0) {
    Set-Location -Path ".."
    if (Test-Path "codex-rs\target\release\codex.exe") {
        Write-Host "Build SUCCESS!" -ForegroundColor Green
        Write-Host "Next step: .\install-phase4.ps1" -ForegroundColor Cyan
    }
} else {
    Set-Location -Path ".."
    Write-Host "Build FAILED!" -ForegroundColor Red
    exit 1
}
