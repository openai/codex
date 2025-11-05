# Codex Tauri Quick Install
# Fast incremental build and install

$ErrorActionPreference = "Continue"

Write-Host "=== Codex Tauri Quick Install ===" -ForegroundColor Cyan
Write-Host ""

# Step 1: Build
Write-Host "[1/3] Building..." -ForegroundColor Yellow
npm run tauri build

if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

# Step 2: Find MSI
Write-Host ""
Write-Host "[2/3] Finding MSI..." -ForegroundColor Yellow
$msiPath = Get-ChildItem ".\src-tauri\target\release\bundle\msi\*.msi" | Select-Object -First 1

if (-not $msiPath) {
    Write-Host "MSI not found!" -ForegroundColor Red
    exit 1
}

Write-Host "Found: $($msiPath.Name)" -ForegroundColor Green

# Step 3: Install
Write-Host ""
Write-Host "[3/3] Installing..." -ForegroundColor Yellow
Write-Host "This may require admin privileges..." -ForegroundColor Gray

$msiFullPath = $msiPath.FullName

# Silent install with force reinstall
msiexec /i "$msiFullPath" /qb REINSTALL=ALL REINSTALLMODE=vomus

Write-Host ""
Write-Host "=== Installation Complete ===" -ForegroundColor Green
Write-Host ""
Write-Host "Check system tray for Codex icon!" -ForegroundColor Cyan
Write-Host "Run security test: .\test-security.ps1" -ForegroundColor Cyan

