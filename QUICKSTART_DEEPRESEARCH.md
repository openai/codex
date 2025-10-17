# 🚀 Deep Research & Sub-Agent クイックスタート

**5分で始めるCodex Deep Research & サブエージェント機能**

---

## 📋 目次

1. [インストール](#インストール)
2. [Deep Research - 基本編](#deep-research---基本編)
3. [Deep Research - 応用編](#deep-research---応用編)
4. [サブエージェント - 基本編](#サブエージェント---基本編)
5. [サブエージェント - 応用編](#サブエージェント---応用編)
6. [カスタムコマンド一覧](#カスタムコマンド一覧)

---

## 1️⃣ インストール

### 必要なもの

- Rust 1.76+ （[rustup.rs](https://rustup.rs)）
- Node.js 18+ （npm）
- Git

### インストール手順

```bash
# 1. リポジトリをクローン
git clone https://github.com/zapabob/codex.git
cd codex

# 2. Rustコンポーネントをビルド
cd codex-rs
cargo build --release -p codex-deep-research

# 3. CLIをグローバルインストール
cd ../codex-cli
npm install -g .

# 4. 動作確認
codex --version
```

**期待される出力**:
```
codex 0.47.0-alpha.1
```

---

## 2️⃣ Deep Research - 基本編

### 最初のDeep Research（APIキー不要）

```bash
codex research "What are Rust async best practices?"
```

**実行結果**:
```
🔍 Starting deep research on: What are Rust async best practices?
   Depth: 3, Breadth: 8
   Budget: 60000 tokens

🌐 Using Web Search Provider with DuckDuckGo integration
   Priority: Brave > Google > Bing > DuckDuckGo (no API key required)
   🔓 No API keys found, using DuckDuckGo (free, no API key required)

📊 Research Report:
   Sources found: 12
   Confidence: High

💾 Report saved to: artifacts/report.md
```

### レポートを確認

```bash
cat artifacts/report.md
```

---

## 3️⃣ Deep Research - 応用編

### 深い調査（Depth 5）

```bash
codex research "Rust memory safety mechanisms" --depth 5
```

### 幅広い調査（Breadth 20）

```bash
codex research "Web framework comparison 2025" --breadth 20
```

### トークン節約モード

```bash
codex research "Quick topic" \
  --depth 2 \
  --breadth 5 \
  --budget 15000 \
  --lightweight-fallback
```

### カスタム出力先

```bash
codex research "Kubernetes best practices" \
  --depth 4 \
  --out kubernetes-research.md
```

### Gemini CLI統合（新機能）

```bash
# Gemini CLIでGoogle検索を使用（要: gemini CLI + GOOGLE_API_KEY）
codex research "Rust async best practices" \
  --gemini \
  --depth 4

# 環境変数設定
export GOOGLE_API_KEY="your-google-api-key"

# Gemini CLI経由で高品質検索
codex research "Latest AI trends 2025" \
  --gemini \
  --breadth 15
```

**詳細**: [Gemini CLI統合ガイド](docs/gemini-cli-integration.md)

### MCP統合（高度）

```bash
# MCPサーバーを起動（別ターミナル）
codex mcp server --port 3000

# MCP経由で調査
codex research "AI safety research" \
  --mcp "http://localhost:3000" \
  --depth 4
```

---

## 4️⃣ サブエージェント - 基本編

### コードレビュー

```bash
# TypeScriptコードをレビュー
codex delegate code-reviewer --scope ./src
```

**実行結果**:
```
🤖 Delegating to agent 'code-reviewer'...
   Goal: Process files in ./src
   Budget: 40000 tokens

✅ Agent 'code-reviewer' completed!
   Status: Success
   Tokens used: 12500
   Duration: 15.3s

📄 Generated artifacts:
   - code-review-report.md
   - issues-found.json
```

### テスト生成

```bash
codex delegate test-gen --scope ./src/utils
```

### セキュリティ監査

```bash
codex delegate sec-audit \
  --scope ./ \
  --out security-audit.json
```

---

## 5️⃣ サブエージェント - 応用編

### 言語別レビュー

#### TypeScript/JavaScript

```bash
codex delegate ts-reviewer \
  --goal "Review React components for hooks violations" \
  --scope ./src/components \
  --budget 50000
```

#### Python

```bash
codex delegate python-reviewer \
  --goal "Check PEP 8 compliance and type hints" \
  --scope ./backend \
  --budget 40000
```

#### Rust

```bash
codex delegate rust-reviewer \
  --goal "Check Clippy warnings and unsafe code" \
  --scope ./src \
  --budget 30000
```

#### Unity C#

```bash
codex delegate unity-reviewer \
  --goal "Check GC allocations in Update loops" \
  --scope ./Assets/Scripts \
  --budget 35000
```

### カスタムゴール指定

```bash
codex delegate code-reviewer \
  --goal "Find all TODO comments and create issue tracker" \
  --scope ./src \
  --out todos.json
```

### デッドライン指定

```bash
codex delegate test-gen \
  --scope ./src/api \
  --deadline 30 \
  --out tests/api/
```

（30分以内に完了を試みる）

---

## 6️⃣ カスタムコマンド一覧

### Deep Research コマンド

| コマンド | 説明 | 例 |
|---------|------|-----|
| `codex research <topic>` | 基本的な調査 | `codex research "Rust"` |
| `--depth <1-5>` | 調査の深さ | `--depth 5` |
| `--breadth <N>` | ソース数 | `--breadth 20` |
| `--budget <N>` | トークン上限 | `--budget 100000` |
| `--citations` | 引用を含める | `--citations` |
| `--lightweight-fallback` | 軽量版使用 | `--lightweight-fallback` |
| `--gemini` | Gemini CLI使用 | `--gemini` |
| `--mcp <URL>` | MCP統合 | `--mcp "http://localhost:3000"` |
| `--out <FILE>` | 出力先 | `--out report.md` |

### サブエージェント コマンド

| コマンド | 説明 | 例 |
|---------|------|-----|
| `codex delegate <agent>` | エージェント呼び出し | `codex delegate code-reviewer` |
| `--goal <TEXT>` | ゴール指定 | `--goal "Review code"` |
| `--scope <PATH>` | 対象パス | `--scope ./src` |
| `--budget <N>` | トークン上限 | `--budget 40000` |
| `--deadline <MIN>` | 制限時間 | `--deadline 30` |
| `--out <FILE>` | 出力先 | `--out result.json` |

### 利用可能なエージェント

| エージェント名 | 用途 | 推奨Budget |
|--------------|------|-----------|
| `code-reviewer` | 汎用コードレビュー | 40,000 |
| `ts-reviewer` | TypeScript専用 | 35,000 |
| `python-reviewer` | Python専用 | 35,000 |
| `rust-reviewer` | Rust専用 | 30,000 |
| `unity-reviewer` | Unity C#専用 | 40,000 |
| `test-gen` | テスト生成 | 50,000 |
| `sec-audit` | セキュリティ監査 | 60,000 |

---

## 🎯 実践例

### 例1: 新技術の調査

```bash
# 深く詳細に調査
codex research "WebAssembly WASI preview 2" \
  --depth 5 \
  --breadth 15 \
  --citations \
  --out wasi-research.md

# レポート確認
cat wasi-research.md
```

### 例2: プロジェクト全体のセキュリティチェック

```bash
# セキュリティ監査実行
codex delegate sec-audit \
  --goal "Find SQL injection and XSS vulnerabilities" \
  --scope ./ \
  --budget 80000 \
  --out security-report.json

# 結果確認
cat security-report.json | jq '.artifacts'
```

### 例3: テストカバレッジ向上

```bash
# 既存コードに対してテスト生成
codex delegate test-gen \
  --goal "Generate unit tests with 80% coverage" \
  --scope ./src/services \
  --budget 60000 \
  --out tests/services/

# 生成されたテスト確認
ls -la tests/services/
```

### 例4: コードレビュー自動化

```bash
# TypeScriptプロジェクトをレビュー
codex delegate ts-reviewer \
  --goal "Review for React hooks rules, type safety, and performance" \
  --scope ./src \
  --budget 50000 \
  --deadline 45 \
  --out review-report.md

# レビュー結果確認
cat review-report.md
```

---

## 🔧 環境変数設定（オプション）

### 商用API利用時

```bash
# Brave Search API（推奨）
export BRAVE_API_KEY="your-brave-api-key"

# Google Custom Search
export GOOGLE_API_KEY="your-google-api-key"
export GOOGLE_CSE_ID="your-cse-id"

# Bing Web Search
export BING_API_KEY="your-bing-api-key"

# 永続化（bashの場合）
echo 'export BRAVE_API_KEY="your-key"' >> ~/.bashrc
source ~/.bashrc
```

### 設定確認

```bash
# APIキーが設定されているか確認
codex research "test" --depth 1

# 期待される出力（APIキー設定時）
# ✅ Brave Search API detected
```

---

## 📊 パフォーマンスヒント

### トークン節約のコツ

1. **Depth を調整**
   - Depth 1-2: クイックリサーチ（5,000-15,000トークン）
   - Depth 3: 標準的な調査（25,000-50,000トークン）
   - Depth 4-5: 深い調査（60,000-150,000トークン）

2. **Breadth を調整**
   - Breadth 5: 最小限のソース
   - Breadth 8: 標準（デフォルト）
   - Breadth 15-20: 包括的な調査

3. **Lightweight Fallback を使用**
   ```bash
   codex research "topic" \
     --depth 2 \
     --breadth 5 \
     --budget 15000 \
     --lightweight-fallback
   ```

### 速度最適化

1. **DuckDuckGo使用**（APIキーなし）
   - 無料・即座に利用可能
   - 応答速度: 1-3秒

2. **商用API使用**（APIキーあり）
   - Brave: 0.5-1秒（推奨）
   - Google: 0.3-0.8秒（最速）
   - Bing: 0.5-1秒

---

## 🐛 よくある問題

### Q1: `codex: command not found`

**解決策**:
```bash
# グローバルインストールを再実行
cd codex-cli
npm install -g .

# PATHを確認
echo $PATH
```

### Q2: タイムアウトエラー

**解決策**:
```bash
# ネットワーク確認
ping google.com

# タイムアウト時間を延長（設定ファイル編集）
# codex-rs/deep-research/src/web_search_provider.rs
# .timeout(std::time::Duration::from_secs(60))
```

### Q3: レポートが生成されない

**解決策**:
```bash
# 出力ディレクトリを確認
ls -la artifacts/

# 明示的にパス指定
codex research "topic" --out $(pwd)/my-report.md
```

---

## 🎓 次のステップ

### 学習リソース

1. **詳細ドキュメント**: `codex-rs/deep-research/README.md`
2. **Gemini CLI統合**: `docs/gemini-cli-integration.md` 🆕
3. **サブエージェント設定**: `.codex/agents/*.yaml`
4. **API統合ガイド**: `docs/codex-subagents-deep-research.md`

### 高度な使い方

1. **カスタムエージェント作成**: `.codex/agents/custom-agent.yaml`
2. **MCPサーバー統合**: `codex mcp server`
3. **CI/CD統合**: GitHub Actions でのレビュー自動化

---

## 🎉 完了！

これでCodex Deep Research & サブエージェント機能を使いこなせるはずや！

**困ったら**:
- GitHub Issues: https://github.com/zapabob/codex/issues
- ドキュメント: `docs/`
- サンプル: `_docs/`

---

**作成日**: 2025-10-11  
**バージョン**: 0.47.0-alpha.1  
**Status**: ✅ Production Ready

