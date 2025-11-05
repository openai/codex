# Codex Sub-Agents & Deep Research Configuration

このディレクトリには、Codexのサブエージェント機能とDeep Research拡張の設定ファイルが格納されています。

## 📁 ディレクトリ構造

```
.codex/
├── agents/           # サブエージェント定義（YAML）
│   ├── researcher.yaml
│   ├── test-gen.yaml
│   └── sec-audit.yaml
├── policies/         # 権限・許可リスト
│   ├── net.allowlist
│   └── mcp.allowlist
├── prompts/          # システムプロンプト
│   ├── meta-prompt.md
│   └── starter-kit.md
└── scripts/          # 実行スクリプト
    ├── run_research.sh
    └── run_delegate.sh
```

## 🚀 使い方

### Deep Research実行

```bash
# 基本的な使用法
codex research "調査したいトピック" --depth 3 --budget 60000

# スクリプト経由
bash .codex/scripts/run_research.sh "Rustのプロセス分離 2023-2025比較"
```

### サブエージェント委任

```bash
# テスト生成エージェントに委任
codex delegate test-gen --scope ./src --deadline 2h

# セキュリティ監査エージェントに委任
codex delegate sec-audit --scope ./src --budget 40000

# コードレビューエージェントに委任
codex delegate code-reviewer --scope ./src --out artifacts/code-review.md

# スクリプト経由
bash .codex/scripts/run_delegate.sh sec-audit
```

## 🔧 サブエージェント設定

### 1. Deep Researcher
- **目的**: 計画的な多段探索と反証でレポート作成
- **ツール**: MCP検索, クローラー, PDFリーダー
- **出力**: `artifacts/report.md`, `artifacts/evidence/*.json`

### 2. Test Generator
- **目的**: 差分に対するユニット/回帰テスト自動生成
- **ツール**: コードインデクサ, npm/pytest/cargo/go
- **成功基準**: CI green, カバレッジ+10%

### 3. Security Auditor
- **目的**: CVE横断・依存監査・静的解析
- **ツール**: snyk, trivy, safety, bandit
- **出力**: 脆弱性レポート, 修正パッチ

### 4. Code Reviewer (Multi-Language)
- **目的**: 多言語対応コードレビュー・品質チェック・ベストプラクティス提案
- **対応言語**: TypeScript/JavaScript, Python, Rust, C# Unity
- **ツール**: eslint, pylint, clippy, dotnet-analyzer
- **出力**: レビューコメント, 改善提案, PRテンプレート
- **特徴**: 言語自動検出, フレームワーク対応, 重要度付きレビュー

#### 4-1. TypeScript Reviewer
- **特化**: React, Next.js, Express, NestJS
- **チェック**: 型安全性, async/await, Hooks, パフォーマンス

#### 4-2. Python Reviewer
- **特化**: Django, FastAPI, Flask, pytest
- **チェック**: PEP 8, 型ヒント, セキュリティ（SQL injection等）

#### 4-3. Unity Reviewer
- **特化**: Unity C#, パフォーマンス最適化, VR/AR
- **チェック**: GC回避, オブジェクトプール, ScriptableObject

## 📊 成果物（Artifacts）

すべての成果物は `artifacts/` ディレクトリに出力されます：

- `report.md`: リサーチレポート（出典付き）
- `evidence/*.json`: エビデンスデータ
- `test-report.md`: テストレポート
- `coverage-diff.json`: カバレッジ差分
- `sec-audit.md`: セキュリティ監査結果
- `patches/*.diff`: 修正パッチ

## 🔒 セキュリティ

- ネットワークアクセスは `policies/net.allowlist` で制限
- MCPツールは `policies/mcp.allowlist` で制限
- シークレットは自動的にリダクト
- 各エージェントは最小権限原則で動作

## 📚 参考資料

- [Meta-Prompt](.codex/prompts/meta-prompt.md): 詳細な実装指針
- [Starter Kit](.codex/prompts/starter-kit.md): 実装テンプレート集
- [要件定義書](../docs/codex-subagents-deep-research.md): 完全な仕様

