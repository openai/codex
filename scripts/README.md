# GitHub PR Review 自動設定スクリプト

## 📊 概要

GitHub PR ReviewをCodex CLIとGemini CLIで自動化するための設定を自動で行うスクリプトです。

## 🚀 使用方法

### Windows (PowerShell)

```powershell
powershell -ExecutionPolicy Bypass -File scripts/setup-pr-review.ps1
```

### macOS / Linux (Bash)

```bash
chmod +x scripts/setup-pr-review.sh
bash scripts/setup-pr-review.sh
```

## 📋 事前準備

### 1. GitHub CLI インストール

#### Windows
```powershell
winget install --id GitHub.cli
```

#### macOS
```bash
brew install gh
```

#### Linux
```bash
curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg | sudo dd of=/usr/share/keyrings/githubcli-archive-keyring.gpg
echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | sudo tee /etc/apt/sources.list.d/github-cli.list > /dev/null
sudo apt update
sudo apt install gh
```

### 2. GitHub CLI 認証

```bash
gh auth login
```

### 3. GitHub App 作成

1. GitHub Organization Settings > Developer settings > GitHub Apps
2. "New GitHub App" をクリック
3. 以下の設定:

```
GitHub App name: Codex PR Reviewer
Homepage URL: https://github.com/your-org/your-repo
Webhook URL: (空でOK)

Permissions:
- Repository permissions:
  - Contents: Read
  - Pull requests: Write
  - Metadata: Read

Subscribe to events:
- Pull request
```

4. App ID を確認
5. "Generate a private key" をクリックして `.pem` ファイルをダウンロード

### 4. API Keys 取得

#### OpenAI API Key
1. [OpenAI Platform](https://platform.openai.com/api-keys) でAPI Key作成
2. API Keyをコピー

#### Google AI Studio API Key
1. [Google AI Studio](https://aistudio.google.com/app/apikey) でAPI Key作成
2. API Keyをコピー

## 📝 スクリプト実行

スクリプトを実行すると、以下の情報を入力するよう求められます：

1. **GitHub App ID**: GitHub Appの設定ページで確認したApp ID
2. **GitHub App Private Key**: ダウンロードした `.pem` ファイルのパス
3. **OpenAI API Key**: OpenAI PlatformのAPI Key
4. **Gemini API Key**: Google AI StudioのAPI Key
5. **Gemini Model**: 使用するGeminiモデル（デフォルト: `gemini-2.5-flash`）
6. **Repository Owner**: GitHubの組織名またはユーザー名
7. **Repository Name**: リポジトリ名

## ✅ 自動設定内容

スクリプトは以下の設定を自動で行います：

### Repository Secrets
- `OPENAI_API_KEY`: OpenAI API Key
- `GEMINI_API_KEY`: Gemini API Key
- `CODE_REVIEW_APP_PRIVATE_KEY`: GitHub App Private Key

### Repository Variables
- `CODE_REVIEW_APP_ID`: GitHub App ID
- `AI_REVIEW_GEMINI_MODEL`: 使用するGeminiモデル

### Workflow ファイル確認
- `.github/workflows/pr-review.yml` の存在確認
- `.github/workflows/pr-review-gemini.yml` の存在確認

### Git操作
- 変更のコミット（オプション）
- メインブランチへのプッシュ（オプション）

## 🎯 実行後の確認

### 1. GitHub Repository Settings 確認

```
Settings > Secrets and variables > Actions
```

以下が設定されていることを確認：
- Secrets: `OPENAI_API_KEY`, `GEMINI_API_KEY`, `CODE_REVIEW_APP_PRIVATE_KEY`
- Variables: `CODE_REVIEW_APP_ID`, `AI_REVIEW_GEMINI_MODEL`

### 2. GitHub Actions 確認

```
Actions タブ
```

ワークフローが表示されていることを確認：
- PR Review with Codex
- PR Review with Gemini CLI

### 3. テストPR作成

1. テスト用のブランチを作成
2. 小さな変更を加える
3. PRを作成
4. GitHub Actionsが自動実行されることを確認
5. PRにレビューコメントが投稿されることを確認

## 🔧 トラブルシューティング

### GitHub CLI 認証エラー

```bash
gh auth status
gh auth login
```

### Secrets設定エラー

```bash
# 手動設定
gh secret set OPENAI_API_KEY --repo owner/repo
gh secret set GEMINI_API_KEY --repo owner/repo
gh secret set CODE_REVIEW_APP_PRIVATE_KEY --repo owner/repo
```

### Variables設定エラー

```bash
# 手動設定
gh variable set CODE_REVIEW_APP_ID --body "12345" --repo owner/repo
gh variable set AI_REVIEW_GEMINI_MODEL --body "gemini-2.5-flash" --repo owner/repo
```

### Workflow実行エラー

1. GitHub Actions タブでエラーログを確認
2. Secrets/Variablesが正しく設定されているか確認
3. GitHub Appの権限を確認
4. API Keyの有効性を確認

## 📚 参考ドキュメント

- [設定ガイド](../_docs/GitHub_PR_Review_設定ガイド.md)
- [実装ログ](../_docs/2025-10-23_033517_GitHub_PR_Review_実装.md)
- [羅針盤技術ブログ - Gemini CLI](https://compasscorp.hatenablog.com/entry/github-pr-review-gemini-cli)
- [羅針盤技術ブログ - Codex CLI](https://compasscorp.hatenablog.com/entry/github-pr-review-codex-cli)

## 🎉 完了

設定が完了したら、PRを作成してテストしてください！

---

**作成者**: zapabob  
**バージョン**: 1.0.0  
**最終更新**: 2025-10-23