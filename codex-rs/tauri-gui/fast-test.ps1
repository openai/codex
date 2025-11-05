# Fast Test - Debug build for quick testing

Write-Host "=== Fast Debug Build ===" -ForegroundColor Cyan
Write-Host "Cargo.toml fix applied" -ForegroundColor Green
Write-Host ""

Write-Host "[1/2] Debug build (no optimization, fast)..." -ForegroundColor Yellow
cd src-tauri
cargo build 2>&1 | Select-String -Pattern "Compiling|Finished|error"

if ($LASTEXITCODE -eq 0) {
    Write-Host ""
    Write-Host "Build SUCCESS!" -ForegroundColor Green
    
    $exe = ".\target\debug\codex-tauri.exe"
    if (Test-Path $exe) {
        $size = [math]::Round((Get-Item $exe).Length / 1MB, 2)
        Write-Host "File: $exe" -ForegroundColor Gray
        Write-Host "Size: $size MB" -ForegroundColor Gray
        
        Write-Host ""
        Write-Host "[2/2] Running app..." -ForegroundColor Yellow
        Start-Process $exe
        
        Write-Host ""
        Write-Host "App launched!" -ForegroundColor Green
        Write-Host "Check system tray for Codex icon" -ForegroundColor Cyan
        
        # Play sound
        Add-Type -AssemblyName System.Windows.Forms
        $player = New-Object System.Media.SoundPlayer "C:\Users\downl\Desktop\SO8T\.cursor\marisa_owattaze.wav"
        $player.PlaySync()
        
        Write-Host ""
        Write-Host "Owattaze!" -ForegroundColor Magenta
    }
} else {
    Write-Host ""
    Write-Host "Build FAILED!" -ForegroundColor Red
    Write-Host "Check errors above" -ForegroundColor Yellow
}

cd ..


