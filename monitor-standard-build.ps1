#!/usr/bin/env pwsh
# Standard Build Monitor

Write-Host "`n=== Standard Build Monitor ===" -ForegroundColor Cyan
Write-Host "Monitoring: codex-rs/target/release/codex.exe" -ForegroundColor Yellow
Write-Host ""

$maxWait = 600  # 10 minutes
$elapsed = 0
$interval = 10

while ($elapsed -lt $maxWait) {
    Clear-Host
    Write-Host "`n=== Standard Build Monitor ===" -ForegroundColor Cyan
    Write-Host "Elapsed: $elapsed seconds / $maxWait seconds max" -ForegroundColor Yellow
    Write-Host ""
    
    # Check cargo processes
    $cargoProcesses = Get-Process cargo -ErrorAction SilentlyContinue
    if ($cargoProcesses) {
        Write-Host "Cargo Processes: $($cargoProcesses.Count)" -ForegroundColor Green
        foreach ($proc in $cargoProcesses) {
            $cpu = [math]::Round($proc.CPU, 2)
            $mem = [math]::Round($proc.WorkingSet64 / 1MB, 2)
            Write-Host "  PID $($proc.Id): CPU=${cpu}s, MEM=${mem}MB" -ForegroundColor Gray
        }
    } else {
        Write-Host "Cargo Processes: 0 (Build Complete or Failed)" -ForegroundColor Yellow
    }
    
    Write-Host ""
    
    # Check binary
    if (Test-Path "codex-rs\target\release\codex.exe") {
        $binary = Get-Item "codex-rs\target\release\codex.exe"
        $size = [math]::Round($binary.Length / 1MB, 2)
        $age = (Get-Date) - $binary.LastWriteTime
        
        Write-Host "Binary Status: EXISTS" -ForegroundColor Green
        Write-Host "  Size: ${size} MB" -ForegroundColor Gray
        Write-Host "  Modified: $($binary.LastWriteTime)" -ForegroundColor Gray
        Write-Host "  Age: $([math]::Floor($age.TotalMinutes)) min $([math]::Floor($age.TotalSeconds % 60)) sec" -ForegroundColor Gray
        
        if ($age.TotalMinutes -lt 2 -and -not $cargoProcesses) {
            Write-Host "`nBUILD COMPLETE!" -ForegroundColor Green
            Write-Host "Press any key to install..." -ForegroundColor Yellow
            $null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
            
            Write-Host "`nInstalling..." -ForegroundColor Cyan
            & ".\install-phase4.ps1"
            
            Write-Host "`nTesting commands..." -ForegroundColor Cyan
            codex --version
            codex --help | Select-String "delegate-parallel|agent-create"
            
            break
        }
    } else {
        Write-Host "Binary Status: NOT READY" -ForegroundColor Yellow
    }
    
    Write-Host ""
    
    # Show log tail
    if (Test-Path "build-standard.log") {
        Write-Host "Build Log (last 5 lines):" -ForegroundColor Cyan
        Get-Content "build-standard.log" -Tail 5 -ErrorAction SilentlyContinue | ForEach-Object {
            Write-Host "  $_" -ForegroundColor Gray
        }
    }
    
    Write-Host "`nPress Ctrl+C to stop monitoring" -ForegroundColor DarkGray
    
    Start-Sleep -Seconds $interval
    $elapsed += $interval
}

if ($elapsed -ge $maxWait) {
    Write-Host "`nTimeout reached!" -ForegroundColor Red
}

