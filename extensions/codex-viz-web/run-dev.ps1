# Codex Viz Development Launcher (PowerShell)
# Starts both backend and frontend in parallel

$ErrorActionPreference = "Stop"

Write-Host "🚀 Starting Codex Repository Visualizer..." -ForegroundColor Cyan

# Check if backend and frontend directories exist
if (-not (Test-Path "backend")) {
    Write-Host "❌ Backend directory not found" -ForegroundColor Red
    exit 1
}

if (-not (Test-Path "frontend")) {
    Write-Host "❌ Frontend directory not found" -ForegroundColor Red
    exit 1
}

# Start backend
Write-Host "🦀 Starting Rust backend..." -ForegroundColor Yellow
$backendJob = Start-Job -ScriptBlock {
    Set-Location $using:PWD\backend
    cargo run
}

# Wait for backend to be ready
Write-Host "⏳ Waiting for backend to start..." -ForegroundColor Yellow
Start-Sleep -Seconds 3

# Start frontend
Write-Host "⚛️  Starting React frontend..." -ForegroundColor Yellow
$frontendJob = Start-Job -ScriptBlock {
    Set-Location $using:PWD\frontend
    npm run dev
}

Write-Host ""
Write-Host "✅ Services started!" -ForegroundColor Green
Write-Host "📊 Backend:  http://localhost:3001" -ForegroundColor Cyan
Write-Host "🎨 Frontend: http://localhost:3000" -ForegroundColor Cyan
Write-Host ""
Write-Host "Press Ctrl+C to stop all services" -ForegroundColor Yellow

# Cleanup function
$cleanup = {
    Write-Host ""
    Write-Host "🛑 Shutting down services..." -ForegroundColor Yellow
    Stop-Job $backendJob -ErrorAction SilentlyContinue
    Stop-Job $frontendJob -ErrorAction SilentlyContinue
    Remove-Job $backendJob -ErrorAction SilentlyContinue
    Remove-Job $frontendJob -ErrorAction SilentlyContinue
}

# Register cleanup on Ctrl+C
Register-EngineEvent PowerShell.Exiting -Action $cleanup

try {
    # Stream output from jobs
    while ($true) {
        Receive-Job $backendJob
        Receive-Job $frontendJob
        Start-Sleep -Milliseconds 100
    }
}
finally {
    & $cleanup
}

