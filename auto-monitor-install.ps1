#!/usr/bin/env pwsh
# Auto-monitor Build & Test

Write-Host "`n=== Auto Build Monitor & Test ===" -ForegroundColor Cyan
Write-Host ""

$binary = "$env:USERPROFILE\.cargo\bin\codex.exe"
$logFile = "build-clean-install.log"
$maxWait = 900  # 15 minutes
$elapsed = 0
$interval = 20

Write-Host "Monitoring build progress..." -ForegroundColor Yellow
Write-Host "Binary target: $binary" -ForegroundColor Gray
Write-Host "Log file: $logFile" -ForegroundColor Gray
Write-Host ""

while ($elapsed -lt $maxWait) {
    Start-Sleep -Seconds $interval
    $elapsed += $interval
    
    $min = [math]::Floor($elapsed / 60)
    $sec = $elapsed % 60
    Write-Host "[${min}m ${sec}s] " -NoNewline -ForegroundColor Yellow
    
    # Check cargo
    $cargo = Get-Process cargo -ErrorAction SilentlyContinue
    if ($cargo) {
        Write-Host "Building... ($($cargo.Count) processes) " -NoNewline -ForegroundColor Cyan
    } else {
        Write-Host "Cargo done! " -NoNewline -ForegroundColor Green
    }
    
    # Check binary
    if (Test-Path $binary) {
        $f = Get-Item $binary
        $age = (Get-Date) - $f.LastWriteTime
        
        if ($age.TotalSeconds -lt 90) {
            Write-Host "`n`n=== BUILD COMPLETE! ===" -ForegroundColor Green
            Write-Host ""
            Write-Host "Binary installed successfully!" -ForegroundColor Cyan
            Write-Host "  Location: $binary" -ForegroundColor White
            Write-Host "  Size: $([math]::Round($f.Length/1MB,2)) MB" -ForegroundColor White
            Write-Host "  Modified: $($f.LastWriteTime)" -ForegroundColor White
            Write-Host ""
            
            # Auto Test
            Write-Host "=== Auto Testing ===" -ForegroundColor Cyan
            Write-Host ""
            
            Write-Host "Test 1: Version" -ForegroundColor Yellow
            $version = codex --version
            Write-Host "  $version" -ForegroundColor White
            Write-Host ""
            
            Write-Host "Test 2: Check new commands" -ForegroundColor Yellow
            $help = codex --help
            $parallel = $help | Select-String "delegate-parallel"
            $create = $help | Select-String "agent-create"
            
            if ($parallel) {
                Write-Host "  [OK] delegate-parallel found" -ForegroundColor Green
            } else {
                Write-Host "  [FAIL] delegate-parallel NOT found" -ForegroundColor Red
            }
            
            if ($create) {
                Write-Host "  [OK] agent-create found" -ForegroundColor Green
            } else {
                Write-Host "  [FAIL] agent-create NOT found" -ForegroundColor Red
            }
            Write-Host ""
            
            Write-Host "Test 3: delegate-parallel help" -ForegroundColor Yellow
            codex delegate-parallel --help | Select-Object -First 10
            Write-Host ""
            
            Write-Host "Test 4: agent-create help" -ForegroundColor Yellow
            codex agent-create --help | Select-Object -First 10
            Write-Host ""
            
            Write-Host "=== Installation Complete! ===" -ForegroundColor Green
            Write-Host ""
            Write-Host "Ready to use:" -ForegroundColor Cyan
            Write-Host "  codex delegate researcher --goal 'Test' --budget 5000" -ForegroundColor Gray
            Write-Host "  codex delegate-parallel researcher --goals 'Test'" -ForegroundColor Gray
            Write-Host "  codex agent-create 'List PowerShell files' --budget 3000" -ForegroundColor Gray
            Write-Host ""
            
            break
        }
    }
    
    # Show log progress
    if (Test-Path $logFile) {
        $lines = Get-Content $logFile -ErrorAction SilentlyContinue
        if ($lines -and $lines.Count -gt 0) {
            $lastLine = $lines | Select-Object -Last 1
            if ($lastLine -and $lastLine.Length -gt 0) {
                $display = $lastLine.Substring(0, [Math]::Min(60, $lastLine.Length))
                Write-Host "$display" -ForegroundColor DarkGray
            }
        }
        
        # Check for errors
        $errors = $lines | Select-String -Pattern "^error:"
        if ($errors -and $errors.Count -gt 0) {
            Write-Host "`n[ERROR] Build failed!" -ForegroundColor Red
            $errors | Select-Object -Last 3 | ForEach-Object { Write-Host "  $($_.Line)" -ForegroundColor Red }
            Write-Host ""
            break
        }
    } else {
        Write-Host "Waiting for log..." -ForegroundColor Gray
    }
}

if ($elapsed -ge $maxWait) {
    Write-Host "`n`n=== Timeout ===" -ForegroundColor Red
    Write-Host "Build did not complete in $($maxWait/60) minutes" -ForegroundColor Yellow
    Write-Host "Check $logFile for details" -ForegroundColor Cyan
}

