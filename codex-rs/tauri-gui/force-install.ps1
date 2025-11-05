# Codex Tauri - 差分ビルド＆強制インストールスクリプト
# 既存インストールを上書きして最新版を強制インストール

param(
    [switch]$SkipBuild,    # ビルドスキップ（既にビルド済みの場合）
    [switch]$Debug,        # デバッグビルド（高速）
    [switch]$Verbose       # 詳細出力
)

$ErrorActionPreference = "Stop"

Write-Host "🚀 Codex Tauri - 差分ビルド＆強制インストール" -ForegroundColor Cyan
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host ""

# カレントディレクトリ確認
if (-not (Test-Path ".\src-tauri")) {
    Write-Host "❌ エラー: src-tauriディレクトリが見つかりません" -ForegroundColor Red
    Write-Host "   codex-tauriディレクトリで実行してください" -ForegroundColor Yellow
    exit 1
}

# Step 1: 既存インストールの確認と削除
Write-Host "📦 Step 1: 既存インストール確認" -ForegroundColor Yellow
$installedApp = Get-WmiObject -Class Win32_Product | Where-Object { $_.Name -like "*Codex*" }

if ($installedApp) {
    Write-Host "   既存インストール発見: $($installedApp.Name)" -ForegroundColor Gray
    Write-Host "   アンインストール中..." -ForegroundColor Yellow
    
    try {
        $installedApp.Uninstall() | Out-Null
        Write-Host "   ✅ アンインストール完了" -ForegroundColor Green
        Start-Sleep -Seconds 2
    } catch {
        Write-Host "   ⚠️  アンインストール失敗（手動削除が必要な場合があります）" -ForegroundColor Yellow
    }
} else {
    Write-Host "   既存インストールなし（初回インストール）" -ForegroundColor Gray
}

# Step 2: 差分ビルド実行
if (-not $SkipBuild) {
    Write-Host ""
    Write-Host "🔨 Step 2: 差分ビルド実行" -ForegroundColor Yellow
    
    $buildMode = if ($Debug) { "debug" } else { "release" }
    Write-Host "   ビルドモード: $buildMode" -ForegroundColor Gray
    
    # 前回のビルド情報確認
    $targetDir = ".\src-tauri\target\$buildMode"
    if (Test-Path $targetDir) {
        $lastBuild = (Get-ChildItem $targetDir -Recurse -File | Sort-Object LastWriteTime -Descending | Select-Object -First 1).LastWriteTime
        Write-Host "   前回ビルド: $lastBuild" -ForegroundColor Gray
        Write-Host "   差分ビルドを実行（変更されたファイルのみコンパイル）" -ForegroundColor Cyan
    } else {
        Write-Host "   初回ビルド" -ForegroundColor Gray
    }
    
    Write-Host ""
    
    # Rustビルド（高速差分ビルド）
    Write-Host "   🦀 Rustコンパイル中..." -ForegroundColor Cyan
    Push-Location .\src-tauri
    
    try {
        if ($Debug) {
            # デバッグビルド（高速、最適化なし）
            cargo build --package codex-tauri 2>&1 | ForEach-Object {
                if ($_ -match "Compiling|Finished") {
                    Write-Host "      $_" -ForegroundColor Gray
                }
            }
        } else {
            # リリースビルド（最適化あり、差分利用）
            cargo build --release --package codex-tauri 2>&1 | ForEach-Object {
                if ($_ -match "Compiling|Finished") {
                    Write-Host "      $_" -ForegroundColor Gray
                }
            }
        }
        
        if ($LASTEXITCODE -ne 0) {
            throw "Cargo build failed"
        }
        
        Write-Host "   ✅ Rustビルド完了" -ForegroundColor Green
    } catch {
        Write-Host "   ❌ ビルドエラー: $_" -ForegroundColor Red
        Pop-Location
        exit 1
    }
    
    Pop-Location
    
    # MSIバンドル作成
    Write-Host ""
    Write-Host "   📦 MSIインストーラー作成中..." -ForegroundColor Cyan
    
    if ($Debug) {
        npm run tauri build -- --debug 2>&1 | ForEach-Object {
            if ($_ -match "Finished|Creating") {
                Write-Host "      $_" -ForegroundColor Gray
            }
        }
    } else {
        npm run tauri build 2>&1 | ForEach-Object {
            if ($_ -match "Finished|Creating") {
                Write-Host "      $_" -ForegroundColor Gray
            }
        }
    }
    
    if ($LASTEXITCODE -ne 0) {
        Write-Host "   ❌ MSI作成失敗" -ForegroundColor Red
        exit 1
    }
    
    Write-Host "   ✅ MSIインストーラー作成完了" -ForegroundColor Green
    
} else {
    Write-Host ""
    Write-Host "⏭️  Step 2: ビルドスキップ（--SkipBuild指定）" -ForegroundColor Yellow
}

# Step 3: MSIファイル確認
Write-Host ""
Write-Host "📄 Step 3: MSIファイル確認" -ForegroundColor Yellow

$buildMode = if ($Debug) { "debug" } else { "release" }
$msiPath = ".\src-tauri\target\$buildMode\bundle\msi"

if (-not (Test-Path $msiPath)) {
    Write-Host "   ❌ MSIディレクトリが見つかりません: $msiPath" -ForegroundColor Red
    exit 1
}

$msiFiles = Get-ChildItem $msiPath -Filter "*.msi" | Sort-Object LastWriteTime -Descending

if ($msiFiles.Count -eq 0) {
    Write-Host "   ❌ MSIファイルが見つかりません" -ForegroundColor Red
    exit 1
}

$msiFile = $msiFiles[0]
$msiFullPath = $msiFile.FullName
$msiSize = [math]::Round($msiFile.Length / 1MB, 2)

Write-Host "   MSIファイル: $($msiFile.Name)" -ForegroundColor Gray
Write-Host "   サイズ: $msiSize MB" -ForegroundColor Gray
Write-Host "   パス: $msiFullPath" -ForegroundColor Gray

# Step 4: 強制インストール
Write-Host ""
Write-Host "💾 Step 4: 強制インストール" -ForegroundColor Yellow
Write-Host "   管理者権限が必要な場合があります..." -ForegroundColor Gray
Write-Host ""

try {
    # msiexecで強制インストール
    # /i: インストール
    # /qb: 基本UI表示
    # REINSTALL=ALL: 全コンポーネント再インストール
    # REINSTALLMODE=vomus: 強制上書き
    
    Write-Host "   インストール中（進捗ウィンドウが表示されます）..." -ForegroundColor Cyan
    
    $arguments = @(
        "/i",
        "`"$msiFullPath`"",
        "/qb",           # 基本UI（進捗バーのみ）
        "REINSTALL=ALL",
        "REINSTALLMODE=vomus"
    )
    
    $process = Start-Process -FilePath "msiexec.exe" -ArgumentList $arguments -Wait -PassThru
    
    if ($process.ExitCode -eq 0) {
        Write-Host ""
        Write-Host "   ✅ インストール成功！" -ForegroundColor Green
    } elseif ($process.ExitCode -eq 1602) {
        Write-Host ""
        Write-Host "   ⚠️  インストールがキャンセルされました" -ForegroundColor Yellow
        exit 1
    } elseif ($process.ExitCode -eq 1618) {
        Write-Host ""
        Write-Host "   ⚠️  別のインストールが実行中です。完了後に再試行してください" -ForegroundColor Yellow
        exit 1
    } else {
        Write-Host ""
        Write-Host "   ❌ インストールエラー（終了コード: $($process.ExitCode)）" -ForegroundColor Red
        exit 1
    }
    
} catch {
    Write-Host ""
    Write-Host "   ❌ インストール失敗: $_" -ForegroundColor Red
    exit 1
}

# Step 5: インストール確認
Write-Host ""
Write-Host "✅ Step 5: インストール確認" -ForegroundColor Yellow

Start-Sleep -Seconds 2

$installedApp = Get-WmiObject -Class Win32_Product | Where-Object { $_.Name -like "*Codex*" }

if ($installedApp) {
    Write-Host "   インストール済み: $($installedApp.Name)" -ForegroundColor Green
    Write-Host "   バージョン: $($installedApp.Version)" -ForegroundColor Gray
    Write-Host "   インストール場所: $($installedApp.InstallLocation)" -ForegroundColor Gray
} else {
    Write-Host "   ⚠️  インストールの確認ができませんでした" -ForegroundColor Yellow
    Write-Host "   スタートメニューから起動を試してください" -ForegroundColor Gray
}

# Step 6: 起動
Write-Host ""
Write-Host "🚀 Step 6: アプリケーション起動" -ForegroundColor Yellow

$exePath = "$env:LOCALAPPDATA\Programs\Codex\Codex.exe"
if (Test-Path $exePath) {
    Write-Host "   起動中: $exePath" -ForegroundColor Cyan
    Start-Process $exePath
    Write-Host "   ✅ 起動完了" -ForegroundColor Green
} else {
    # 代替パスを探す
    $programFiles = "$env:ProgramFiles\Codex\Codex.exe"
    if (Test-Path $programFiles) {
        Write-Host "   起動中: $programFiles" -ForegroundColor Cyan
        Start-Process $programFiles
        Write-Host "   ✅ 起動完了" -ForegroundColor Green
    } else {
        Write-Host "   ⚠️  実行ファイルが見つかりません" -ForegroundColor Yellow
        Write-Host "   スタートメニューから「Codex」を検索して起動してください" -ForegroundColor Gray
    }
}

# 完了
Write-Host ""
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host "🎉 差分ビルド＆強制インストール完了！" -ForegroundColor Green
Write-Host "===============================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "次のステップ:" -ForegroundColor White
Write-Host "1. システムトレイアイコンを確認" -ForegroundColor Gray
Write-Host "2. セキュリティテスト実行: .\test-security.ps1" -ForegroundColor Gray
Write-Host "3. 詳細テスト: SECURITY_TEST.md 参照" -ForegroundColor Gray
Write-Host ""

# ログ保存
$logContent = @"
# Codex Tauri インストールログ

**日時**: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
**ビルドモード**: $buildMode
**MSIファイル**: $($msiFile.Name)
**サイズ**: $msiSize MB
**インストール**: 成功

## インストール情報

- 名前: $($installedApp.Name)
- バージョン: $($installedApp.Version)
- 場所: $($installedApp.InstallLocation)

## 次のステップ

1. セキュリティテスト実行
2. 実機動作確認
3. パフォーマンステスト

"@

$logContent | Out-File ".\install-log.txt" -Encoding UTF8
Write-Host "📄 インストールログを install-log.txt に保存しました" -ForegroundColor Cyan

