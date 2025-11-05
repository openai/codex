#!/usr/bin/env pwsh
# Monitor clean dev build + install progress

Write-Host "`n=== Monitoring Clean Dev Build & Install ===" -ForegroundColor Cyan
Write-Host ""

$binary = "$env:USERPROFILE\.cargo\bin\codex.exe"
$logFile = "install-clean-dev.log"
$maxWait = 900  # 15 minutes for full rebuild
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
    $cargoProcs = Get-Process cargo -ErrorAction SilentlyContinue
    if ($cargoProcs) {
        Write-Host "  Cargo: $($cargoProcs.Count) processes running" -ForegroundColor Cyan
    } else {
        Write-Host "  Cargo: No processes" -ForegroundColor Gray
    }
    
    # Check binary
    if (Test-Path $binary) {
        $f = Get-Item $binary
        $age = (Get-Date) - $f.LastWriteTime
        $ageSec = [math]::Floor($age.TotalSeconds)
        
        if ($ageSec -lt 60) {
            Write-Host "`n=== BUILD COMPLETE! ===" -ForegroundColor Green
            Write-Host ""
            Write-Host "Binary successfully installed:" -ForegroundColor Cyan
            Write-Host "  Location: $binary" -ForegroundColor White
            Write-Host "  Size: $([math]::Round($f.Length/1MB,2)) MB" -ForegroundColor White
            Write-Host "  Modified: $($f.LastWriteTime)" -ForegroundColor White
            Write-Host ""
            
            # Test installation
            Write-Host "=== Testing Installation ===" -ForegroundColor Cyan
            Write-Host ""
            
            Write-Host "1. Version check:" -ForegroundColor Yellow
            try {
                $version = codex --version
                Write-Host "  $version" -ForegroundColor Green
            } catch {
                Write-Host "  [ERROR] Failed to run codex --version" -ForegroundColor Red
            }
            Write-Host ""
            
            Write-Host "2. New commands check:" -ForegroundColor Yellow
            try {
                $help = codex --help 2>&1 | Out-String
                
                if ($help -match "delegate-parallel") {
                    Write-Host "  [OK] delegate-parallel command found" -ForegroundColor Green
                } else {
                    Write-Host "  [WARN] delegate-parallel not found" -ForegroundColor Yellow
                }
                
                if ($help -match "agent-create") {
                    Write-Host "  [OK] agent-create command found" -ForegroundColor Green
                } else {
                    Write-Host "  [WARN] agent-create not found" -ForegroundColor Yellow
                }
            } catch {
                Write-Host "  [ERROR] Failed to check commands" -ForegroundColor Red
            }
            Write-Host ""
            
            Write-Host "3. Delegate command help:" -ForegroundColor Yellow
            try {
                codex delegate --help | Select-Object -First 3
            } catch {
                Write-Host "  [ERROR] Failed to run delegate --help" -ForegroundColor Red
            }
            Write-Host ""
            
            Write-Host "=== Installation Complete! ===" -ForegroundColor Green
            Write-Host ""
            Write-Host "Ready to use new commands:" -ForegroundColor Cyan
            Write-Host "  1. codex delegate researcher --goal 'Test research' --budget 5000" -ForegroundColor White
            Write-Host "  2. codex delegate-parallel researcher --goals 'Test1,Test2' --budgets 5000,5000" -ForegroundColor White
            Write-Host "  3. codex agent-create 'List all files in current directory' --budget 3000" -ForegroundColor White
            Write-Host ""
            
            break
        } else {
            Write-Host "  Binary: Exists (${ageSec}s old, still building)" -ForegroundColor Gray
        }
    } else {
        Write-Host "  Binary: Not created yet" -ForegroundColor Gray
    }
    
    # Show log progress
    if (Test-Path $logFile) {
        $currentSize = (Get-Item $logFile).Length
        if ($currentSize -ne $lastSize) {
            $lines = Get-Content $logFile -ErrorAction SilentlyContinue -Tail 20
            if ($lines) {
                # Find last meaningful line
                $lastLine = $lines | Where-Object { $_ -match "Compiling|Finished|Installing|error" } | Select-Object -Last 1
                if ($lastLine) {
                    $display = $lastLine.Substring(0, [Math]::Min(80, $lastLine.Length))
                    Write-Host "  Log: $display" -ForegroundColor DarkCyan
                }
            }
            $lastSize = $currentSize
        }
    }
    
    # Check for errors
    if (Test-Path $logFile) {
        $content = Get-Content $logFile -ErrorAction SilentlyContinue -Tail 50
        $errors = $content | Select-String -Pattern "^error(\[|:)"
        if ($errors -and $errors.Count -gt 0) {
            Write-Host "`n  [ERROR] Compilation errors detected!" -ForegroundColor Red
            Write-Host "  Latest errors:" -ForegroundColor Yellow
            $errors | Select-Object -Last 3 | ForEach-Object { 
                Write-Host "    $($_.Line)" -ForegroundColor Red 
            }
            Write-Host "`n  Check install-clean-dev.log for full details" -ForegroundColor Yellow
            Write-Host ""
            break
        }
    }
}

if ($elapsed -ge $maxWait) {
    Write-Host "`n=== Timeout ===" -ForegroundColor Red
    Write-Host "Build did not complete in $maxWait seconds" -ForegroundColor Yellow
    Write-Host "Check install-clean-dev.log for details" -ForegroundColor Cyan
}

Write-Host "`nMonitoring ended." -ForegroundColor Gray
Write-Host ""

