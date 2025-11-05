# 🚀 sccache自動インストール&セットアップスクリプト
# 用途: Rustビルドキャッシュで2回目以降のビルドを70〜90%高速化
# 実行: .\install-sccache.ps1

Write-Host "🚀 sccache インストール開始..." -ForegroundColor Cyan

# 1. sccacheのインストール確認
$sccachePath = Get-Command sccache -ErrorAction SilentlyContinue

if ($null -eq $sccachePath) {
    Write-Host "📦 sccache をインストール中..." -ForegroundColor Yellow
    cargo install sccache
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "❌ インストール失敗！" -ForegroundColor Red
        exit 1
    }
    
    Write-Host "✅ sccache インストール完了！" -ForegroundColor Green
} else {
    Write-Host "✅ sccache は既にインストール済み: $($sccachePath.Source)" -ForegroundColor Green
}

# 2. 環境変数設定（現在のセッション）
$env:RUSTC_WRAPPER = "sccache"
Write-Host "✅ 環境変数設定完了（現在のセッション）" -ForegroundColor Green

# 3. PowerShellプロファイルへの追加
$profilePath = $PROFILE
$profileDir = Split-Path $profilePath -Parent

if (-not (Test-Path $profileDir)) {
    Write-Host "📁 PowerShellプロファイルディレクトリ作成中..." -ForegroundColor Yellow
    New-Item -ItemType Directory -Path $profileDir -Force | Out-Null
}

$sccacheConfig = @'

# 🚀 sccache - Rustビルドキャッシュ（Codex高速化）
$env:RUSTC_WRAPPER = "sccache"
'@

if (Test-Path $profilePath) {
    $profileContent = Get-Content $profilePath -Raw
    if ($profileContent -notmatch "RUSTC_WRAPPER") {
        Add-Content -Path $profilePath -Value $sccacheConfig
        Write-Host "✅ PowerShellプロファイルに設定追加: $profilePath" -ForegroundColor Green
    } else {
        Write-Host "⚠️  PowerShellプロファイルに既に設定あり（スキップ）" -ForegroundColor Yellow
    }
} else {
    Set-Content -Path $profilePath -Value $sccacheConfig
    Write-Host "✅ PowerShellプロファイル新規作成: $profilePath" -ForegroundColor Green
}

# 4. sccache統計表示
Write-Host "`n📊 sccache 統計情報:" -ForegroundColor Cyan
sccache --show-stats

# 5. 使用方法表示
Write-Host "`n🎯 使用方法:" -ForegroundColor Cyan
Write-Host "  1. 通常通りビルド: cargo build --release -p codex-cli" -ForegroundColor White
Write-Host "  2. キャッシュ統計: sccache --show-stats" -ForegroundColor White
Write-Host "  3. キャッシュクリア: sccache --zero-stats" -ForegroundColor White
Write-Host "`n⚡ 2回目以降のビルドが70〜90%高速化されます！" -ForegroundColor Green

# 6. 次回セッション用の注意
Write-Host "`n⚠️  次回PowerShellセッションから自動で有効化されます" -ForegroundColor Yellow
Write-Host "   今すぐ有効にするには: " -NoNewline -ForegroundColor White
Write-Host '$env:RUSTC_WRAPPER = "sccache"' -ForegroundColor Cyan

