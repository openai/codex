# Wait for build completion and auto-install
$targetBinary = "codex-rs\target\release\codex.exe"
$maxWaitMinutes = 10
$startTime = Get-Date

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Waiting for Build Completion" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

while ($true) {
    $elapsed = (Get-Date) - $startTime
    $elapsedSec = [math]::Floor($elapsed.TotalSeconds)
    
    # Timeout check
    if ($elapsed.TotalMinutes -gt $maxWaitMinutes) {
        Write-Host "`n`nTimeout! ($maxWaitMinutes min)" -ForegroundColor Red
        exit 1
    }
    
    # Check binary
    if (Test-Path $targetBinary) {
        $fileInfo = Get-Item $targetBinary
        $age = (Get-Date) - $fileInfo.LastWriteTime
        
        # Fresh binary (created within last 60 seconds)
        if ($age.TotalSeconds -lt 60) {
            Write-Host "`n`n========================================" -ForegroundColor Green
            Write-Host " BUILD SUCCESS!" -ForegroundColor Green
            Write-Host "========================================" -ForegroundColor Green
            Write-Host ""
            Write-Host "Build Time: $([math]::Floor($elapsed.TotalMinutes))m $([math]::Floor($elapsed.TotalSeconds % 60))s" -ForegroundColor White
            Write-Host "Binary Size: $([math]::Round($fileInfo.Length / 1MB, 2)) MB" -ForegroundColor White
            Write-Host "Modified: $($fileInfo.LastWriteTime)" -ForegroundColor White
            Write-Host ""
            
            # Auto-install
            Write-Host "Installing..." -ForegroundColor Yellow
            $installDir = "$env:USERPROFILE\.codex\bin"
            if (-not (Test-Path $installDir)) {
                New-Item -ItemType Directory -Path $installDir -Force | Out-Null
            }
            Copy-Item -Path $targetBinary -Destination "$installDir\codex.exe" -Force
            
            Write-Host ""
            Write-Host "========================================" -ForegroundColor Cyan
            Write-Host " INSTALLATION COMPLETE!" -ForegroundColor Green
            Write-Host "========================================" -ForegroundColor Cyan
            Write-Host ""
            Write-Host "Test commands:" -ForegroundColor Yellow
            Write-Host "  codex --version" -ForegroundColor White
            Write-Host "  codex delegate-parallel --help" -ForegroundColor White
            Write-Host "  codex agent-create --help" -ForegroundColor White
            Write-Host ""
            exit 0
        }
    }
    
    # Check cargo processes
    $cargoCount = (Get-Process cargo -ErrorAction SilentlyContinue).Count
    
    if ($cargoCount -eq 0) {
        Write-Host "`n`nNo cargo processes!" -ForegroundColor Red
        if (Test-Path $targetBinary) {
            Write-Host "But binary exists (old build)" -ForegroundColor Yellow
        } else {
            Write-Host "Build may have failed" -ForegroundColor Red
            if (Test-Path "build-retry.log") {
                Write-Host "`nLast 10 lines:" -ForegroundColor Yellow
                Get-Content "build-retry.log" -Tail 10
            }
        }
        exit 1
    }
    
    # Progress indicator
    $dots = "." * (($elapsedSec / 2) % 4 + 1)
    Write-Host "`r[$elapsedSec s] Building$dots ($cargoCount cargo)" -NoNewline -ForegroundColor Yellow
    
    Start-Sleep -Seconds 2
}

