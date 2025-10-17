#!/usr/bin/env pwsh
# Monitor cargo install progress

Write-Host "`n=== Monitoring Installation ===" -ForegroundColor Cyan
Write-Host ""

$binary = "$env:USERPROFILE\.cargo\bin\codex.exe"
$logFile = "install-final.log"
$maxWait = 720  # 12 minutes
$elapsed = 0
$interval = 30
$lastSize = 0

while ($elapsed -lt $maxWait) {
    Start-Sleep -Seconds $interval
    $elapsed += $interval
    
    $min = [math]::Floor($elapsed / 60)
    $sec = $elapsed % 60
    Write-Host "`n[${min}m ${sec}s] Status:" -ForegroundColor Yellow
    
    # Check cargo processes
    $cargo = Get-Process cargo -ErrorAction SilentlyContinue
    if ($cargo) {
        Write-Host "  Cargo: $($cargo.Count) processes running" -ForegroundColor Cyan
    } else {
        Write-Host "  Cargo: No processes (may be done)" -ForegroundColor Gray
    }
    
    # Check job status
    $job = Get-Job | Where-Object { $_.State -eq 'Running' }
    if ($job) {
        Write-Host "  Job: Running" -ForegroundColor Cyan
    } else {
        Write-Host "  Job: Completed" -ForegroundColor Green
    }
    
    # Check binary
    if (Test-Path $binary) {
        $f = Get-Item $binary
        $age = (Get-Date) - $f.LastWriteTime
        $ageSec = [math]::Floor($age.TotalSeconds)
        
        if ($ageSec -lt 60) {
            Write-Host "`n=== BUILD COMPLETE! ===" -ForegroundColor Green
            Write-Host ""
            Write-Host "Binary installed:" -ForegroundColor Cyan
            Write-Host "  Location: $binary" -ForegroundColor White
            Write-Host "  Size: $([math]::Round($f.Length/1MB,2)) MB" -ForegroundColor White
            Write-Host "  Modified: $($f.LastWriteTime)" -ForegroundColor White
            Write-Host ""
            
            # Cleanup jobs
            Get-Job | Stop-Job
            Get-Job | Remove-Job
            
            # Test commands
            Write-Host "=== Testing Installation ===" -ForegroundColor Cyan
            Write-Host ""
            
            Write-Host "1. Version:" -ForegroundColor Yellow
            codex --version
            Write-Host ""
            
            Write-Host "2. Available commands:" -ForegroundColor Yellow
            $help = codex --help
            $newCommands = $help | Select-String -Pattern "(delegate-parallel|agent-create)"
            if ($newCommands) {
                Write-Host "  [OK] New commands found:" -ForegroundColor Green
                $newCommands | ForEach-Object { Write-Host "    - $($_.Line.Trim())" -ForegroundColor White }
            } else {
                Write-Host "  [WARN] New commands not found" -ForegroundColor Yellow
            }
            Write-Host ""
            
            Write-Host "3. Test delegate command:" -ForegroundColor Yellow
            codex delegate --help | Select-Object -First 5
            Write-Host ""
            
            Write-Host "=== Installation Complete! ===" -ForegroundColor Green
            Write-Host ""
            Write-Host "Ready to use:" -ForegroundColor Cyan
            Write-Host "  codex delegate researcher --goal 'Test' --budget 5000" -ForegroundColor Gray
            Write-Host "  codex delegate-parallel researcher --goals 'Test' --budgets 5000" -ForegroundColor Gray
            Write-Host "  codex agent-create 'List files' --budget 3000" -ForegroundColor Gray
            Write-Host ""
            
            break
        } else {
            Write-Host "  Binary: Exists but old (${ageSec}s ago)" -ForegroundColor Gray
        }
    } else {
        Write-Host "  Binary: Not created yet" -ForegroundColor Gray
    }
    
    # Show log progress
    if (Test-Path $logFile) {
        $currentSize = (Get-Item $logFile).Length
        if ($currentSize -ne $lastSize) {
            $lines = Get-Content $logFile -ErrorAction SilentlyContinue
            if ($lines -and $lines.Count -gt 0) {
                $lastLine = $lines | Select-Object -Last 1
                if ($lastLine -and $lastLine.Length -gt 0) {
                    $display = $lastLine.Substring(0, [Math]::Min(70, $lastLine.Length))
                    Write-Host "  Log: $display" -ForegroundColor DarkGray
                }
            }
            $lastSize = $currentSize
        }
    }
    
    # Check for errors in log
    if (Test-Path $logFile) {
        $content = Get-Content $logFile -ErrorAction SilentlyContinue
        $errors = $content | Select-String -Pattern "^error:"
        if ($errors -and $errors.Count -gt 0) {
            Write-Host "`n  [WARNING] Errors detected in log!" -ForegroundColor Red
            Write-Host "  Latest error:" -ForegroundColor Yellow
            $errors | Select-Object -Last 1 | ForEach-Object { Write-Host "    $($_.Line)" -ForegroundColor Red }
        }
    }
}

if ($elapsed -ge $maxWait) {
    Write-Host "`n=== Timeout ===" -ForegroundColor Red
    Write-Host "Build did not complete in time" -ForegroundColor Yellow
    Write-Host "Check install-complete.log for details" -ForegroundColor Cyan
    
    # Cleanup
    Get-Job | Stop-Job
    Get-Job | Remove-Job
}

