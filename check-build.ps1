# ビルド状況確認スクリプト

Write-Host "===================================" -ForegroundColor Cyan
Write-Host " Phase 4 ビルド状況確認" -ForegroundColor Cyan
Write-Host "===================================" -ForegroundColor Cyan
Write-Host ""

# 1. Cargo プロセス確認
Write-Host "[1] Cargo プロセス確認..." -ForegroundColor Yellow
$cargoProcesses = Get-Process cargo -ErrorAction SilentlyContinue
if ($cargoProcesses) {
    Write-Host "  実行中: $($cargoProcesses.Count)個のCargoプロセス" -ForegroundColor Green
    $cargoProcesses | Select-Object Id, ProcessName, StartTime, @{Name='CPU(s)';Expression={[math]::Round($_.CPU,2)}} | Format-Table
} else {
    Write-Host "  Cargoプロセスなし（ビルド完了 or 未開始）" -ForegroundColor Gray
}
Write-Host ""

# 2. ビルド成果物確認
Write-Host "[2] ビルド成果物確認..." -ForegroundColor Yellow
$codexBinary = "codex-rs\target\release\codex.exe"
if (Test-Path $codexBinary) {
    Write-Host "  ビルド完了！" -ForegroundColor Green
    $fileInfo = Get-Item $codexBinary
    Write-Host "  ファイル: $($fileInfo.Name)" -ForegroundColor White
    Write-Host "  サイズ: $([math]::Round($fileInfo.Length / 1MB, 2)) MB" -ForegroundColor White
    Write-Host "  更新日時: $($fileInfo.LastWriteTime)" -ForegroundColor White
    Write-Host ""
    Write-Host "  次のステップ:" -ForegroundColor Cyan
    Write-Host "    .\install-phase4.ps1" -ForegroundColor White
} else {
    Write-Host "  ビルド中... または未開始" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  ビルド開始コマンド:" -ForegroundColor Cyan
    Write-Host "    cd codex-rs" -ForegroundColor White
    Write-Host "    cargo build --release -p codex-cli" -ForegroundColor White
}
Write-Host ""

# 3. ビルドログ確認
Write-Host "[3] ビルドログ確認..." -ForegroundColor Yellow
$buildLog = "codex-rs\build.log"
if (Test-Path $buildLog) {
    Write-Host "  ビルドログ発見: $buildLog" -ForegroundColor Green
    $logSize = (Get-Item $buildLog).Length
    Write-Host "  ログサイズ: $([math]::Round($logSize / 1KB, 2)) KB" -ForegroundColor White
    Write-Host ""
    Write-Host "  最新のエラー/警告を確認:" -ForegroundColor Cyan
    Write-Host "    Get-Content codex-rs\build.log | Select-String -Pattern 'error|warning' | Select-Object -Last 10" -ForegroundColor White
} else {
    Write-Host "  ビルドログなし" -ForegroundColor Gray
}
Write-Host ""

Write-Host "===================================" -ForegroundColor Cyan
Write-Host " 5-10分後に再度実行してください" -ForegroundColor Cyan
Write-Host "===================================" -ForegroundColor Cyan

