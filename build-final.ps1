# Phase 4 Final Build Script
# Ensures correct directory and builds successfully

$ErrorActionPreference = "Stop"

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Phase 4: Final Build" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

# Get absolute paths
$rootDir = $PSScriptRoot
$codexRsDir = Join-Path $rootDir "codex-rs"
$cargoToml = Join-Path $codexRsDir "Cargo.toml"
$targetBinary = Join-Path $codexRsDir "target\release\codex.exe"

Write-Host "[1] Directory Check" -ForegroundColor Yellow
Write-Host "  Root: $rootDir" -ForegroundColor Gray
Write-Host "  CodexRS: $codexRsDir" -ForegroundColor Gray

if (-not (Test-Path $codexRsDir)) {
    Write-Host "  ERROR: codex-rs not found!" -ForegroundColor Red
    exit 1
}

if (-not (Test-Path $cargoToml)) {
    Write-Host "  ERROR: Cargo.toml not found!" -ForegroundColor Red
    exit 1
}

Write-Host "  OK: All directories found" -ForegroundColor Green
Write-Host ""

Write-Host "[2] Clean Release" -ForegroundColor Yellow
$releaseDir = Join-Path $codexRsDir "target\release"
if (Test-Path $releaseDir) {
    Remove-Item -Recurse -Force $releaseDir -ErrorAction SilentlyContinue
    Write-Host "  Cleaned release directory" -ForegroundColor Gray
}
Write-Host ""

Write-Host "[3] Starting Build" -ForegroundColor Yellow
Write-Host "  Command: cargo build --release -p codex-cli" -ForegroundColor Gray
Write-Host "  Working Directory: $codexRsDir" -ForegroundColor Gray
Write-Host "  Estimated time: 5-10 minutes" -ForegroundColor Gray
Write-Host ""

$buildStart = Get-Date

try {
    Push-Location $codexRsDir
    
    $buildOutput = cargo build --release -p codex-cli 2>&1
    $buildExitCode = $LASTEXITCODE
    
    Pop-Location
    
    $buildEnd = Get-Date
    $buildDuration = ($buildEnd - $buildStart).TotalSeconds
    
    Write-Host ""
    Write-Host "[4] Build Result" -ForegroundColor Yellow
    
    if ($buildExitCode -eq 0 -and (Test-Path $targetBinary)) {
        $fileInfo = Get-Item $targetBinary
        Write-Host ""
        Write-Host "  BUILD SUCCESS!" -ForegroundColor Green
        Write-Host "  Time: $([math]::Floor($buildDuration / 60))m $([math]::Floor($buildDuration % 60))s" -ForegroundColor White
        Write-Host "  Size: $([math]::Round($fileInfo.Length / 1MB, 2)) MB" -ForegroundColor White
        Write-Host "  Path: $targetBinary" -ForegroundColor White
        Write-Host ""
        Write-Host "========================================" -ForegroundColor Cyan
        Write-Host " Ready to Install!" -ForegroundColor Green
        Write-Host "========================================" -ForegroundColor Cyan
        Write-Host ""
        Write-Host "Run: .\install-phase4.ps1" -ForegroundColor Cyan
        Write-Host ""
        exit 0
    }
    else {
        Write-Host ""
        Write-Host "  BUILD FAILED!" -ForegroundColor Red
        Write-Host "  Exit code: $buildExitCode" -ForegroundColor Red
        Write-Host ""
        Write-Host "Last 20 lines of output:" -ForegroundColor Yellow
        $buildOutput | Select-Object -Last 20 | ForEach-Object { Write-Host "  $_" -ForegroundColor Gray }
        Write-Host ""
        exit 1
    }
}
catch {
    Pop-Location
    Write-Host ""
    Write-Host "  BUILD ERROR!" -ForegroundColor Red
    Write-Host "  $_" -ForegroundColor Red
    exit 1
}

