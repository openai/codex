# Codex GUI Launcher Script
# Starts the GUI server and opens browser

$ErrorActionPreference = "Stop"

$codexGuiPath = Join-Path $env:USERPROFILE ".cargo\bin\codex-gui.exe"
$port = 8787
$url = "http://localhost:$port"

Write-Host "Starting Codex GUI..." -ForegroundColor Cyan

# Check if GUI executable exists
if (-not (Test-Path $codexGuiPath)) {
    Write-Host "Error: codex-gui.exe not found: $codexGuiPath" -ForegroundColor Red
    Write-Host "Please build and install GUI first." -ForegroundColor Yellow
    exit 1
}

# Check if port is already in use
$portInUse = Get-NetTCPConnection -LocalPort $port -ErrorAction SilentlyContinue
if ($portInUse) {
    Write-Host "Port $port is already in use. GUI may already be running." -ForegroundColor Yellow
    Write-Host "Opening browser to: $url" -ForegroundColor Cyan
    Start-Process $url
    exit 0
}

# Start GUI server in background
Write-Host "Starting GUI server on port $port..." -ForegroundColor Yellow
$guiProcess = Start-Process -FilePath $codexGuiPath -NoNewWindow -PassThru

# Wait a moment for server to start
Start-Sleep -Seconds 2

# Check if process is still running
if ($guiProcess.HasExited) {
    Write-Host "Error: GUI server failed to start" -ForegroundColor Red
    exit 1
}

Write-Host "GUI server started (PID: $($guiProcess.Id))" -ForegroundColor Green
Write-Host "Opening browser to: $url" -ForegroundColor Cyan

# Open browser
Start-Process $url

Write-Host ""
Write-Host "Codex GUI is running. Press Ctrl+C to stop the server." -ForegroundColor Yellow
Write-Host "To stop: Get-Process | Where-Object { `$_.Id -eq $($guiProcess.Id) } | Stop-Process" -ForegroundColor Gray

# Wait for process to exit
try {
    $guiProcess.WaitForExit()
} catch {
    Write-Host "GUI server stopped." -ForegroundColor Yellow
}

