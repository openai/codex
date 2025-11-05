# Phase 4 Installation Script
# Install codex.exe to global bin directory

$ErrorActionPreference = "Stop"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Phase 4: Installation" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Check if binary exists
$binaryPath = "codex-rs\target\release\codex.exe"
if (-not (Test-Path $binaryPath)) {
    Write-Host "ERROR: Binary not found!" -ForegroundColor Red
    Write-Host "Path: $binaryPath" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Please run build first:" -ForegroundColor Yellow
    Write-Host "  .\fast-build.ps1 -Release" -ForegroundColor White
    exit 1
}

$fileInfo = Get-Item $binaryPath
Write-Host "Found binary:" -ForegroundColor Green
Write-Host "  Size: $([math]::Round($fileInfo.Length / 1MB, 2)) MB" -ForegroundColor White
Write-Host "  Modified: $($fileInfo.LastWriteTime)" -ForegroundColor White
Write-Host ""

# Create install directory
$installDir = "$env:USERPROFILE\.codex\bin"
Write-Host "Install directory: $installDir" -ForegroundColor Cyan

if (-not (Test-Path $installDir)) {
    Write-Host "  Creating directory..." -ForegroundColor Yellow
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    Write-Host "  Created!" -ForegroundColor Green
} else {
    Write-Host "  Directory exists" -ForegroundColor Green
}
Write-Host ""

# Copy binary
Write-Host "Copying binary..." -ForegroundColor Yellow
try {
    Copy-Item -Path $binaryPath -Destination "$installDir\codex.exe" -Force
    Write-Host "  Copied successfully!" -ForegroundColor Green
} catch {
    Write-Host "  ERROR: Failed to copy binary!" -ForegroundColor Red
    Write-Host "  $_" -ForegroundColor Red
    exit 1
}
Write-Host ""

# Verify installation
$installedBinary = "$installDir\codex.exe"
if (Test-Path $installedBinary) {
    $installedInfo = Get-Item $installedBinary
    Write-Host "Verification:" -ForegroundColor Cyan
    Write-Host "  Path: $installedBinary" -ForegroundColor White
    Write-Host "  Size: $([math]::Round($installedInfo.Length / 1MB, 2)) MB" -ForegroundColor White
    Write-Host ""
} else {
    Write-Host "ERROR: Installation verification failed!" -ForegroundColor Red
    exit 1
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Installation Complete!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Test commands:" -ForegroundColor Yellow
Write-Host "  codex --version" -ForegroundColor White
Write-Host "  codex delegate-parallel --help" -ForegroundColor White
Write-Host "  codex agent-create --help" -ForegroundColor White
Write-Host ""
