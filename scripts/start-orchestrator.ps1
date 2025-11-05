# Codex Orchestrator Production Startup Script
# Version: 0.56.0
# Author: zapabob

param(
    [string]$Transport = "tcp",
    [int]$Port = 9876,
    [string]$Socket = "/tmp/codex-orchestrator.sock",
    [string]$Pipe = "\\.\pipe\codex-orchestrator",
    [switch]$Background,
    [switch]$Monitor,
    [string]$LogDir = ".codex/logs"
)

$ErrorActionPreference = "Stop"

Write-Host "🚀 Codex Orchestrator Production Startup" -ForegroundColor Cyan
Write-Host "=========================================" -ForegroundColor Cyan

# Create log directory
if (-not (Test-Path $LogDir)) {
    New-Item -ItemType Directory -Path $LogDir -Force | Out-Null
    Write-Host "✅ Created log directory: $LogDir" -ForegroundColor Green
}

$timestamp = Get-Date -Format "yyyy-MM-dd_HH-mm-ss"
$logFile = Join-Path $LogDir "orchestrator_$timestamp.log"

# Check if codex is installed
$codexPath = Get-Command codex -ErrorAction SilentlyContinue
if (-not $codexPath) {
    Write-Host "❌ codex command not found. Please install globally first:" -ForegroundColor Red
    Write-Host "   cd codex-rs; cargo install --path cli --force" -ForegroundColor Yellow
    exit 1
}

Write-Host "✅ Found codex: $($codexPath.Source)" -ForegroundColor Green

# Check if orchestrator is already running
$existingProcess = Get-Process -Name "codex" -ErrorAction SilentlyContinue | Where-Object {
    $_.CommandLine -like "*orchestrator*"
}

if ($existingProcess) {
    Write-Host "⚠️  Orchestrator already running (PID: $($existingProcess.Id))" -ForegroundColor Yellow
    $response = Read-Host "Stop existing process and restart? (y/N)"
    if ($response -eq "y" -or $response -eq "Y") {
        Stop-Process -Id $existingProcess.Id -Force
        Write-Host "✅ Stopped existing orchestrator" -ForegroundColor Green
        Start-Sleep -Seconds 2
    } else {
        Write-Host "❌ Cancelled. Existing orchestrator is still running." -ForegroundColor Red
        exit 1
    }
}

# Build orchestrator command
$cmdArgs = @("orchestrator", "start")

switch ($Transport) {
    "tcp" {
        $cmdArgs += @("--transport", "tcp", "--port", $Port)
        Write-Host "🌐 Transport: TCP (port $Port)" -ForegroundColor Cyan
    }
    "uds" {
        $cmdArgs += @("--transport", "uds", "--socket", $Socket)
        Write-Host "🔌 Transport: Unix Domain Socket ($Socket)" -ForegroundColor Cyan
    }
    "named-pipe" {
        $cmdArgs += @("--transport", "named-pipe", "--pipe", $Pipe)
        Write-Host "📡 Transport: Named Pipe ($Pipe)" -ForegroundColor Cyan
    }
    default {
        Write-Host "❌ Invalid transport: $Transport" -ForegroundColor Red
        exit 1
    }
}

Write-Host "📝 Log file: $logFile" -ForegroundColor Cyan

# Start orchestrator
if ($Background) {
    Write-Host "🔄 Starting orchestrator in background..." -ForegroundColor Yellow
    
    $processInfo = New-Object System.Diagnostics.ProcessStartInfo
    $processInfo.FileName = "codex"
    $processInfo.Arguments = $cmdArgs -join " "
    $processInfo.RedirectStandardOutput = $true
    $processInfo.RedirectStandardError = $true
    $processInfo.UseShellExecute = $false
    $processInfo.CreateNoWindow = $true
    
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $processInfo
    
    # Redirect output to log file
    Register-ObjectEvent -InputObject $process -EventName OutputDataReceived -Action {
        param($sender, $e)
        if ($e.Data) {
            Add-Content -Path $using:logFile -Value $e.Data
        }
    } | Out-Null
    
    Register-ObjectEvent -InputObject $process -EventName ErrorDataReceived -Action {
        param($sender, $e)
        if ($e.Data) {
            Add-Content -Path $using:logFile -Value "[ERROR] $($e.Data)"
        }
    } | Out-Null
    
    $process.Start() | Out-Null
    $process.BeginOutputReadLine()
    $process.BeginErrorReadLine()
    
    Write-Host "✅ Orchestrator started (PID: $($process.Id))" -ForegroundColor Green
    Write-Host "📊 Monitor logs: tail -f $logFile" -ForegroundColor Cyan
    
    # Save PID for monitoring
    $pidFile = Join-Path $LogDir "orchestrator.pid"
    $process.Id | Out-File -FilePath $pidFile -Encoding utf8
    Write-Host "💾 PID saved to: $pidFile" -ForegroundColor Green
    
    if ($Monitor) {
        Write-Host "`n🔍 Starting monitoring mode..." -ForegroundColor Cyan
        Write-Host "Press Ctrl+C to stop monitoring (orchestrator will keep running)" -ForegroundColor Yellow
        Get-Content -Path $logFile -Wait
    }
} else {
    Write-Host "🔄 Starting orchestrator in foreground..." -ForegroundColor Yellow
    Write-Host "Press Ctrl+C to stop" -ForegroundColor Yellow
    Write-Host ""
    
    & codex @cmdArgs 2>&1 | Tee-Object -FilePath $logFile
}

Write-Host "`n✅ Orchestrator startup complete!" -ForegroundColor Green


