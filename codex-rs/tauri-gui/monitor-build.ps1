# Build Monitor - Wait for completion and play sound

$soundPath = "C:\Users\downl\Desktop\SO8T\.cursor\marisa_owattaze.wav"
$maxWait = 1200  # 20 minutes
$waited = 0

Write-Host "Monitoring build progress..." -ForegroundColor Cyan
Write-Host "Waiting for: src-tauri\target\release\codex-tauri.exe" -ForegroundColor Gray
Write-Host ""

while ($waited -lt $maxWait) {
    # Check if exe exists
    if (Test-Path ".\src-tauri\target\release\codex-tauri.exe") {
        $exe = Get-Item ".\src-tauri\target\release\codex-tauri.exe"
        $sizeMB = [math]::Round($exe.Length / 1MB, 2)
        
        Write-Host ""
        Write-Host "=== Build Complete! ===" -ForegroundColor Green
        Write-Host "File: codex-tauri.exe" -ForegroundColor Gray
        Write-Host "Size: $sizeMB MB" -ForegroundColor Gray
        Write-Host "Time: $exe.LastWriteTime" -ForegroundColor Gray
        Write-Host ""
        
        # Play sound
        Write-Host "Playing completion sound..." -ForegroundColor Magenta
        Add-Type -AssemblyName System.Windows.Forms
        $player = New-Object System.Media.SoundPlayer $soundPath
        $player.PlaySync()
        
        Write-Host ""
        Write-Host "Owattaze!" -ForegroundColor Magenta
        Write-Host ""
        
        # Check for MSI
        if (Test-Path ".\src-tauri\target\release\bundle\msi") {
            $msi = Get-ChildItem ".\src-tauri\target\release\bundle\msi\*.msi" -ErrorAction SilentlyContinue
            if ($msi) {
                Write-Host "MSI Installer ready!" -ForegroundColor Green
                Write-Host "Location: $($msi.FullName)" -ForegroundColor Gray
                Write-Host "Size: $([math]::Round($msi.Length / 1MB, 2)) MB" -ForegroundColor Gray
                Write-Host ""
                Write-Host "To install, run:" -ForegroundColor Cyan
                Write-Host "  msiexec /i `"$($msi.FullName)`" /qb" -ForegroundColor Yellow
            }
        }
        
        exit 0
    }
    
    # Progress indicator
    if ($waited % 10 -eq 0) {
        $elapsed = $waited
        Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Still building... ($elapsed seconds elapsed)" -ForegroundColor Yellow
    }
    
    Start-Sleep -Seconds 5
    $waited += 5
}

Write-Host ""
Write-Host "Timeout after $maxWait seconds" -ForegroundColor Red
exit 1

