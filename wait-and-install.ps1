# Wait for build and install
# Phase 4: Parallel Execution & Custom Agent

param(
    [int]$MaxWaitMinutes = 15
)

$ErrorActionPreference = "Stop"
$codexBinary = "codex-rs\target\release\codex.exe"
$startTime = Get-Date
$checkInterval = 10 # seconds

Write-Host "======================================" -ForegroundColor Cyan
Write-Host " Phase 4 Build Monitor" -ForegroundColor Cyan
Write-Host "======================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "Waiting for build to complete..." -ForegroundColor Yellow
Write-Host "  Binary: $codexBinary" -ForegroundColor Gray
Write-Host "  Max wait: $MaxWaitMinutes minutes" -ForegroundColor Gray
Write-Host ""

$dotCount = 0
while ($true) {
    $elapsed = (Get-Date) - $startTime
    $elapsedMinutes = [math]::Floor($elapsed.TotalMinutes)
    $elapsedSeconds = [math]::Floor($elapsed.TotalSeconds)
    
    # Check timeout
    if ($elapsed.TotalMinutes -gt $MaxWaitMinutes) {
        Write-Host "`n`nTimeout reached!" -ForegroundColor Red
        Write-Host "Build took longer than $MaxWaitMinutes minutes" -ForegroundColor Yellow
        Write-Host "Check cargo processes: Get-Process cargo" -ForegroundColor White
        exit 1
    }
    
    # Check if binary exists
    if (Test-Path $codexBinary) {
        $fileInfo = Get-Item $codexBinary
        Write-Host "`n`nBuild COMPLETE!" -ForegroundColor Green
        Write-Host "  Time: $elapsedMinutes min $($elapsedSeconds % 60) sec" -ForegroundColor White
        Write-Host "  Size: $([math]::Round($fileInfo.Length / 1MB, 2)) MB" -ForegroundColor White
        Write-Host "  Modified: $($fileInfo.LastWriteTime)" -ForegroundColor White
        Write-Host ""
        break
    }
    
    # Check if cargo is still running
    $cargoProcesses = Get-Process -Name cargo -ErrorAction SilentlyContinue
    if (-not $cargoProcesses) {
        Write-Host "`n`nERROR: No cargo process found!" -ForegroundColor Red
        Write-Host "Build may have failed" -ForegroundColor Yellow
        Write-Host "Check build log:" -ForegroundColor White
        Write-Host "  Get-Content codex-rs\build.log -Tail 50" -ForegroundColor Gray
        exit 1
    }
    
    # Progress indicator
    $dots = "." * ($dotCount % 4)
    $spaces = " " * (3 - ($dotCount % 4))
    Write-Host "`r  Building$dots$spaces ($elapsedSeconds sec, $($cargoProcesses.Count) cargo processes)" -NoNewline -ForegroundColor Yellow
    
    $dotCount++
    Start-Sleep -Seconds $checkInterval
}

# Install
Write-Host "======================================" -ForegroundColor Cyan
Write-Host " Installing..." -ForegroundColor Cyan
Write-Host "======================================" -ForegroundColor Cyan
Write-Host ""

$installDir = "$env:USERPROFILE\.codex\bin"

if (-not (Test-Path $installDir)) {
    Write-Host "Creating install directory..." -ForegroundColor Yellow
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
}

Write-Host "Copying binary..." -ForegroundColor Yellow
Copy-Item -Path $codexBinary -Destination "$installDir\codex.exe" -Force

Write-Host ""
Write-Host "======================================" -ForegroundColor Cyan
Write-Host " Installation COMPLETE!" -ForegroundColor Green
Write-Host "======================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "Verify installation:" -ForegroundColor Yellow
Write-Host "  codex --version" -ForegroundColor White
Write-Host "  codex delegate-parallel --help" -ForegroundColor White
Write-Host "  codex agent-create --help" -ForegroundColor White
Write-Host ""
Write-Host "Test commands:" -ForegroundColor Yellow
Write-Host "  codex delegate researcher 'Find Rust async best practices' --budget 10000" -ForegroundColor White
Write-Host "  codex agent-create 'Count files in current directory' --budget 5000" -ForegroundColor White
Write-Host ""

