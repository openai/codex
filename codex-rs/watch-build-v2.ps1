# Watch build completion and auto-install v2
# ASCII-safe version without encoding issues

Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "  Codex v1.2.0 Build Monitor & Auto-Install" -ForegroundColor Cyan
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host ""

$buildExe = ".\tauri-gui\src-tauri\target\release\codex-tauri-gui.exe"
$msiPath = ".\tauri-gui\src-tauri\target\release\bundle\msi"
$checkCount = 0
$maxChecks = 600  # 10 minutes (10 sec x 600)

Write-Host "Waiting for build completion..." -ForegroundColor Yellow
Write-Host ""

while ($checkCount -lt $maxChecks) {
    $checkCount++
    
    # Check if cargo is still running
    $cargoRunning = Get-Process -Name "cargo" -ErrorAction SilentlyContinue
    
    if (-not $cargoRunning) {
        # Cargo finished, check for exe
        if (Test-Path $buildExe) {
            Write-Host ""
            Write-Host "Build completed!" -ForegroundColor Green
            
            $exe = Get-Item $buildExe
            $exeSize = [math]::Round($exe.Length / 1MB, 2)
            Write-Host "   File: codex-tauri-gui.exe" -ForegroundColor Gray
            Write-Host "   Size: $exeSize MB" -ForegroundColor Gray
            Write-Host "   Updated: $($exe.LastWriteTime)" -ForegroundColor Gray
            Write-Host ""
            
            # Check for MSI
            if (Test-Path $msiPath) {
                $msi = Get-ChildItem "$msiPath\*.msi" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
                if ($msi) {
                    Write-Host "MSI created: $($msi.Name)" -ForegroundColor Green
                    Write-Host ""
                    
                    # Install
                    Write-Host "Starting forced installation..." -ForegroundColor Cyan
                    Write-Host ""
                    
                    .\install-unified.ps1
                    
                    exit 0
                } else {
                    Write-Host "MSI not yet created. Create manually:" -ForegroundColor Yellow
                    Write-Host "   cd tauri-gui ; npx tauri build" -ForegroundColor Gray
                }
            } else {
                Write-Host "MSI not yet created. Create manually:" -ForegroundColor Yellow
                Write-Host "   cd tauri-gui ; npx tauri build" -ForegroundColor Gray
            }
            
            break
        } else {
            Write-Host "Cargo finished but executable not found" -ForegroundColor Yellow
        }
    }
    
    # Progress indicator
    if ($checkCount % 6 -eq 0) {
        $minutes = [math]::Floor($checkCount / 6)
        $seconds = ($checkCount % 6) * 10
        Write-Host "   Waiting... (${minutes}m ${seconds}s elapsed)" -ForegroundColor Gray
    }
    
    Start-Sleep -Seconds 10
}

if ($checkCount -ge $maxChecks) {
    Write-Host ""
    Write-Host "Timeout: Build did not complete" -ForegroundColor Red
    exit 1
}

