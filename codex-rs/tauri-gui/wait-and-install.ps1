# Wait for build completion and install

Write-Host "Waiting for build to complete..." -ForegroundColor Cyan

$maxWait = 600  # 10 minutes
$waited = 0

while ($waited -lt $maxWait) {
    if (Test-Path ".\src-tauri\target\release\bundle\msi") {
        $msi = Get-ChildItem ".\src-tauri\target\release\bundle\msi\*.msi" -ErrorAction SilentlyContinue
        
        if ($msi) {
            Write-Host ""
            Write-Host "Build complete!" -ForegroundColor Green
            Write-Host "MSI: $($msi.Name)" -ForegroundColor Gray
            Write-Host "Size: $([math]::Round($msi.Length / 1MB, 2)) MB" -ForegroundColor Gray
            Write-Host ""
            
            # Install
            Write-Host "Installing..." -ForegroundColor Yellow
            $msiPath = $msi.FullName
            Start-Process -FilePath "msiexec.exe" -ArgumentList "/i", "`"$msiPath`"", "/qb", "REINSTALL=ALL", "REINSTALLMODE=vomus" -Wait
            
            Write-Host ""
            Write-Host "Installation complete!" -ForegroundColor Green
            
            # Play sound
            Add-Type -AssemblyName System.Windows.Forms
            $player = New-Object System.Media.SoundPlayer "C:\Users\downl\Desktop\SO8T\.cursor\marisa_owattaze.wav"
            $player.PlaySync()
            
            Write-Host "Owattaze!" -ForegroundColor Magenta
            
            exit 0
        }
    }
    
    Write-Host "." -NoNewline -ForegroundColor Gray
    Start-Sleep -Seconds 5
    $waited += 5
}

Write-Host ""
Write-Host "Timeout waiting for build" -ForegroundColor Red
exit 1

