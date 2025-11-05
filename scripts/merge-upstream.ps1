# Copyright 2025 zapabob
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

# Upstream同期スクリプト
# Usage: powershell -ExecutionPolicy Bypass -File scripts/merge-upstream.ps1

Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  🔄 Upstream同期スクリプト" -ForegroundColor Green
Write-Host "========================================`n" -ForegroundColor Cyan

# 1. 現在のブランチを確認
$currentBranch = git branch --show-current
Write-Host "📍 現在のブランチ: $currentBranch`n" -ForegroundColor Yellow

if ($currentBranch -ne "upstream-sync-2025-10-23") {
    Write-Host "❌ upstream-sync-2025-10-23 ブランチに切り替えてください" -ForegroundColor Red
    exit 1
}

# 2. zapabob独自ファイルを一時保存
Write-Host "💾 zapabob独自ファイルをバックアップ中...`n" -ForegroundColor Cyan

$backupDir = ".backup-zapabob-$(Get-Date -Format 'yyyyMMdd-HHmmss')"
New-Item -ItemType Directory -Path $backupDir -Force | Out-Null

# 独自ファイルリストを読み込み
if (Test-Path ".zapabob-files") {
    Get-Content ".zapabob-files" | ForEach-Object {
        $line = $_.Trim()
        if ($line -and -not $line.StartsWith("#")) {
            if (Test-Path $line) {
                Write-Host "  ✅ バックアップ: $line" -ForegroundColor Green
                $destPath = Join-Path $backupDir $line
                $destDir = Split-Path $destPath -Parent
                if (-not (Test-Path $destDir)) {
                    New-Item -ItemType Directory -Path $destDir -Force | Out-Null
                }
                Copy-Item -Path $line -Destination $destPath -Recurse -Force
            }
        }
    }
}

Write-Host "`n✅ バックアップ完了: $backupDir`n" -ForegroundColor Green

# 3. upstream/main をマージ
Write-Host "🔄 upstream/main をマージ中...`n" -ForegroundColor Cyan

# "ours" 戦略で独自変更を優先（競合時）
git merge upstream/main -X ours --no-edit

if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ マージ中にエラーが発生しました" -ForegroundColor Red
    Write-Host "手動で競合を解決してください`n" -ForegroundColor Yellow
    exit 1
}

Write-Host "✅ マージ完了`n" -ForegroundColor Green

# 4. zapabob独自ファイルを復元
Write-Host "📥 zapabob独自ファイルを復元中...`n" -ForegroundColor Cyan

if (Test-Path "$backupDir/.zapabob-files") {
    Get-Content "$backupDir/.zapabob-files" | ForEach-Object {
        $line = $_.Trim()
        if ($line -and -not $line.StartsWith("#")) {
            $sourcePath = Join-Path $backupDir $line
            if (Test-Path $sourcePath) {
                Write-Host "  ✅ 復元: $line" -ForegroundColor Green
                $destDir = Split-Path $line -Parent
                if ($destDir -and -not (Test-Path $destDir)) {
                    New-Item -ItemType Directory -Path $destDir -Force | Out-Null
                }
                Copy-Item -Path $sourcePath -Destination $line -Recurse -Force
            }
        }
    }
}

Write-Host "`n✅ 復元完了`n" -ForegroundColor Green

# 5. zapabob独自ファイルをステージング
Write-Host "📝 zapabob独自ファイルをステージング中...`n" -ForegroundColor Cyan

git add .zapabob-files
git add zapabob/
git add _docs/
git add CHANGELOG.md
git add CONTRIBUTING.md
git add scripts/setup-pr-review.*
git add scripts/README.md
git add .github/workflows/pr-review*.yml
git add config.toml
git add README.md
git add LICENSE

Write-Host "✅ ステージング完了`n" -ForegroundColor Green

# 6. 統合コミット作成
Write-Host "💾 統合コミット作成中...`n" -ForegroundColor Cyan

git commit --amend --no-edit

Write-Host "✅ コミット完了`n" -ForegroundColor Green

# 7. 確認
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  ✅ Upstream同期完了！" -ForegroundColor Green
Write-Host "========================================`n" -ForegroundColor Cyan

Write-Host "📊 次のステップ:`n" -ForegroundColor Yellow
Write-Host "  1. 変更内容を確認: git log --oneline -10" -ForegroundColor White
Write-Host "  2. テスト実行: cargo test" -ForegroundColor White
Write-Host "  3. mainブランチに切り替え: git checkout main" -ForegroundColor White
Write-Host "  4. upstream-syncをマージ: git merge upstream-sync-2025-10-23" -ForegroundColor White
Write-Host "  5. リモートにプッシュ: git push origin main`n" -ForegroundColor White

# 8. バックアップディレクトリ削除確認
Write-Host "🗑️  バックアップディレクトリを削除しますか? (y/n):" -ForegroundColor Cyan
$delete = Read-Host
if ($delete -eq "y" -or $delete -eq "Y") {
    Remove-Item -Path $backupDir -Recurse -Force
    Write-Host "✅ バックアップディレクトリを削除しました`n" -ForegroundColor Green
} else {
    Write-Host "📁 バックアップは保持されます: $backupDir`n" -ForegroundColor Yellow
}

Write-Host "========================================`n" -ForegroundColor Cyan
