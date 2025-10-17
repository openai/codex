# Codex Sub-Agents Quick Start Guide

> 🚀 **zapabob/codex Enhanced Feature**: Specialized AI sub-agents for delegated tasks

## 概要

Codex サブエージェントは、特定のタスクに特化したAIエージェントを呼び出す機能です。コードレビュー、テスト生成、セキュリティ監査、調査タスクなどを自動化できます。

## インストール

```bash
# グローバルインストール
npm install -g @openai/codex

# または、Rust バイナリを直接ビルド
cd codex-rs
cargo build --release -p codex-cli
```

## 利用可能なサブエージェント

### 1. Code Reviewer (`code-reviewer`)

**目的**: コードの包括的なレビュー（セキュリティ、パフォーマンス、ベストプラクティス）

**使用例**:
```bash
codex delegate code-reviewer \
  --goal "Review TypeScript components for security issues" \
  --scope ./src/components \
  --budget 40000
```

**チェック項目**:
- 型安全性（TypeScript/Rust）
- セキュリティ脆弱性（SQL injection, XSS等）
- パフォーマンス最適化
- 言語固有のベストプラクティス

**出力**:
- `artifacts/code-review-report.md` - 詳細レビューレポート
- `code-review-reports/review-summary.json` - JSON形式サマリー

---

### 2. Test Generator (`test-gen`)

**目的**: 包括的なテストスイート生成（Unit, Integration, E2E）

**使用例**:
```bash
codex delegate test-gen \
  --goal "Generate unit tests for user authentication module" \
  --scope ./src/auth \
  --budget 30000
```

**生成内容**:
- Unit テスト（80%+ カバレッジ目標）
- Integration テスト
- エッジケース・エラーハンドリングテスト
- テストフィクスチャとモック

**出力**:
- `artifacts/test-generation-report.md`
- `artifacts/test-coverage-analysis.json`

---

### 3. Security Auditor (`sec-audit`)

**目的**: セキュリティ監査（CVEスキャン、依存関係分析、脆弱性パッチ提案）

**使用例**:
```bash
codex delegate sec-audit \
  --goal "Audit dependencies for CVEs" \
  --budget 50000
```

**チェック項目**:
- 全依存関係のCVEスキャン
- コード内の潜在的脆弱性
- パッチ推奨（バージョン番号付き）
- 優先度別レポート（Critical/High/Medium/Low）

**出力**:
- `artifacts/security-audit-report.md`
- `security-reports/vulnerability-summary.json`
- `security-reports/patch-recommendations.md`

---

### 4. Researcher (`researcher`)

**目的**: 複数ソースからの調査・検証・引用付きレポート生成

**使用例**:
```bash
codex delegate researcher \
  --goal "Research React Server Components best practices" \
  --budget 60000
```

**調査内容**:
- 5+ 信頼できるソースから情報収集
- ファクトのクロス検証・矛盾検出
- 全主張に引用を提供
- 実装例を含む構造化レポート

**出力**:
- `artifacts/research-report.md`
- `research-reports/sources.json`
- `research-reports/cross-validation-report.md`

---

## コマンドオプション

### `codex delegate` コマンド

```bash
codex delegate <AGENT> [OPTIONS]
```

**必須引数**:
- `<AGENT>` - エージェント名（例: `code-reviewer`, `test-gen`, `sec-audit`, `researcher`）

**オプション**:
- `--goal <GOAL>` - タスクの目標（省略時は scope から自動生成）
- `--scope <PATH>` - 対象ディレクトリ/ファイル
- `--budget <TOKENS>` - トークン予算（デフォルトはエージェント定義に従う）
- `--deadline <MINUTES>` - タイムアウト時間（分）
- `--out <PATH>` - 結果レポートの出力先JSONファイル

---

## カスタムエージェントの作成

### エージェント定義ファイル（YAML）

エージェントは `.codex/agents/<name>.yaml` に定義します。

**例: custom-agent.yaml**

```yaml
name: "custom-agent"
goal: "Custom agent for specific tasks"
tools:
  mcp:
    - grep
    - read_file
    - codebase_search
  fs:
    read: true
    write:
      - "./artifacts"
  net:
    allow:
      - "https://docs.rs/*"
  shell:
    exec:
      - cargo
      - npm
policies:
  context:
    max_tokens: 30000
    retention: "job"
  secrets:
    redact: true
success_criteria:
  - "Criterion 1"
  - "Criterion 2"
artifacts:
  - "artifacts/custom-output.md"
```

### フィールド説明

| フィールド | 説明 |
|-----------|------|
| `name` | エージェント名 |
| `goal` | エージェントの目的 |
| `tools.mcp` | 利用可能なMCPツールリスト |
| `tools.fs.read` | ファイル読み取り許可 |
| `tools.fs.write` | 書き込み許可パスリスト |
| `tools.net.allow` | ネットワークアクセス許可パターン |
| `tools.shell.exec` | 実行可能なシェルコマンドリスト |
| `policies.context.max_tokens` | 最大トークン数 |
| `policies.context.retention` | コンテキスト保持期間（`job`, `session`, `permanent`） |
| `policies.secrets.redact` | シークレット自動除去 |
| `success_criteria` | 成功基準リスト |
| `artifacts` | 生成するアーティファクトパス |

---

## ベストプラクティス

### 1. 適切な予算設定

```bash
# 小規模タスク（単一ファイルレビュー）
codex delegate code-reviewer --scope ./src/app.ts --budget 10000

# 中規模タスク（モジュール単位）
codex delegate test-gen --scope ./src/auth --budget 30000

# 大規模タスク（プロジェクト全体監査）
codex delegate sec-audit --budget 50000
```

### 2. scope の効果的な使用

```bash
# 特定ファイル
codex delegate code-reviewer --scope ./src/components/Button.tsx

# ディレクトリ全体
codex delegate test-gen --scope ./src/services

# プロジェクトルート
codex delegate sec-audit --scope ./
```

### 3. 結果の永続化

```bash
codex delegate code-reviewer \
  --scope ./src \
  --out ./reports/code-review-$(date +%Y%m%d).json
```

### 4. Deep Research との組み合わせ

```bash
# まず調査
codex research "Rust async/await best practices" --depth 3

# 調査結果を基にコードレビュー
codex delegate code-reviewer \
  --goal "Review Rust code for async/await best practices based on research" \
  --scope ./src
```

---

## トラブルシューティング

### エージェントが見つからない

```bash
❌ Agent 'code-reviewer' not found
   Available agents:
     - code-reviewer
     - test-gen
```

**解決策**: `.codex/agents/` ディレクトリにYAMLファイルがあるか確認

```bash
ls -la .codex/agents/
```

### 予算超過エラー

```bash
❌ Token budget exceeded for agent 'sec-audit'
```

**解決策**: `--budget` を増やすか、`--scope` を狭める

```bash
codex delegate sec-audit --scope ./src/core --budget 80000
```

### 権限エラー

```bash
❌ File write permission denied
```

**解決策**: エージェント定義の `tools.fs.write` を確認

```yaml
tools:
  fs:
    write:
      - "./artifacts"      # これが必要
      - "./your-output-dir"
```

---

## CI/CD統合

### GitHub Actions

```yaml
name: Codex Sub-Agent Review

on:
  pull_request:
    types: [opened, synchronize]

jobs:
  code-review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
      
      - name: Install Codex
        run: npm install -g @openai/codex
      
      - name: Run Code Reviewer
        env:
          OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}
        run: |
          codex delegate code-reviewer \
            --scope ./src \
            --out ./code-review-report.json
      
      - name: Upload Report
        uses: actions/upload-artifact@v4
        with:
          name: code-review-report
          path: ./code-review-report.json
```

---

## 詳細ドキュメント

- [要件定義書](docs/REQUIREMENTS_SPECIFICATION.md) - 機能仕様
- [実装計画](_docs/2025-10-11_要件定義書に基づく実装計画.md) - 実装ロードマップ
- [メタプロンプト](.codex/META_PROMPT_CONTINUOUS_IMPROVEMENT.md) - 開発ガイドライン

---

## ライセンス

Apache-2.0

---

**Project**: zapabob/codex  
**Version**: 0.47.0-alpha.1  
**Last Updated**: 2025-10-11
