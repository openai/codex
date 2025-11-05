# Watch build completion and auto-install
# Monitors cargo build process and installs when complete

Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "  Codex v1.2.0 Build Monitor & Auto Install" -ForegroundColor Cyan
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host ""

$buildExe = ".\tauri-gui\src-tauri\target\release\codex-tauri-gui.exe"
$msiPath = ".\tauri-gui\src-tauri\target\release\bundle\msi"
$checkCount = 0
$maxChecks = 600  # 10 minutes (10 sec x 600)

Write-Host "Waiting for build to complete..." -ForegroundColor Yellow
Write-Host ""

while ($checkCount -lt $maxChecks) {
    $checkCount++
    
    # Check if cargo is still running
    $cargoRunning = Get-Process -Name "cargo" -ErrorAction SilentlyContinue
    
    if (-not $cargoRunning) {
        # Cargo finished, check for exe
        if (Test-Path $buildExe) {
            Write-Host ""
            Write-Host "✅ ビルド完了！" -ForegroundColor Green
            
            $exe = Get-Item $buildExe
            $exeSize = [math]::Round($exe.Length / 1MB, 2)
            Write-Host "   ファイル: codex-tauri-gui.exe" -ForegroundColor Gray
            Write-Host "   サイズ: $exeSize MB" -ForegroundColor Gray
            Write-Host "   更新日時: $($exe.LastWriteTime)" -ForegroundColor Gray
            Write-Host ""
            
            # Check for MSI
            if (Test-Path $msiPath) {
                $msi = Get-ChildItem "$msiPath\*.msi" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
                if ($msi) {
                    Write-Host "✅ MSI作成完了: $($msi.Name)" -ForegroundColor Green
                    Write-Host ""
                    
                    # Install
                    Write-Host "📦 強制インストール開始..." -ForegroundColor Cyan
                    Write-Host ""
                    
                    .\install-unified.ps1
                    
                    exit 0
                } else {
                    Write-Host "⚠️  MSIがまだ作成されていません。手動で作成してください:" -ForegroundColor Yellow
                    Write-Host "   cd tauri-gui ; npx tauri build" -ForegroundColor Gray
                }
            } else {
                Write-Host "⚠️  MSIがまだ作成されていません。手動で作成してください:" -ForegroundColor Yellow
                Write-Host "   cd tauri-gui ; npx tauri build" -ForegroundColor Gray
            }
            
            break
        } else {
            Write-Host "⚠️  Cargo完了したが、実行ファイルが見つかりません" -ForegroundColor Yellow
        }
    }
    
    # Progress indicator
    if ($checkCount % 6 -eq 0) {
        $minutes = [math]::Floor($checkCount / 6)
        $seconds = ($checkCount % 6) * 10
        Write-Host "   待機中... ($minutes分${seconds}秒経過)" -ForegroundColor Gray
    }
    
    Start-Sleep -Seconds 10
}

if ($checkCount -ge $maxChecks) {
    Write-Host ""
    Write-Host "❌ タイムアウト: ビルドが完了しませんでした" -ForegroundColor Red
    exit 1
}

