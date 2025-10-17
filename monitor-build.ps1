# Build Monitor
$targetBinary = "codex-rs\target\release\codex.exe"
$logFile = "build-progress.log"
$startTime = Get-Date

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Monitoring Build Progress" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""

$lastSize = 0
$noChangeCount = 0

while ($true) {
    $elapsed = (Get-Date) - $startTime
    $elapsedMin = [math]::Floor($elapsed.TotalMinutes)
    $elapsedSec = [math]::Floor($elapsed.TotalSeconds % 60)
    
    # Check if binary exists
    if (Test-Path $targetBinary) {
        $fileInfo = Get-Item $targetBinary
        Write-Host "`n`n========================================" -ForegroundColor Green
        Write-Host " BUILD COMPLETE!" -ForegroundColor Green
        Write-Host "========================================" -ForegroundColor Green
        Write-Host "Time: ${elapsedMin}m ${elapsedSec}s" -ForegroundColor White
        Write-Host "Size: $([math]::Round($fileInfo.Length / 1MB, 2)) MB" -ForegroundColor White
        Write-Host ""
        Write-Host "Next: .\install-phase4.ps1" -ForegroundColor Cyan
        break
    }
    
    # Check cargo processes
    $cargoCount = (Get-Process -Name cargo -ErrorAction SilentlyContinue | Measure-Object).Count
    
    if ($cargoCount -eq 0) {
        Write-Host "`n`nERROR: No cargo processes!" -ForegroundColor Red
        if (Test-Path $logFile) {
            Write-Host "`nLast 10 lines:" -ForegroundColor Yellow
            Get-Content $logFile -Tail 10
        }
        break
    }
    
    # Check log file
    $logInfo = ""
    if (Test-Path $logFile) {
        $currentSize = (Get-Item $logFile).Length
        if ($currentSize -eq $lastSize) {
            $noChangeCount++
        } else {
            $noChangeCount = 0
            $lastSize = $currentSize
        }
        
        $lastLine = Get-Content $logFile -Tail 1 -ErrorAction SilentlyContinue
        if ($lastLine -match "Compiling (.+)") {
            $logInfo = " | $($Matches[1])"
        }
    }
    
    # Progress
    $dots = "." * (($elapsed.TotalSeconds / 2) % 4)
    Write-Host "`r[${elapsedMin}m ${elapsedSec}s] Building$dots ($cargoCount cargo)$logInfo" -NoNewline -ForegroundColor Yellow
    
    Start-Sleep -Seconds 2
}

