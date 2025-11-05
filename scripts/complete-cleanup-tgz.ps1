# Git履歴から大容量tgzファイルを完全削除するスクリプト
# git-filter-repoを使用

Write-Host "🔧 Git履歴から大容量ファイルを完全削除します..." -ForegroundColor Cyan

# 現在のディレクトリ確認
$repoPath = "C:\Users\downl\Desktop\codex"
Set-Location $repoPath

# git-filter-repoをインストール（pip経由）
Write-Host "`n📦 git-filter-repo をインストール中..." -ForegroundColor Yellow
py -3 -m pip install --user git-filter-repo

# バックアップブランチ作成
Write-Host "`n💾 バックアップブランチ作成..." -ForegroundColor Yellow
git branch backup-before-complete-cleanup 2>$null

# 削除対象ファイルリストを作成
$filesToRemove = @(
    "codex-cli/openai-codex-0.52.0.tgz",
    "codex-cli/zapabob-codex-0.52.0.tgz"
)

Write-Host "`n🗑️  以下のファイルを履歴から削除:" -ForegroundColor Yellow
$filesToRemove | ForEach-Object { Write-Host "  - $_" }

# 各ファイルをgit-filter-repoで削除
foreach ($file in $filesToRemove) {
    Write-Host "`n🔧 $file を削除中..." -ForegroundColor Cyan
    py -3 -m git_filter_repo --path $file --invert-paths --force
}

# または、一括削除（コメントを外して使用）
# $fileList = $filesToRemove -join "`n"
# $fileList | Out-File -FilePath "files-to-remove.txt" -Encoding UTF8
# py -3 -m git_filter_repo --paths-from-file files-to-remove.txt --invert-paths --force

# refs/originalを削除
Write-Host "`n🧹 古い参照を削除..." -ForegroundColor Yellow
if (Test-Path .git\refs\original) {
    Remove-Item -Path .git\refs\original -Recurse -Force
}

# Reflogをクリーンアップ
Write-Host "`n🧹 Reflogクリーンアップ..." -ForegroundColor Yellow
git reflog expire --expire=now --all

# ガベージコレクション
Write-Host "`n🧹 ガベージコレクション実行中..." -ForegroundColor Yellow
git gc --prune=now --aggressive

# リポジトリサイズ確認
Write-Host "`n📊 リポジトリサイズ:" -ForegroundColor Green
git count-objects -vH

Write-Host "`n✅ 完了！次のコマンドで強制プッシュしてください:" -ForegroundColor Green
Write-Host "  git push origin main --force" -ForegroundColor Cyan

Write-Host "`n⚠️  注意: すべてのブランチと履歴を書き換えました。" -ForegroundColor Yellow
Write-Host "  チーム開発の場合は、全員にgit clone し直してもらう必要があります。" -ForegroundColor Yellow

