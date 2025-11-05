# Watch build progress in real-time

Write-Host "Watching build progress..." -ForegroundColor Cyan
Write-Host "Press Ctrl+C to stop" -ForegroundColor Gray
Write-Host ""

$count = 0

while ($true) {
    Clear-Host
    Write-Host "=== Codex Tauri Build Monitor ===" -ForegroundColor Cyan
    Write-Host "Checked: $count times" -ForegroundColor Gray
    Write-Host ""
    
    # Check exe
    $exe = ".\src-tauri\target\release\codex-tauri.exe"
    if (Test-Path $exe) {
        $file = Get-Item $exe
        $sizeMB = [math]::Round($file.Length / 1MB, 2)
        
        Write-Host "BUILD COMPLETE!" -ForegroundColor Green
        Write-Host "Size: $sizeMB MB" -ForegroundColor Gray
        Write-Host "Time: $($file.LastWriteTime)" -ForegroundColor Gray
        Write-Host ""
        Write-Host "Playing sound..." -ForegroundColor Magenta
        
        Add-Type -AssemblyName System.Windows.Forms
        $player = New-Object System.Media.SoundPlayer "C:\Users\downl\Desktop\SO8T\.cursor\marisa_owattaze.wav"
        $player.PlaySync()
        
        Write-Host ""
        Write-Host "Owattaze!" -ForegroundColor Magenta
        Write-Host ""
        Write-Host "Next: .\test-security.ps1" -ForegroundColor Cyan
        break
    }
    
    # Check cargo processes
    $cargoProcs = Get-Process -Name "cargo", "rustc" -ErrorAction SilentlyContinue
    if ($cargoProcs) {
        Write-Host "Cargo/Rustc running: $($cargoProcs.Count) processes" -ForegroundColor Yellow
    } else {
        Write-Host "No cargo processes found..." -ForegroundColor Yellow
    }
    
    Write-Host ""
    Write-Host "Waiting... (checks every 5 seconds)" -ForegroundColor Gray
    
    Start-Sleep -Seconds 5
    $count++
}

