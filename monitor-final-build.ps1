#!/usr/bin/env pwsh
# Monitor Final Build and Auto-Test

Write-Host "`n=== Monitoring Final Build ===" -ForegroundColor Cyan
Write-Host ""

$maxWait = 600  # 10 minutes
$elapsed = 0
$interval = 20

while ($elapsed -lt $maxWait) {
    Start-Sleep -Seconds $interval
    $elapsed += $interval
    
    Write-Host "[${elapsed}s] Checking..." -ForegroundColor Gray
    
    # Check cargo processes
    $cargo = Get-Process cargo -ErrorAction SilentlyContinue
    if ($cargo) {
        Write-Host "  Cargo: $($cargo.Count) processes (building...)" -ForegroundColor Yellow
    } else {
        Write-Host "  Cargo: Done" -ForegroundColor Green
    }
    
    # Check binary
    $binary = "$env:USERPROFILE\.cargo\bin\codex.exe"
    if (Test-Path $binary) {
        $f = Get-Item $binary
        $age = (Get-Date) - $f.LastWriteTime
        $ageMin = [math]::Floor($age.TotalMinutes)
        
        if ($ageMin -lt 5) {
            Write-Host "`nBUILD COMPLETE!" -ForegroundColor Green
            Write-Host "Binary: $([math]::Round($f.Length/1MB,2))MB at $($f.LastWriteTime)" -ForegroundColor Cyan
            Write-Host ""
            
            # Test commands
            Write-Host "=== Testing Commands ===" -ForegroundColor Cyan
            Write-Host "`n1. Version:" -ForegroundColor Yellow
            codex --version
            
            Write-Host "`n2. New Commands:" -ForegroundColor Yellow
            $help = codex --help
            $delegateParallel = $help | Select-String "delegate-parallel"
            $agentCreate = $help | Select-String "agent-create"
            
            if ($delegateParallel) {
                Write-Host "  [OK] delegate-parallel found" -ForegroundColor Green
            } else {
                Write-Host "  [FAIL] delegate-parallel NOT found" -ForegroundColor Red
            }
            
            if ($agentCreate) {
                Write-Host "  [OK] agent-create found" -ForegroundColor Green
            } else {
                Write-Host "  [FAIL] agent-create NOT found" -ForegroundColor Red
            }
            
            Write-Host "`n3. delegate-parallel help:" -ForegroundColor Yellow
            codex delegate-parallel --help | Select-Object -First 8
            
            Write-Host "`n4. agent-create help:" -ForegroundColor Yellow
            codex agent-create --help | Select-Object -First 8
            
            Write-Host "`n=== Installation Complete! ===" -ForegroundColor Green
            Write-Host ""
            Write-Host "Ready to test:" -ForegroundColor Cyan
            Write-Host "  codex delegate researcher --goal 'Test' --budget 5000" -ForegroundColor Gray
            Write-Host "  codex delegate-parallel researcher --goals 'Test' --budgets 5000" -ForegroundColor Gray
            Write-Host "  codex agent-create 'List files' --budget 3000" -ForegroundColor Gray
            Write-Host ""
            break
        }
    } else {
        Write-Host "  Binary: Not ready" -ForegroundColor DarkGray
    }
    
    # Show log tail
    if (Test-Path "build-final-install.log") {
        $lastLine = Get-Content "build-final-install.log" -Tail 1 -ErrorAction SilentlyContinue
        if ($lastLine) {
            Write-Host "  Latest: $lastLine" -ForegroundColor DarkGray
        }
    }
}

if ($elapsed -ge $maxWait) {
    Write-Host "`nTimeout!" -ForegroundColor Red
    Write-Host "Check build-final-install.log for details" -ForegroundColor Yellow
}

