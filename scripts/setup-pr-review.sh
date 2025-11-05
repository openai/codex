#!/bin/bash
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

# GitHub PR Review 自動設定スクリプト (Bash版)
# Usage: bash scripts/setup-pr-review.sh

echo ""
echo "========================================"
echo "  🚀 GitHub PR Review 自動設定"
echo "========================================"
echo ""

# 1. 必要な情報を収集
echo "📋 設定情報を入力してください:"
echo ""

# GitHub App ID
read -p "GitHub App ID: " APP_ID
if [ -z "$APP_ID" ]; then
    echo "❌ GitHub App IDが必要です"
    exit 1
fi

# GitHub App Private Key
echo ""
echo "GitHub App Private Key (.pem ファイルのパス):"
read -p "Private Key ファイルパス: " PRIVATE_KEY_PATH
if [ -z "$PRIVATE_KEY_PATH" ] || [ ! -f "$PRIVATE_KEY_PATH" ]; then
    echo "❌ 有効なPrivate Keyファイルパスが必要です"
    exit 1
fi
PRIVATE_KEY=$(cat "$PRIVATE_KEY_PATH")

# OpenAI API Key
echo ""
read -p "OpenAI API Key: " OPENAI_KEY
if [ -z "$OPENAI_KEY" ]; then
    echo "❌ OpenAI API Keyが必要です"
    exit 1
fi

# Gemini API Key
echo ""
read -p "Gemini API Key: " GEMINI_KEY
if [ -z "$GEMINI_KEY" ]; then
    echo "❌ Gemini API Keyが必要です"
    exit 1
fi

# Gemini Model
echo ""
read -p "Gemini Model (デフォルト: gemini-2.5-flash): " GEMINI_MODEL
if [ -z "$GEMINI_MODEL" ]; then
    GEMINI_MODEL="gemini-2.5-flash"
fi

# Repository情報
echo ""
echo "GitHub Repository情報:"
read -p "Repository Owner (組織名またはユーザー名): " REPO_OWNER
read -p "Repository Name: " REPO_NAME

if [ -z "$REPO_OWNER" ] || [ -z "$REPO_NAME" ]; then
    echo "❌ Repository情報が必要です"
    exit 1
fi

REPO="$REPO_OWNER/$REPO_NAME"

# 2. GitHub CLI チェック
echo ""
echo "🔍 GitHub CLI チェック中..."
if ! command -v gh &> /dev/null; then
    echo "❌ GitHub CLI (gh) がインストールされていません"
    echo "インストール: https://cli.github.com/"
    exit 1
fi

# GitHub CLI 認証確認
if ! gh auth status &> /dev/null; then
    echo "❌ GitHub CLI が認証されていません"
    echo "実行してください: gh auth login"
    exit 1
fi

echo "✅ GitHub CLI 認証済み"

# 3. Repository Secretsを設定
echo ""
echo "🔐 Repository Secrets を設定中..."

# OpenAI API Key
echo "  - OPENAI_API_KEY を設定中..."
echo "$OPENAI_KEY" | gh secret set OPENAI_API_KEY --repo "$REPO"
if [ $? -eq 0 ]; then
    echo "  ✅ OPENAI_API_KEY 設定完了"
else
    echo "  ❌ OPENAI_API_KEY 設定失敗"
fi

# Gemini API Key
echo "  - GEMINI_API_KEY を設定中..."
echo "$GEMINI_KEY" | gh secret set GEMINI_API_KEY --repo "$REPO"
if [ $? -eq 0 ]; then
    echo "  ✅ GEMINI_API_KEY 設定完了"
else
    echo "  ❌ GEMINI_API_KEY 設定失敗"
fi

# GitHub App Private Key
echo "  - CODE_REVIEW_APP_PRIVATE_KEY を設定中..."
echo "$PRIVATE_KEY" | gh secret set CODE_REVIEW_APP_PRIVATE_KEY --repo "$REPO"
if [ $? -eq 0 ]; then
    echo "  ✅ CODE_REVIEW_APP_PRIVATE_KEY 設定完了"
else
    echo "  ❌ CODE_REVIEW_APP_PRIVATE_KEY 設定失敗"
fi

# 4. Repository Variablesを設定
echo ""
echo "📊 Repository Variables を設定中..."

# GitHub App ID
echo "  - CODE_REVIEW_APP_ID を設定中..."
gh variable set CODE_REVIEW_APP_ID --body "$APP_ID" --repo "$REPO"
if [ $? -eq 0 ]; then
    echo "  ✅ CODE_REVIEW_APP_ID 設定完了"
else
    echo "  ❌ CODE_REVIEW_APP_ID 設定失敗"
fi

# Gemini Model
echo "  - AI_REVIEW_GEMINI_MODEL を設定中..."
gh variable set AI_REVIEW_GEMINI_MODEL --body "$GEMINI_MODEL" --repo "$REPO"
if [ $? -eq 0 ]; then
    echo "  ✅ AI_REVIEW_GEMINI_MODEL 設定完了"
else
    echo "  ❌ AI_REVIEW_GEMINI_MODEL 設定失敗"
fi

# 5. Workflow ファイル確認
echo ""
echo "📄 Workflow ファイル確認中..."
WORKFLOW_DIR=".github/workflows"
PR_REVIEW_YML="$WORKFLOW_DIR/pr-review.yml"
PR_REVIEW_GEMINI_YML="$WORKFLOW_DIR/pr-review-gemini.yml"

if [ -f "$PR_REVIEW_YML" ]; then
    echo "  ✅ pr-review.yml が存在します"
else
    echo "  ❌ pr-review.yml が存在しません"
fi

if [ -f "$PR_REVIEW_GEMINI_YML" ]; then
    echo "  ✅ pr-review-gemini.yml が存在します"
else
    echo "  ❌ pr-review-gemini.yml が存在しません"
fi

# 6. Git commit and push
echo ""
read -p "📤 変更をコミット・プッシュしますか? (y/n): " COMMIT
if [ "$COMMIT" = "y" ] || [ "$COMMIT" = "Y" ]; then
    echo ""
    echo "📝 変更をコミット中..."
    git add .github/workflows/
    git commit -m "feat: Add GitHub PR Review workflows with Codex and Gemini CLI"
    
    echo "📤 変更をプッシュ中..."
    git push origin main
    
    if [ $? -eq 0 ]; then
        echo "✅ 変更をプッシュしました"
    else
        echo "❌ プッシュに失敗しました"
    fi
fi

# 7. 完了メッセージ
echo ""
echo "========================================"
echo "  🎉 設定完了！"
echo "========================================"
echo ""

echo "✅ 設定完了項目:"
echo "  - OPENAI_API_KEY: 設定済み"
echo "  - GEMINI_API_KEY: 設定済み"
echo "  - CODE_REVIEW_APP_PRIVATE_KEY: 設定済み"
echo "  - CODE_REVIEW_APP_ID: $APP_ID"
echo "  - AI_REVIEW_GEMINI_MODEL: $GEMINI_MODEL"

echo ""
echo "📝 次のステップ:"
echo "  1. PRを作成してテストしてください"
echo "  2. GitHub Actionsタブで実行状況を確認してください"
echo "  3. PR Reviewコメントを確認してください"

echo ""
echo "🔗 参考リンク:"
echo "  - 設定ガイド: _docs/GitHub_PR_Review_設定ガイド.md"
echo "  - 実装ログ: _docs/2025-10-23_033517_GitHub_PR_Review_実装.md"

echo ""
echo "========================================"
echo ""
