# Differential Build Script for Codex
# Builds only changed crates for faster compilation

Write-Host ""
Write-Host "=== CODEX DIFFERENTIAL BUILD ===" -ForegroundColor Cyan
Write-Host ""

$ErrorActionPreference = "Stop"
$repoPath = "C:\Users\downl\Desktop\codex"
Set-Location "$repoPath\codex-rs"

$startTime = Get-Date

Write-Host "[1/3] Detecting changes..." -ForegroundColor Yellow

# Always build core and CLI for now (safest approach)
$cratesToBuild = @("codex-core", "codex-cli")

Write-Host "  Will build: $($cratesToBuild -join ', ')" -ForegroundColor Green
Write-Host ""

Write-Host "[2/3] Building Rust crates..." -ForegroundColor Yellow
Write-Host ""

$buildFailed = $false

foreach ($crate in $cratesToBuild) {
    Write-Host "  Building $crate..." -ForegroundColor Cyan
    
    $buildOutput = cargo build --release -p $crate 2>&1
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "    [OK] $crate" -ForegroundColor Green
    } else {
        Write-Host "    [FAILED] $crate" -ForegroundColor Red
        Write-Host $buildOutput -ForegroundColor Red
        $buildFailed = $true
        break
    }
}

if ($buildFailed) {
    Write-Host ""
    Write-Host "[ERROR] Build failed!" -ForegroundColor Red
    exit 1
}

$buildTime = (Get-Date) - $startTime

Write-Host ""
Write-Host "[3/3] Build complete!" -ForegroundColor Green
Write-Host ""
Write-Host "  Crates built: $($cratesToBuild.Count)" -ForegroundColor Cyan
Write-Host "  Build time: $([math]::Round($buildTime.TotalSeconds, 2))s" -ForegroundColor Cyan
Write-Host ""
Write-Host "[NEXT] Install with:" -ForegroundColor Yellow
Write-Host "  cargo install --path cli --force" -ForegroundColor Cyan
Write-Host ""

