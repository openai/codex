# GitHub PR Review 設定ガイド

## 📊 実装概要

**日時**: 2025-10-23  
**タスク**: GitHubのPRをコードレビューできるように設定  
**参考記事**: [羅針盤技術ブログ](https://compasscorp.hatenablog.com/entry/github-pr-review-gemini-cli)

## 🛠️ 実装内容

### 1. GitHub Actions ワークフロー作成

#### 主要ファイル
- `.github/workflows/pr-review.yml` - Codex CLI + Gemini CLI フォールバック
- `.github/workflows/pr-review-gemini.yml` - Gemini CLI 専用

#### 機能
- **自動PRレビュー**: PR作成・更新時に自動実行
- **マルチモデル対応**: Codex CLI + Gemini CLI
- **セキュリティレビュー**: 専用セキュリティチェック
- **フォールバック機能**: Codex失敗時にGemini CLIに切り替え

### 2. 必要な設定

#### GitHub Repository Settings

##### Variables (Repository Settings > Secrets and variables > Actions > Variables)
```
CODE_REVIEW_APP_ID: GitHub AppのApp ID
AI_REVIEW_GEMINI_MODEL: gemini-2.5-flash または gemini-2.5-pro
```

##### Secrets (Repository Settings > Secrets and variables > Actions > Secrets)
```
CODE_REVIEW_APP_PRIVATE_KEY: GitHub AppのPrivate Key
OPENAI_API_KEY: OpenAI API Key
GEMINI_API_KEY: Google AI Studio API Key
```

### 3. GitHub App設定

#### 1. GitHub App作成
1. GitHub Organization Settings > Developer settings > GitHub Apps
2. "New GitHub App" をクリック
3. 以下の設定:

```
GitHub App name: Codex PR Reviewer
Homepage URL: https://github.com/your-org/your-repo
Webhook URL: (空でOK)
Webhook secret: (空でOK)

Permissions:
- Repository permissions:
  - Contents: Read
  - Pull requests: Write
  - Metadata: Read

- Subscribe to events:
  - Pull request
```

#### 2. App ID取得
- App作成後、General settings で App ID を確認
- `CODE_REVIEW_APP_ID` 変数に設定

#### 3. Private Key生成
- "Generate a private key" をクリック
- ダウンロードした `.pem` ファイルの内容を `CODE_REVIEW_APP_PRIVATE_KEY` シークレットに設定

### 4. API Key設定

#### OpenAI API Key
1. [OpenAI Platform](https://platform.openai.com/api-keys) でAPI Key作成
2. `OPENAI_API_KEY` シークレットに設定

#### Google AI Studio API Key
1. [Google AI Studio](https://aistudio.google.com/app/apikey) でAPI Key作成
2. `GEMINI_API_KEY` シークレットに設定

## 🚀 使用方法

### 1. ワークフロー有効化
- リポジトリに `.github/workflows/` ファイルを配置
- デフォルトで有効化される

### 2. PR作成時の自動実行
- PRを作成・更新すると自動でコードレビューが実行される
- レビュー結果はPRのコメントとして投稿される

### 3. 手動実行
- Actions タブから手動実行も可能

## 📊 ワークフロー詳細

### Codex CLI + Gemini CLI フォールバック
```yaml
name: PR Review with Codex
on:
  pull_request:
    types: [opened, synchronize, reopened]

jobs:
  codex_review:
    # Codex CLI でレビュー実行
    # 失敗時は Gemini CLI にフォールバック
  
  security_review:
    # セキュリティ専用レビュー
```

### Gemini CLI 専用
```yaml
name: PR Review with Gemini CLI
on:
  pull_request:
    types: [opened, synchronize, reopened]

jobs:
  gemini_review:
    # Gemini CLI でレビュー実行
```

## 🎯 レビュー内容

### コード品質
- コード構造と組織化
- ベストプラクティスの遵守
- パフォーマンス考慮
- 保守性と可読性

### セキュリティ
- 潜在的なセキュリティ脆弱性
- 入力検証とサニタイゼーション
- 認証と認可
- データ保護対策

### 技術分析
- アルゴリズム効率
- エラーハンドリング
- テストカバレッジ
- ドキュメント完全性

## 🔧 カスタマイズ

### レビューガイドライン変更
- ワークフロー内の `REVIEW_GUIDELINES` を編集
- プロジェクト固有の要件に合わせて調整

### モデル変更
- `AI_REVIEW_GEMINI_MODEL` 変数でGeminiモデル指定
- `--model` パラメータでCodexモデル指定

### タイムアウト調整
- `timeout-minutes` で実行時間制限調整

## 📈 期待効果

### 開発効率向上
- 自動コードレビューによる品質向上
- 人間のレビュアーの負荷軽減
- 一貫したレビュー基準

### セキュリティ強化
- 自動セキュリティチェック
- 脆弱性の早期発見
- セキュリティベストプラクティスの強制

### コード品質向上
- ベストプラクティスの自動チェック
- パフォーマンス問題の早期発見
- ドキュメント不足の指摘

## 🔗 参考リンク

- [羅針盤技術ブログ - Gemini CLI](https://compasscorp.hatenablog.com/entry/github-pr-review-gemini-cli)
- [羅針盤技術ブログ - Codex CLI](https://compasscorp.hatenablog.com/entry/github-pr-review-codex-cli)
- [OpenAI Codex CLI](https://github.com/openai/codex)
- [Google Gemini CLI](https://github.com/google/gemini-cli)

## 📝 備考

- pnpm使用により高速インストール（4-5秒 vs 20-25秒）
- GitHub App認証による柔軟な権限管理
- マルチモデル対応による冗長性確保
- セキュリティ専用レビューによる包括的チェック

---

**実装者**: zapabob  
**完了日時**: 2025-10-23  
**ステータス**: ✅ 完了
