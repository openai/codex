# Codex エマージェンシー修復スクリプト
# 
# ビルド・インストールで問題が発生した際の緊急修復用
# なんJ風に言うと: トラブった時の救世主や！🚑

$ErrorActionPreference = "Continue"

Write-Host "========================================" -ForegroundColor Red
Write-Host " Codex Emergency Repair Script" -ForegroundColor Red
Write-Host "========================================" -ForegroundColor Red
Write-Host ""

# Step 0: Auto-detect codex-rs directory
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$CurrentDir = Get-Location

if (-not (Test-Path "Cargo.toml")) {
    Write-Host "[*] Auto-detecting codex-rs directory..." -ForegroundColor Cyan
    
    $PossiblePaths = @(
        $ScriptDir,
        (Join-Path $CurrentDir "codex-rs"),
        (Join-Path (Split-Path $CurrentDir -Parent) "codex-rs"),
        "C:\Users\downl\Desktop\codex-main\codex-main\codex-rs"
    )
    
    $Found = $false
    foreach ($Path in $PossiblePaths) {
        if (Test-Path (Join-Path $Path "Cargo.toml")) {
            Write-Host "[*] Found codex-rs at: $Path" -ForegroundColor Cyan
            Set-Location $Path
            $Found = $true
            break
        }
    }
    
    if (-not $Found) {
        Write-Host "[ERROR] Could not find codex-rs directory" -ForegroundColor Red
        Write-Host "Please run from codex-rs directory" -ForegroundColor Yellow
        exit 1
    }
}

Write-Host "[OK] Working directory: $(Get-Location)" -ForegroundColor Green
Write-Host ""

# Problem diagnosis
Write-Host "[*] Diagnosing issues..." -ForegroundColor Cyan

# 1. codex プロセスの確認
Write-Host "`n[1/6] 実行中の codex プロセスを確認..." -ForegroundColor Yellow
$CodexProcesses = Get-Process codex -ErrorAction SilentlyContinue
if ($CodexProcesses) {
    Write-Host "   ⚠️  実行中のプロセスを検出: $($CodexProcesses.Count) 個" -ForegroundColor Yellow
    Write-Host "   🔧 プロセスを強制停止中..." -ForegroundColor Cyan
    $CodexProcesses | Stop-Process -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 3
    Write-Host "   ✅ プロセス停止完了" -ForegroundColor Green
} else {
    Write-Host "   ✅ 実行中のプロセスなし" -ForegroundColor Green
}

# 2. ロックファイルの削除
Write-Host "`n[2/6] Cargo ロックファイルを確認..." -ForegroundColor Yellow
if (Test-Path "Cargo.lock") {
    Write-Host "   🔧 Cargo.lock をクリーン..." -ForegroundColor Cyan
    Remove-Item "Cargo.lock" -Force -ErrorAction SilentlyContinue
    Write-Host "   ✅ 削除完了" -ForegroundColor Green
}

# 3. target ディレクトリのクリーンアップ
Write-Host "`n[3/6] ビルドキャッシュをクリーン..." -ForegroundColor Yellow
if (Test-Path "target") {
    $TargetSize = (Get-ChildItem "target" -Recurse | Measure-Object -Property Length -Sum).Sum / 1GB
    Write-Host "   📊 現在のサイズ: $([math]::Round($TargetSize, 2)) GB" -ForegroundColor Gray
    
    Write-Host "   🔧 cargo clean 実行中..." -ForegroundColor Cyan
    cargo clean 2>&1 | Out-Null
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "   ✅ クリーン完了" -ForegroundColor Green
    } else {
        Write-Host "   ⚠️  cargo clean でエラー。target ディレクトリを直接削除します" -ForegroundColor Yellow
        Remove-Item "target" -Recurse -Force -ErrorAction SilentlyContinue
        Write-Host "   ✅ 強制削除完了" -ForegroundColor Green
    }
}

# 4. 古いバイナリのクリーンアップ
Write-Host "`n[4/6] 古いバイナリをクリーン..." -ForegroundColor Yellow
$InstallPath = "$env:USERPROFILE\.cargo\bin"
$OldBackups = Get-ChildItem "$InstallPath\codex.exe.backup-*" -ErrorAction SilentlyContinue

if ($OldBackups) {
    Write-Host "   🔧 古いバックアップを削除: $($OldBackups.Count) 個" -ForegroundColor Cyan
    $OldBackups | Remove-Item -Force -ErrorAction SilentlyContinue
    Write-Host "   ✅ バックアップクリーン完了" -ForegroundColor Green
}

# 5. リリースビルド
Write-Host "`n[5/6] リリースビルド実行中..." -ForegroundColor Yellow
Write-Host "   ⏳ これには5～15分かかる場合があります..." -ForegroundColor Gray

$BuildStart = Get-Date
$BuildOutput = cargo build --release -p codex-cli 2>&1 | Out-String
$BuildDuration = (Get-Date) - $BuildStart

if ($LASTEXITCODE -eq 0) {
    Write-Host "   ✅ ビルド成功！（所要時間: $([math]::Round($BuildDuration.TotalMinutes, 1)) 分）" -ForegroundColor Green
} else {
    Write-Host "   ❌ ビルド失敗" -ForegroundColor Red
    
    # エラー分析
    if ($BuildOutput -match "ring") {
        Write-Host "`n   🔧 ring クレートのエラーを検出" -ForegroundColor Yellow
        Write-Host "   対策 1: Visual Studio Build Tools を確認" -ForegroundColor Cyan
        Write-Host "   対策 2: 以下のコマンドで依存関係を更新:" -ForegroundColor Cyan
        Write-Host "      cargo update -p ring" -ForegroundColor White
        
        # 自動修復試行
        Write-Host "`n   🔧 自動修復を試行中..." -ForegroundColor Cyan
        cargo update -p ring 2>&1 | Out-Null
        
        Write-Host "   🔧 再ビルド中..." -ForegroundColor Cyan
        $BuildOutput = cargo build --release -p codex-cli 2>&1 | Out-String
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "   ✅ 修復成功！ビルド完了" -ForegroundColor Green
        } else {
            Write-Host "`n   ❌ 修復失敗。ビルドログ:" -ForegroundColor Red
            Write-Host $BuildOutput | Select-String "error" | Select-Object -First 10
            exit 1
        }
    } elseif ($BuildOutput -match "could not compile") {
        Write-Host "`n   🔧 コンパイルエラーを検出" -ForegroundColor Yellow
        Write-Host $BuildOutput | Select-String "error\[" | Select-Object -First 10
        exit 1
    } else {
        Write-Host "`n   ❌ 不明なビルドエラー" -ForegroundColor Red
        Write-Host $BuildOutput | Select-String "error|warning" | Select-Object -First 10
        exit 1
    }
}

# 6. グローバルインストール
Write-Host "`n[6/6] グローバルインストール中..." -ForegroundColor Yellow

$SourceBinary = ".\target\release\codex.exe"
$DestBinary = "$env:USERPROFILE\.cargo\bin\codex.exe"

if (-not (Test-Path $SourceBinary)) {
    Write-Error-Custom "ビルドされたバイナリが見つかりません: $SourceBinary"
    exit 1
}

# バックアップ
if (Test-Path $DestBinary) {
    $BackupPath = "$DestBinary.backup-$(Get-Date -Format 'yyyyMMdd-HHmmss')"
    Write-Host "   💾 既存バイナリをバックアップ..." -ForegroundColor Cyan
    Copy-Item $DestBinary $BackupPath -Force -ErrorAction SilentlyContinue
}

# インストール実行（リトライ機能付き）
$MaxRetries = 3
$RetryCount = 0
$InstallSuccess = $false

while ($RetryCount -lt $MaxRetries -and -not $InstallSuccess) {
    try {
        if ($RetryCount -gt 0) {
            Write-Host "   🔄 リトライ $RetryCount/$MaxRetries ..." -ForegroundColor Yellow
            # プロセス停止
            Get-Process codex -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
            Start-Sleep -Seconds 3
        }
        
        Copy-Item $SourceBinary $DestBinary -Force
        $InstallSuccess = $true
        Write-Host "   ✅ インストール成功！" -ForegroundColor Green
        Log "Installed to $DestBinary"
    } catch {
        $RetryCount++
        if ($RetryCount -lt $MaxRetries) {
            Write-Host "   ⚠️  インストール失敗。リトライします..." -ForegroundColor Yellow
            Start-Sleep -Seconds 2
        } else {
            Write-Error-Custom "インストールに失敗しました: $_"
            Write-Host "`n   🔧 手動修復手順:" -ForegroundColor Yellow
            Write-Host "   1. タスクマネージャーで codex.exe を完全停止" -ForegroundColor White
            Write-Host "   2. 以下を実行:" -ForegroundColor White
            Write-Host "      Remove-Item $DestBinary -Force" -ForegroundColor Cyan
            Write-Host "      Copy-Item $SourceBinary $DestBinary -Force" -ForegroundColor Cyan
            Log "Installation failed after $MaxRetries retries: $_"
            exit 1
        }
    }
}

# 7. 動作確認
Write-Host "`n📋 動作確認中..." -ForegroundColor Cyan
Start-Sleep -Seconds 1

$VersionOutput = & codex --version 2>&1
if ($LASTEXITCODE -eq 0) {
    Write-Host "   ✅ 動作確認完了" -ForegroundColor Green
    Write-Host "   📌 インストールバージョン: $VersionOutput" -ForegroundColor Green
    Log "Version check passed: $VersionOutput"
} else {
    Write-Host "   ❌ 動作確認失敗" -ForegroundColor Red
    Write-Host "   エラー: $VersionOutput" -ForegroundColor Red
    Log "Version check failed: $VersionOutput"
    exit 1
}

# 最終サマリー
Write-Host ""
Write-Host "╔══════════════════════════════════════════════════════════╗" -ForegroundColor Green
Write-Host "║              ✅ 修復 & インストール完了！                ║" -ForegroundColor Green
Write-Host "╚══════════════════════════════════════════════════════════╝" -ForegroundColor Green
Write-Host ""
Write-Host "📦 インストール先: $DestBinary" -ForegroundColor Cyan
Write-Host "📋 ログファイル: $LogFile" -ForegroundColor Cyan
Write-Host "⏱️  ビルド時間: $([math]::Round($BuildDuration.TotalMinutes, 1)) 分" -ForegroundColor Cyan

# バックアップ一覧
$AllBackups = Get-ChildItem "$InstallPath\codex.exe.backup-*" -ErrorAction SilentlyContinue
if ($AllBackups) {
    Write-Host "`n💾 利用可能なバックアップ: $($AllBackups.Count) 個" -ForegroundColor Gray
    $AllBackups | Select-Object -First 3 | ForEach-Object {
        Write-Host "   - $($_.Name) ($([math]::Round($_.Length / 1MB, 2)) MB)" -ForegroundColor Gray
    }
}

Write-Host "`n🚀 使用例:" -ForegroundColor Yellow
Write-Host "   codex delegate code-reviewer --scope codex-rs\cli" -ForegroundColor White
Write-Host "   codex research 'Rust async patterns' --depth 3" -ForegroundColor White
Write-Host ""
Write-Host "なんJ風に言うと: エラー修復してインストール完璧や！🔥🚀💪" -ForegroundColor Magenta

# ヘルスチェック
Write-Host "`n🏥 ヘルスチェック..." -ForegroundColor Cyan
Write-Host "   [1] バイナリサイズ: $([math]::Round((Get-Item $DestBinary).Length / 1MB, 2)) MB" -ForegroundColor White
Write-Host "   [2] 更新日時: $((Get-Item $DestBinary).LastWriteTime)" -ForegroundColor White
Write-Host "   [3] バージョン: $VersionOutput" -ForegroundColor White

# サブエージェント一覧確認
if (Test-Path ".\.codex\agents") {
    $Agents = Get-ChildItem ".\.codex\agents\*.yaml" -ErrorAction SilentlyContinue
    if ($Agents) {
        Write-Host "   [4] サブエージェント: $($Agents.Count) 個利用可能" -ForegroundColor White
        $Agents | ForEach-Object {
            $AgentName = $_.BaseName
            Write-Host "       - $AgentName" -ForegroundColor Gray
        }
    }
}

Write-Host "`n✅ 全システム正常！" -ForegroundColor Green

