# BFG Repo-Cleanerを使ってGit履歴から大容量ファイルを削除
# 最もシンプルで確実な方法

Write-Host "🔧 BFG Repo-Cleanerで大容量ファイルを削除します..." -ForegroundColor Cyan

$repoPath = "C:\Users\downl\Desktop\codex"
Set-Location $repoPath

# BFG Repo-Cleanerをダウンロード
$bfgUrl = "https://repo1.maven.org/maven2/com/madgag/bfg/1.14.0/bfg-1.14.0.jar"
$bfgPath = ".\bfg.jar"

if (-not (Test-Path $bfgPath)) {
    Write-Host "`n📦 BFG Repo-Cleanerをダウンロード中..." -ForegroundColor Yellow
    try {
        Invoke-WebRequest -Uri $bfgUrl -OutFile $bfgPath
        Write-Host "✅ ダウンロード完了" -ForegroundColor Green
    } catch {
        Write-Host "✗ ダウンロード失敗: $_" -ForegroundColor Red
        Write-Host "`n手動でダウンロードしてください:" -ForegroundColor Yellow
        Write-Host "  $bfgUrl" -ForegroundColor Cyan
        exit 1
    }
}

# バックアップブランチ作成
Write-Host "`n💾 バックアップブランチ作成..." -ForegroundColor Yellow
git branch backup-before-bfg 2>$null

# 削除対象ファイルのリストを作成
$filesToDelete = @"
openai-codex-0.52.0.tgz
zapabob-codex-0.52.0.tgz
"@

$filesToDelete | Out-File -FilePath "files-to-delete.txt" -Encoding UTF8

Write-Host "`n🗑️  以下のファイルを履歴から削除:" -ForegroundColor Yellow
Write-Host "  - codex-cli/openai-codex-0.52.0.tgz"
Write-Host "  - codex-cli/zapabob-codex-0.52.0.tgz"

# BFGで削除（100MB以上のファイル全削除）
Write-Host "`n🔧 BFGで100MB以上のファイルを削除中..." -ForegroundColor Cyan
java -jar bfg.jar --strip-blobs-bigger-than 100M .

# または特定ファイル名を指定
Write-Host "`n🔧 特定ファイルも削除中..." -ForegroundColor Cyan
java -jar bfg.jar --delete-files "openai-codex-0.52.0.tgz" .
java -jar bfg.jar --delete-files "zapabob-codex-0.52.0.tgz" .

# Gitクリーンアップ
Write-Host "`n🧹 Gitクリーンアップ実行中..." -ForegroundColor Yellow
git reflog expire --expire=now --all
git gc --prune=now --aggressive

# リポジトリサイズ確認
Write-Host "`n📊 リポジトリサイズ:" -ForegroundColor Green
git count-objects -vH

Write-Host "`n✅ 完了！次のコマンドで強制プッシュしてください:" -ForegroundColor Green
Write-Host "  git push origin main --force" -ForegroundColor Cyan

Write-Host "`n📝 注意事項:" -ForegroundColor Yellow
Write-Host "  - 履歴を完全に書き換えました" -ForegroundColor White
Write-Host "  - チーム開発の場合は全員に git clone し直してもらってください" -ForegroundColor White
Write-Host "  - files-to-delete.txt と bfg.jar は削除してOKです" -ForegroundColor White

# クリーンアップファイル削除確認
$response = Read-Host "`n一時ファイル(bfg.jar, files-to-delete.txt)を削除しますか? (Y/n)"
if ($response -eq "" -or $response -eq "Y" -or $response -eq "y") {
    Remove-Item -Path $bfgPath -Force -ErrorAction SilentlyContinue
    Remove-Item -Path "files-to-delete.txt" -Force -ErrorAction SilentlyContinue
    Write-Host "✅ 一時ファイル削除完了" -ForegroundColor Green
}

