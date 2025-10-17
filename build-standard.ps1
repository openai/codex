# Standard Build Script (No Custom Optimizations)
# Fallback when LLD linker causes issues

$ErrorActionPreference = "Stop"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Standard Build (Safe Mode)" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Remove custom config if exists
if (Test-Path "codex-rs\.cargo") {
    Write-Host "Removing custom build config..." -ForegroundColor Yellow
    Remove-Item -Recurse -Force "codex-rs\.cargo"
    Write-Host "  Done!" -ForegroundColor Green
}
Write-Host ""

# Clean
Write-Host "Cleaning..." -ForegroundColor Yellow
Set-Location "codex-rs"
Remove-Item -Recurse -Force "target\release" -ErrorAction SilentlyContinue
Write-Host "  Done!" -ForegroundColor Green
Write-Host ""

# Build
Write-Host "Building with standard settings..." -ForegroundColor Cyan
Write-Host "  This may take 10-15 minutes" -ForegroundColor Gray
Write-Host ""

$startTime = Get-Date
cargo build --release -p codex-cli

$endTime = Get-Date
$duration = ($endTime - $startTime).TotalSeconds

Set-Location ".."

if ($LASTEXITCODE -eq 0) {
    $binary = "codex-rs\target\release\codex.exe"
    if (Test-Path $binary) {
        $fileInfo = Get-Item $binary
        Write-Host ""
        Write-Host "========================================" -ForegroundColor Green
        Write-Host " BUILD SUCCESS!" -ForegroundColor Green
        Write-Host "========================================" -ForegroundColor Green
        Write-Host ""
        Write-Host "Time: $([math]::Floor($duration / 60))m $([math]::Floor($duration % 60))s" -ForegroundColor White
        Write-Host "Size: $([math]::Round($fileInfo.Length / 1MB, 2)) MB" -ForegroundColor White
        Write-Host ""
        Write-Host "Next: .\install-phase4.ps1" -ForegroundColor Cyan
    }
} else {
    Write-Host ""
    Write-Host "========================================" -ForegroundColor Red
    Write-Host " BUILD FAILED" -ForegroundColor Red
    Write-Host "========================================" -ForegroundColor Red
    exit 1
}

