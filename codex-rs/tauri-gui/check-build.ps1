# Check build status

$exe = ".\src-tauri\target\release\codex-tauri.exe"

if (Test-Path $exe) {
    $file = Get-Item $exe
    $sizeMB = [math]::Round($file.Length / 1MB, 2)
    $age = (Get-Date) - $file.LastWriteTime
    
    Write-Host "BUILD COMPLETE!" -ForegroundColor Green
    Write-Host "File: codex-tauri.exe" -ForegroundColor Gray
    Write-Host "Size: $sizeMB MB" -ForegroundColor Gray
    Write-Host "Built: $($file.LastWriteTime)" -ForegroundColor Gray
    Write-Host "Age: $([math]::Round($age.TotalSeconds, 0)) seconds ago" -ForegroundColor Gray
    Write-Host ""
    Write-Host "Run: .\src-tauri\target\release\codex-tauri.exe" -ForegroundColor Cyan
    Write-Host "Or: .\test-security.ps1" -ForegroundColor Cyan
} else {
    Write-Host "Still building..." -ForegroundColor Yellow
    Write-Host "Wait for completion sound!" -ForegroundColor Gray
}


