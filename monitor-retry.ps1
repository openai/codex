# Monitor Retry Build
$targetBinary = "codex-rs\target\release\codex.exe"
$logFile = "build-retry.log"
$startTime = Get-Date

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Monitoring Retry Build" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$iteration = 0
while ($true) {
    $elapsed = (Get-Date) - $startTime
    $elapsedMin = [math]::Floor($elapsed.TotalMinutes)
    $elapsedSec = [math]::Floor($elapsed.TotalSeconds % 60)
    
    # Check binary
    if (Test-Path $targetBinary) {
        $fileInfo = Get-Item $targetBinary
        $recentWrite = (Get-Date) - $fileInfo.LastWriteTime
        
        if ($recentWrite.TotalSeconds -lt 30) {
            Write-Host "`n`n========================================" -ForegroundColor Green
            Write-Host " BUILD SUCCESS!" -ForegroundColor Green
            Write-Host "========================================" -ForegroundColor Green
            Write-Host "Time: ${elapsedMin}m ${elapsedSec}s" -ForegroundColor White
            Write-Host "Size: $([math]::Round($fileInfo.Length / 1MB, 2)) MB" -ForegroundColor White
            Write-Host "Modified: $($fileInfo.LastWriteTime)" -ForegroundColor White
            Write-Host ""
            Write-Host "Next: .\install-phase4.ps1" -ForegroundColor Cyan
            exit 0
        }
    }
    
    # Check cargo
    $cargoCount = (Get-Process -Name cargo -ErrorAction SilentlyContinue | Measure-Object).Count
    
    if ($cargoCount -eq 0 -and $iteration -gt 2) {
        Write-Host "`n`n========================================" -ForegroundColor Red
        Write-Host " BUILD FAILED" -ForegroundColor Red
        Write-Host "========================================" -ForegroundColor Red
        
        if (Test-Path $logFile) {
            Write-Host "`nLast 20 lines:" -ForegroundColor Yellow
            Get-Content $logFile -Tail 20 | ForEach-Object {
                if ($_ -match "error") {
                    Write-Host "  $_" -ForegroundColor Red
                } else {
                    Write-Host "  $_" -ForegroundColor Gray
                }
            }
        }
        exit 1
    }
    
    # Log info
    $logInfo = ""
    if (Test-Path $logFile) {
        $lastLine = Get-Content $logFile -Tail 1 -ErrorAction SilentlyContinue
        if ($lastLine -match "Compiling (.+)") {
            $logInfo = " | $($Matches[1])"
        }
        elseif ($lastLine -match "Finished") {
            $logInfo = " | Finishing..."
        }
    }
    
    # Progress
    $dots = "." * (($iteration % 4) + 1)
    $spaces = " " * (3 - ($iteration % 4))
    Write-Host "`r[${elapsedMin}m ${elapsedSec}s] Building$dots$spaces ($cargoCount cargo)$logInfo" -NoNewline -ForegroundColor Yellow
    
    $iteration++
    Start-Sleep -Seconds 3
}

