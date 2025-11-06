# Blueprint → Plan データ移行スクリプト
# ユーザーの ~/.codex/blueprints/ を ~/.codex/plans/ に移行

Write-Host "🔄 Blueprint → Plan データ移行スクリプト" -ForegroundColor Cyan
Write-Host ""

$codexDir = Join-Path $env:USERPROFILE ".codex"
$blueprintsDir = Join-Path $codexDir "blueprints"
$plansDir = Join-Path $codexDir "plans"

# blueprints ディレクトリが存在するか確認
if (-not (Test-Path $blueprintsDir)) {
    Write-Host "✓ blueprintsディレクトリは存在しません。移行不要です。" -ForegroundColor Green
    exit 0
}

# plans ディレクトリが既に存在する場合
if (Test-Path $plansDir) {
    Write-Host "⚠️  plansディレクトリが既に存在します。" -ForegroundColor Yellow
    Write-Host "   既存: $plansDir"
    $response = Read-Host "上書きしますか？ (y/N)"
    if ($response -ne "y" -and $response -ne "Y") {
        Write-Host "❌ 移行をキャンセルしました。" -ForegroundColor Red
        exit 1
    }
    
    # バックアップ作成
    $backupDir = "$plansDir.backup.$(Get-Date -Format 'yyyyMMdd_HHmmss')"
    Write-Host "📦 既存plansをバックアップ: $backupDir" -ForegroundColor Yellow
    Move-Item $plansDir $backupDir
}

# ファイル数をカウント
$fileCount = (Get-ChildItem $blueprintsDir -Recurse -File).Count
Write-Host "📊 移行対象: $fileCount ファイル" -ForegroundColor Cyan
Write-Host ""

# blueprints → plans に移動
try {
    Write-Host "🚀 移行開始..." -ForegroundColor Cyan
    Move-Item $blueprintsDir $plansDir -Force
    Write-Host "✓ ディレクトリ移動完了: blueprints → plans" -ForegroundColor Green
    
    # JSONファイル内の blueprint 参照を plan に置換
    Write-Host ""
    Write-Host "🔧 JSONファイル内の参照を更新中..." -ForegroundColor Cyan
    $jsonFiles = Get-ChildItem $plansDir -Include *.json -Recurse
    $updatedCount = 0
    
    foreach ($file in $jsonFiles) {
        $content = [IO.File]::ReadAllText($file.FullName)
        $original = $content
        $content = $content -replace '"blueprint"', '"plan"'
        $content = $content -replace 'blueprint_', 'plan_'
        $content = $content -replace '/blueprints/', '/plans/'
        $content = $content -replace '\.codex/blueprints', '.codex/plans'
        
        if ($content -ne $original) {
            [IO.File]::WriteAllText($file.FullName, $content)
            $updatedCount++
        }
    }
    
    if ($updatedCount -gt 0) {
        Write-Host "✓ $updatedCount 個のJSONファイルを更新" -ForegroundColor Green
    } else {
        Write-Host "✓ JSONファイルの更新は不要でした" -ForegroundColor Green
    }
    
    Write-Host ""
    Write-Host "🎉 移行完了！" -ForegroundColor Green
    Write-Host "   移行元: $blueprintsDir (削除済み)"
    Write-Host "   移行先: $plansDir"
    Write-Host ""
    Write-Host "💡 Tip: 今後は 'codex plan' コマンドを使用してください" -ForegroundColor Cyan
    
} catch {
    Write-Host "❌ エラーが発生しました: $_" -ForegroundColor Red
    exit 1
}


