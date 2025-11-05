# 🎯 Cargo Build Progress Monitor
# Usage: .\monitor-build.ps1

param(
    [int]$RefreshSeconds = 3
)

Write-Host "🎯 Cargo Build Progress Monitor" -ForegroundColor Cyan
Write-Host "Press Ctrl+C to stop monitoring`n" -ForegroundColor Yellow

$startTime = Get-Date

while ($true) {
    Clear-Host
    
    $elapsed = [math]::Round(((Get-Date) - $startTime).TotalMinutes, 1)
    
    Write-Host "============================================================" -ForegroundColor Cyan
    Write-Host "  Cargo Build Monitor - Elapsed: $elapsed min" -ForegroundColor Green
    Write-Host "============================================================" -ForegroundColor Cyan
    Write-Host ""
    
    # プロセス状態
    $cargoProc = Get-Process cargo -ErrorAction SilentlyContinue
    $rustcProc = Get-Process rustc -ErrorAction SilentlyContinue
    
    if ($cargoProc -or $rustcProc) {
        Write-Host "[Process Status]" -ForegroundColor Yellow
        Write-Host "------------------------------------------------------------" -ForegroundColor DarkGray
        
        if ($cargoProc) {
            foreach ($proc in $cargoProc) {
                $runtime = [math]::Round(((Get-Date) - $proc.StartTime).TotalMinutes, 1)
                $cpu = [math]::Round($proc.CPU, 1)
                $memMB = [math]::Round($proc.WorkingSet / 1MB, 0)
                
                Write-Host ("  cargo   PID:{0,-6} CPU:{1,6}%  Mem:{2,5}MB  Runtime:{3,4}min" -f $proc.Id, $cpu, $memMB, $runtime) -ForegroundColor White
            }
        }
        
        if ($rustcProc) {
            foreach ($proc in $rustcProc) {
                $runtime = [math]::Round(((Get-Date) - $proc.StartTime).TotalMinutes, 1)
                $cpu = [math]::Round($proc.CPU, 1)
                $memMB = [math]::Round($proc.WorkingSet / 1MB, 0)
                
                $status = if ($proc.Responding) { "[OK] Active" } else { "[WARN] Hang" }
                Write-Host ("  rustc   PID:{0,-6} CPU:{1,6}%  Mem:{2,5}MB  Runtime:{3,4}min  {4}" -f $proc.Id, $cpu, $memMB, $runtime, $status) -ForegroundColor Cyan
            }
        }
        
        Write-Host ""
        
        # メモリ状況
        $mem = Get-WmiObject Win32_OperatingSystem
        $totalMB = [math]::Round($mem.TotalVisibleMemorySize/1KB, 0)
        $freeMB = [math]::Round($mem.FreePhysicalMemory/1KB, 0)
        $usedMB = $totalMB - $freeMB
        $usedPercent = [math]::Round($usedMB/$totalMB*100, 1)
        
        Write-Host "[Memory Usage]" -ForegroundColor Yellow
        Write-Host "------------------------------------------------------------" -ForegroundColor DarkGray
        
        $barLength = 50
        $filledLength = [math]::Floor($barLength * $usedPercent / 100)
        $bar = "#" * $filledLength + "-" * ($barLength - $filledLength)
        
        $memColor = if ($usedPercent -gt 90) { "Red" } elseif ($usedPercent -gt 75) { "Yellow" } else { "Green" }
        Write-Host "  [$bar] $usedPercent%" -ForegroundColor $memColor
        Write-Host ("  Used: {0:N0} MB / Total: {1:N0} MB (Free: {2:N0} MB)" -f $usedMB, $totalMB, $freeMB) -ForegroundColor White
        Write-Host ""
        
        # CPU状況（rustcのCPU使用率から推定）
        if ($rustcProc) {
            $maxCpu = ($rustcProc | Measure-Object -Property CPU -Maximum).Maximum
            $cpuPercent = [math]::Min([math]::Round($maxCpu / $elapsed / 10, 1), 100)
            
            Write-Host "[Build Activity]" -ForegroundColor Yellow
            Write-Host "------------------------------------------------------------" -ForegroundColor DarkGray
            
            if ($maxCpu -gt 300) {
                Write-Host "  [HIGH] LTO Optimization Phase" -ForegroundColor Red
                Write-Host "         (Link-Time Optimization in progress)" -ForegroundColor Gray
            } elseif ($maxCpu -gt 100) {
                Write-Host "  [MEDIUM] Compiling" -ForegroundColor Yellow
            } elseif ($maxCpu -gt 10) {
                Write-Host "  [LOW] Preparing" -ForegroundColor Green
            } else {
                Write-Host "  [MINIMAL] May be hung" -ForegroundColor DarkYellow
            }
            Write-Host ""
        }
        
        # 完成ファイル確認
        Write-Host "[Build Artifacts]" -ForegroundColor Yellow
        Write-Host "------------------------------------------------------------" -ForegroundColor DarkGray
        
        $releaseExe = Get-ChildItem "target\release\codex.exe" -ErrorAction SilentlyContinue
        if ($releaseExe) {
            $sizeMB = [math]::Round($releaseExe.Length / 1MB, 2)
            $age = [math]::Round(((Get-Date) - $releaseExe.LastWriteTime).TotalMinutes, 1)
            Write-Host ("  [OK] codex.exe - {0} MB (updated {1} min ago)" -f $sizeMB, $age) -ForegroundColor Green
        } else {
            Write-Host "  [PENDING] codex.exe - Not yet created" -ForegroundColor DarkGray
        }
        
        Write-Host ""
        Write-Host "------------------------------------------------------------" -ForegroundColor DarkGray
        Write-Host ("  Next update in {0} seconds... (Ctrl+C to stop)" -f $RefreshSeconds) -ForegroundColor DarkGray
        
    } else {
        Write-Host "[BUILD COMPLETE!]" -ForegroundColor Green
        Write-Host ""
        Write-Host "No cargo/rustc processes running." -ForegroundColor White
        Write-Host "Total elapsed time: $elapsed minutes" -ForegroundColor Cyan
        
        # 最終結果確認
        $releaseExe = Get-ChildItem "target\release\codex.exe" -ErrorAction SilentlyContinue
        if ($releaseExe) {
            $sizeMB = [math]::Round($releaseExe.Length / 1MB, 2)
            Write-Host ""
            Write-Host "[Output] codex.exe ($sizeMB MB)" -ForegroundColor Green
            Write-Host "         $($releaseExe.FullName)" -ForegroundColor Gray
        }
        
        break
    }
    
    Start-Sleep -Seconds $RefreshSeconds
}

Write-Host ""
Write-Host "Monitor stopped." -ForegroundColor Yellow

