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

# GitHub PR Review 自動設定スクリプト
# Usage: powershell -ExecutionPolicy Bypass -File scripts/setup-pr-review.ps1

Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  🚀 GitHub PR Review 自動設定" -ForegroundColor Green
Write-Host "========================================`n" -ForegroundColor Cyan

# 1. 必要な情報を収集
Write-Host "📋 設定情報を入力してください:`n" -ForegroundColor Yellow

# GitHub App ID
$appId = Read-Host "GitHub App ID"
if ([string]::IsNullOrEmpty($appId)) {
    Write-Host "❌ GitHub App IDが必要です" -ForegroundColor Red
    exit 1
}

# GitHub App Private Key
Write-Host "`nGitHub App Private Key (.pem ファイルのパス):" -ForegroundColor Cyan
$privateKeyPath = Read-Host "Private Key ファイルパス"
if ([string]::IsNullOrEmpty($privateKeyPath) -or -not (Test-Path $privateKeyPath)) {
    Write-Host "❌ 有効なPrivate Keyファイルパスが必要です" -ForegroundColor Red
    exit 1
}
$privateKey = Get-Content $privateKeyPath -Raw

# OpenAI API Key
$openaiKey = Read-Host "OpenAI API Key"
if ([string]::IsNullOrEmpty($openaiKey)) {
    Write-Host "❌ OpenAI API Keyが必要です" -ForegroundColor Red
    exit 1
}

# Gemini API Key
$geminiKey = Read-Host "Gemini API Key"
if ([string]::IsNullOrEmpty($geminiKey)) {
    Write-Host "❌ Gemini API Keyが必要です" -ForegroundColor Red
    exit 1
}

# Gemini Model
Write-Host "`nGemini Model (デフォルト: gemini-2.5-flash):" -ForegroundColor Cyan
$geminiModel = Read-Host "Gemini Model"
if ([string]::IsNullOrEmpty($geminiModel)) {
    $geminiModel = "gemini-2.5-flash"
}

# Repository情報
Write-Host "`nGitHub Repository情報:" -ForegroundColor Cyan
$repoOwner = Read-Host "Repository Owner (組織名またはユーザー名)"
$repoName = Read-Host "Repository Name"

if ([string]::IsNullOrEmpty($repoOwner) -or [string]::IsNullOrEmpty($repoName)) {
    Write-Host "❌ Repository情報が必要です" -ForegroundColor Red
    exit 1
}

# 2. GitHub CLI チェック
Write-Host "`n🔍 GitHub CLI チェック中..." -ForegroundColor Cyan
$ghExists = Get-Command gh -ErrorAction SilentlyContinue
if (-not $ghExists) {
    Write-Host "❌ GitHub CLI (gh) がインストールされていません" -ForegroundColor Red
    Write-Host "インストール: https://cli.github.com/" -ForegroundColor Yellow
    exit 1
}

# GitHub CLI 認証確認
$ghAuth = gh auth status 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "❌ GitHub CLI が認証されていません" -ForegroundColor Red
    Write-Host "実行してください: gh auth login" -ForegroundColor Yellow
    exit 1
}

Write-Host "✅ GitHub CLI 認証済み" -ForegroundColor Green

# 3. Repository Secretsを設定
Write-Host "`n🔐 Repository Secrets を設定中..." -ForegroundColor Cyan

# OpenAI API Key
Write-Host "  - OPENAI_API_KEY を設定中..." -ForegroundColor White
echo $openaiKey | gh secret set OPENAI_API_KEY --repo "$repoOwner/$repoName"
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ OPENAI_API_KEY 設定完了" -ForegroundColor Green
} else {
    Write-Host "  ❌ OPENAI_API_KEY 設定失敗" -ForegroundColor Red
}

# Gemini API Key
Write-Host "  - GEMINI_API_KEY を設定中..." -ForegroundColor White
echo $geminiKey | gh secret set GEMINI_API_KEY --repo "$repoOwner/$repoName"
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ GEMINI_API_KEY 設定完了" -ForegroundColor Green
} else {
    Write-Host "  ❌ GEMINI_API_KEY 設定失敗" -ForegroundColor Red
}

# GitHub App Private Key
Write-Host "  - CODE_REVIEW_APP_PRIVATE_KEY を設定中..." -ForegroundColor White
echo $privateKey | gh secret set CODE_REVIEW_APP_PRIVATE_KEY --repo "$repoOwner/$repoName"
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ CODE_REVIEW_APP_PRIVATE_KEY 設定完了" -ForegroundColor Green
} else {
    Write-Host "  ❌ CODE_REVIEW_APP_PRIVATE_KEY 設定失敗" -ForegroundColor Red
}

# 4. Repository Variablesを設定
Write-Host "`n📊 Repository Variables を設定中..." -ForegroundColor Cyan

# GitHub App ID
Write-Host "  - CODE_REVIEW_APP_ID を設定中..." -ForegroundColor White
gh variable set CODE_REVIEW_APP_ID --body "$appId" --repo "$repoOwner/$repoName"
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ CODE_REVIEW_APP_ID 設定完了" -ForegroundColor Green
} else {
    Write-Host "  ❌ CODE_REVIEW_APP_ID 設定失敗" -ForegroundColor Red
}

# Gemini Model
Write-Host "  - AI_REVIEW_GEMINI_MODEL を設定中..." -ForegroundColor White
gh variable set AI_REVIEW_GEMINI_MODEL --body "$geminiModel" --repo "$repoOwner/$repoName"
if ($LASTEXITCODE -eq 0) {
    Write-Host "  ✅ AI_REVIEW_GEMINI_MODEL 設定完了" -ForegroundColor Green
} else {
    Write-Host "  ❌ AI_REVIEW_GEMINI_MODEL 設定失敗" -ForegroundColor Red
}

# 5. Workflow ファイル確認
Write-Host "`n📄 Workflow ファイル確認中..." -ForegroundColor Cyan
$workflowDir = ".github/workflows"
$prReviewYml = "$workflowDir/pr-review.yml"
$prReviewGeminiYml = "$workflowDir/pr-review-gemini.yml"

if (Test-Path $prReviewYml) {
    Write-Host "  ✅ pr-review.yml が存在します" -ForegroundColor Green
} else {
    Write-Host "  ❌ pr-review.yml が存在しません" -ForegroundColor Red
}

if (Test-Path $prReviewGeminiYml) {
    Write-Host "  ✅ pr-review-gemini.yml が存在します" -ForegroundColor Green
} else {
    Write-Host "  ❌ pr-review-gemini.yml が存在しません" -ForegroundColor Red
}

# 6. Git commit and push
Write-Host "`n📤 変更をコミット・プッシュしますか? (y/n):" -ForegroundColor Cyan
$commit = Read-Host
if ($commit -eq "y" -or $commit -eq "Y") {
    Write-Host "`n📝 変更をコミット中..." -ForegroundColor Cyan
    git add .github/workflows/
    git commit -m "feat: Add GitHub PR Review workflows with Codex and Gemini CLI"
    
    Write-Host "📤 変更をプッシュ中..." -ForegroundColor Cyan
    git push origin main
    
    if ($LASTEXITCODE -eq 0) {
        Write-Host "✅ 変更をプッシュしました" -ForegroundColor Green
    } else {
        Write-Host "❌ プッシュに失敗しました" -ForegroundColor Red
    }
}

# 7. 完了メッセージ
Write-Host "`n========================================" -ForegroundColor Cyan
Write-Host "  🎉 設定完了！" -ForegroundColor Green
Write-Host "========================================`n" -ForegroundColor Cyan

Write-Host "✅ 設定完了項目:" -ForegroundColor Yellow
Write-Host "  - OPENAI_API_KEY: 設定済み" -ForegroundColor Green
Write-Host "  - GEMINI_API_KEY: 設定済み" -ForegroundColor Green
Write-Host "  - CODE_REVIEW_APP_PRIVATE_KEY: 設定済み" -ForegroundColor Green
Write-Host "  - CODE_REVIEW_APP_ID: $appId" -ForegroundColor Green
Write-Host "  - AI_REVIEW_GEMINI_MODEL: $geminiModel" -ForegroundColor Green

Write-Host "`n📝 次のステップ:" -ForegroundColor Yellow
Write-Host "  1. PRを作成してテストしてください" -ForegroundColor White
Write-Host "  2. GitHub Actionsタブで実行状況を確認してください" -ForegroundColor White
Write-Host "  3. PR Reviewコメントを確認してください" -ForegroundColor White

Write-Host "`n🔗 参考リンク:" -ForegroundColor Yellow
Write-Host "  - 設定ガイド: _docs/GitHub_PR_Review_設定ガイド.md" -ForegroundColor Cyan
Write-Host "  - 実装ログ: _docs/2025-10-23_033517_GitHub_PR_Review_実装.md" -ForegroundColor Cyan

Write-Host "`n========================================`n" -ForegroundColor Cyan

# 8. 完了音声
if (Test-Path "zapabob/scripts/play-completion-sound.ps1") {
    Write-Host "🔔 完了音声を再生中..." -ForegroundColor Cyan
    powershell -ExecutionPolicy Bypass -File zapabob/scripts/play-completion-sound.ps1
}
