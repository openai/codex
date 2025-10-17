#!/usr/bin/env pwsh
# Auto Install Dev Build

Write-Host "`n=== Auto Install Dev Build ===" -ForegroundColor Cyan
Write-Host "Monitoring: codex-rs/target/debug/codex.exe" -ForegroundColor Yellow
Write-Host ""

$maxWait = 600  # 10 minutes
$elapsed = 0
$interval = 15

while ($elapsed -lt $maxWait) {
    Start-Sleep -Seconds $interval
    $elapsed += $interval
    
    Write-Host "[${elapsed}s] Checking build status..." -ForegroundColor Gray
    
    # Check if binary exists and is fresh
    if (Test-Path "codex-rs\target\debug\codex.exe") {
        $binary = Get-Item "codex-rs\target\debug\codex.exe"
        $age = (Get-Date) - $binary.LastWriteTime
        $size = [math]::Round($binary.Length / 1MB, 2)
        
        if ($age.TotalSeconds -lt 30) {
            Write-Host "`nBUILD COMPLETE!" -ForegroundColor Green
            Write-Host "Binary: ${size}MB at $($binary.LastWriteTime)" -ForegroundColor Cyan
            
            # Install
            Write-Host "`nInstalling to ~/.codex/bin/..." -ForegroundColor Yellow
            $targetDir = "$env:USERPROFILE\.codex\bin"
            New-Item -ItemType Directory -Path $targetDir -Force | Out-Null
            Copy-Item $binary.FullName "$targetDir\codex.exe" -Force
            
            Write-Host "Installed!" -ForegroundColor Green
            
            # Test
            Write-Host "`nTesting commands..." -ForegroundColor Cyan
            Write-Host "Version:" -ForegroundColor Gray
            & "$targetDir\codex.exe" --version
            
            Write-Host "`nChecking new commands:" -ForegroundColor Gray
            $helpOut = & "$targetDir\codex.exe" --help 2>&1
            $hasDelegatePar = $helpOut | Select-String "delegate-parallel"
            $hasAgentCreate = $helpOut | Select-String "agent-create"
            
            if ($hasDelegatePar) {
                Write-Host "  [OK] delegate-parallel found" -ForegroundColor Green
            } else {
                Write-Host "  [MISSING] delegate-parallel not found" -ForegroundColor Red
            }
            
            if ($hasAgentCreate) {
                Write-Host "  [OK] agent-create found" -ForegroundColor Green
            } else {
                Write-Host "  [MISSING] agent-create not found" -ForegroundColor Red
            }
            
            Write-Host "`nShowing help for new commands:" -ForegroundColor Cyan
            Write-Host "`n--- delegate-parallel ---" -ForegroundColor Yellow
            & "$targetDir\codex.exe" delegate-parallel --help 2>&1 | Select-Object -First 10
            
            Write-Host "`n--- agent-create ---" -ForegroundColor Yellow
            & "$targetDir\codex.exe" agent-create --help 2>&1 | Select-Object -First 10
            
            Write-Host "`n=== Installation Complete! ===" -ForegroundColor Green
            break
        }
    }
    
    # Show progress
    $cargo = Get-Process cargo -ErrorAction SilentlyContinue
    if ($cargo) {
        Write-Host "  Cargo: $($cargo.Count) processes running" -ForegroundColor Gray
    } else {
        Write-Host "  Cargo: No processes (waiting for binary...)" -ForegroundColor Yellow
    }
    
    # Show log tail
    if (Test-Path "build-clean-dev.log") {
        $lastLine = Get-Content "build-clean-dev.log" -Tail 1 -ErrorAction SilentlyContinue
        if ($lastLine) {
            Write-Host "  Latest: $lastLine" -ForegroundColor DarkGray
        }
    }
}

if ($elapsed -ge $maxWait) {
    Write-Host "`nTimeout reached!" -ForegroundColor Red
    Write-Host "Check build-clean-dev.log for details" -ForegroundColor Yellow
}

