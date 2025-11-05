# Codex Orchestrator Production Monitoring Script
# Version: 0.56.0
# Author: zapabob

param(
    [int]$RefreshInterval = 5,
    [switch]$Json,
    [string]$LogDir = ".codex/logs"
)

$ErrorActionPreference = "Stop"

function Get-OrchestratorStatus {
    $pidFile = Join-Path $LogDir "orchestrator.pid"
    
    if (-not (Test-Path $pidFile)) {
        return @{
            Running = $false
            Message = "PID file not found. Orchestrator may not be running."
        }
    }
    
    $pid = Get-Content $pidFile -Raw | ForEach-Object { $_.Trim() }
    
    try {
        $process = Get-Process -Id $pid -ErrorAction Stop
        
        $status = @{
            Running = $true
            PID = $pid
            StartTime = $process.StartTime
            Uptime = (Get-Date) - $process.StartTime
            CPUTime = $process.TotalProcessorTime
            Memory = [math]::Round($process.WorkingSet64 / 1MB, 2)
            Threads = $process.Threads.Count
            CommandLine = $process.CommandLine
        }
        
        return $status
    } catch {
        return @{
            Running = $false
            PID = $pid
            Message = "Process $pid not found. Orchestrator may have crashed."
        }
    }
}

function Get-OrchestratorLogs {
    param([int]$Lines = 10)
    
    $latestLog = Get-ChildItem -Path $LogDir -Filter "orchestrator_*.log" |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
    
    if ($latestLog) {
        return Get-Content $latestLog.FullName -Tail $Lines
    } else {
        return @("No log files found")
    }
}

function Show-Dashboard {
    param($status)
    
    Clear-Host
    
    Write-Host "╔════════════════════════════════════════════════════════════════╗" -ForegroundColor Cyan
    Write-Host "║         Codex Orchestrator Production Monitor v0.56.0         ║" -ForegroundColor Cyan
    Write-Host "╚════════════════════════════════════════════════════════════════╝" -ForegroundColor Cyan
    Write-Host ""
    
    if ($status.Running) {
        Write-Host "🟢 STATUS: RUNNING" -ForegroundColor Green
        Write-Host ""
        Write-Host "  📊 Process Information" -ForegroundColor Yellow
        Write-Host "  ├─ PID:       $($status.PID)" -ForegroundColor White
        Write-Host "  ├─ Uptime:    $($status.Uptime.ToString('dd\.hh\:mm\:ss'))" -ForegroundColor White
        Write-Host "  ├─ CPU Time:  $($status.CPUTime.ToString('hh\:mm\:ss'))" -ForegroundColor White
        Write-Host "  ├─ Memory:    $($status.Memory) MB" -ForegroundColor White
        Write-Host "  └─ Threads:   $($status.Threads)" -ForegroundColor White
        Write-Host ""
    } else {
        Write-Host "🔴 STATUS: NOT RUNNING" -ForegroundColor Red
        Write-Host ""
        if ($status.PID) {
            Write-Host "  ⚠️  Last known PID: $($status.PID)" -ForegroundColor Yellow
        }
        Write-Host "  ℹ️  $($status.Message)" -ForegroundColor Yellow
        Write-Host ""
    }
    
    Write-Host "  📝 Recent Logs (last 10 lines)" -ForegroundColor Yellow
    Write-Host "  " + ("─" * 60) -ForegroundColor DarkGray
    
    $logs = Get-OrchestratorLogs -Lines 10
    foreach ($line in $logs) {
        if ($line -match "\[ERROR\]") {
            Write-Host "  $line" -ForegroundColor Red
        } elseif ($line -match "\[WARN\]") {
            Write-Host "  $line" -ForegroundColor Yellow
        } else {
            Write-Host "  $line" -ForegroundColor Gray
        }
    }
    
    Write-Host ""
    Write-Host "  🔄 Auto-refresh: ${RefreshInterval}s | Press Ctrl+C to exit" -ForegroundColor Cyan
    Write-Host ""
}

# Main monitoring loop
if ($Json) {
    # JSON output mode (for scripting)
    $status = Get-OrchestratorStatus
    $status | ConvertTo-Json -Depth 10
} else {
    # Interactive dashboard mode
    Write-Host "🔍 Starting Orchestrator monitoring..." -ForegroundColor Cyan
    Write-Host "Press Ctrl+C to stop" -ForegroundColor Yellow
    Start-Sleep -Seconds 1
    
    while ($true) {
        $status = Get-OrchestratorStatus
        Show-Dashboard -status $status
        Start-Sleep -Seconds $RefreshInterval
    }
}


