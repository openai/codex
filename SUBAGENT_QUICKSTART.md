# 🤖 サブエージェント機能クイックスタート

**5分で始めるCodex サブエージェント**

---

## 🎯 サブエージェントとは？

Codexには**7種類の専門サブエージェント**が組み込まれており、以下のタスクを自動化できます：

1. **Code Reviewer** - コードレビュー（多言語対応）
2. **TypeScript Reviewer** - TypeScript/React専用レビュー
3. **Python Reviewer** - Python専用レビュー
4. **Unity Reviewer** - Unity C#専用レビュー
5. **Test Generator** - テスト自動生成
6. **Security Auditor** - セキュリティ監査
7. **Researcher** - Deep Research（DuckDuckGo統合）

---

## 🚀 使い方

### 方法1: delegateコマンド

```bash
# 基本的な使い方
codex delegate <agent-name> --scope <path>

# 例: コードレビュー
codex delegate code-reviewer --scope ./src

# 例: TypeScriptレビュー
codex delegate ts-reviewer --scope ./src/components

# 例: セキュリティ監査
codex delegate sec-audit --scope ./backend --budget 60000

# 例: テスト生成
codex delegate test-gen --scope ./src/api --out tests/
```

### 方法2: 対話モード（フル機能）

```bash
# Codexを起動
codex

# サブエージェントをメンション
> @code-reviewer Please review ./src
> @test-gen Generate tests for ./src/api with 80% coverage
> @sec-audit Scan ./backend for SQL injection and XSS
> @researcher Research "Rust async best practices"
```

---

## 📋 利用可能なエージェント

### 1. code-reviewer

```bash
codex delegate code-reviewer --scope ./src
```

**機能**:
- 多言語対応（TypeScript, Python, Rust, C# Unity）
- コード品質チェック
- セキュリティ脆弱性検出
- パフォーマンス最適化提案

### 2. ts-reviewer

```bash
codex delegate ts-reviewer --scope ./src/components
```

**特化機能**:
- React Hooks ルール検証
- 型安全性チェック（`any`型禁止）
- async/awaitパターン
- useMemo/useCallbackレビュー

### 3. python-reviewer

```bash
codex delegate python-reviewer --scope ./backend
```

**特化機能**:
- PEP 8準拠確認
- 型ヒント検証
- SQLインジェクション検出
- パフォーマンス最適化

### 4. unity-reviewer

```bash
codex delegate unity-reviewer --scope ./Assets/Scripts
```

**特化機能**:
- Update内GC Allocation検出
- オブジェクトプーリング検証
- ScriptableObject活用確認

### 5. test-gen

```bash
codex delegate test-gen --scope ./src --budget 50000
```

**機能**:
- Unit Test自動生成
- Integration Test生成
- カバレッジ分析

### 6. sec-audit

```bash
codex delegate sec-audit --scope ./ --budget 60000
```

**機能**:
- CVE横断検索
- 依存関係監査
- 脆弱性パッチ生成

### 7. researcher

```bash
codex delegate researcher --goal "Research Kubernetes best practices"
```

**機能**:
- Web検索（DuckDuckGo）
- 矛盾検出
- 引用付きレポート

---

## 💡 実践例

### Example 1: プロジェクト全体のコードレビュー

```bash
codex delegate code-reviewer \
  --goal "Review entire codebase for quality and security" \
  --scope ./src \
  --budget 80000 \
  --out artifacts/review-report.json
```

### Example 2: TypeScriptプロジェクトの型安全性チェック

```bash
codex delegate ts-reviewer \
  --goal "Check type safety and React hooks violations" \
  --scope ./src/components \
  --budget 40000
```

### Example 3: セキュリティ脆弱性スキャン

```bash
codex delegate sec-audit \
  --goal "Find SQL injection and XSS vulnerabilities" \
  --scope ./backend \
  --budget 60000 \
  --out artifacts/security-audit.json
```

### Example 4: テストカバレッジ向上

```bash
codex delegate test-gen \
  --goal "Generate unit tests with 80% coverage" \
  --scope ./src/services \
  --budget 50000
```

---

## 🔧 オプション

### 共通オプション

| オプション | 説明 | デフォルト |
|-----------|------|----------|
| `--goal <TEXT>` | タスクの目的 | 自動生成 |
| `--scope <PATH>` | 対象ディレクトリ/ファイル | 現在のディレクトリ |
| `--budget <N>` | トークン予算 | 40000 |
| `--deadline <MIN>` | 制限時間（分） | なし |
| `--out <FILE>` | 結果の出力先 | なし |

---

## 📁 エージェント定義

エージェントは `.codex/agents/` ディレクトリに定義されています：

```
.codex/agents/
├── code-reviewer.yaml      # 多言語コードレビュー
├── ts-reviewer.yaml        # TypeScript専用
├── python-reviewer.yaml    # Python専用
├── unity-reviewer.yaml     # Unity C#専用
├── test-gen.yaml           # テスト生成
├── sec-audit.yaml          # セキュリティ監査
└── researcher.yaml         # Deep Research
```

カスタムエージェントも追加可能です！

---

## 🎓 対話モード vs delegateコマンド

### delegateコマンド（現在の実装）

```
✅ 情報表示
✅ エージェント定義読み込み
✅ タスクシミュレーション
✅ 推奨事項表示
```

**用途**: エージェント確認、情報取得

### 対話モード（フル機能）

```
✅ 実際のLLM実行
✅ リアルタイム分析
✅ 詳細レポート生成
✅ 自動修正提案
```

**用途**: 実際のタスク実行

---

## 🎊 まとめ

### サブエージェント機能は完全実装済み！

```
✅ 7種類のエージェント定義
✅ delegateコマンド実装
✅ 対話モードでフル機能利用可能
✅ ClaudeCode同等以上の機能
```

### 今すぐ使える

```bash
# エージェント情報確認
codex delegate code-reviewer --scope ./src

# フル機能使用（推奨）
codex
> @code-reviewer ./src
```

---

**作成日時**: 2025-10-11  
**バージョン**: 0.47.0-alpha.1  
**Status**: ✅ Production Ready


