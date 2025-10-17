# 🤖 Gemini CLI 統合ガイド

**Codex起動中にGemini CLIを使ってGoogle検索を行う方法**

---

## 📋 目次

1. [概要](#概要)
2. [前提条件](#前提条件)
3. [セットアップ](#セットアップ)
4. [使用方法](#使用方法)
5. [仕組み](#仕組み)
6. [トラブルシューティング](#トラブルシューティング)

---

## 🎯 概要

Codex Deep Research機能に**Gemini CLI統合**を追加しました。これにより、Codex起動中にターミナルコマンドでGemini CLIを呼び出し、**Google Searchのグラウンディング機能**を使用した高品質な検索が可能になります。

### ✨ 主な特徴

- 🔍 **Google Search統合**: Gemini APIのGrounding機能で最新のWeb情報を取得
- 🤖 **Gemini CLI経由**: ターミナルコマンドで直接Geminiを呼び出し
- 📊 **高品質な結果**: Geminiによる自然言語処理と検索の組み合わせ
- 🔄 **フォールバック対応**: Gemini CLI失敗時は他の検索バックエンドにフォールバック

---

## 📦 前提条件

### 1. Gemini CLIのインストール

```bash
# Go環境が必要
# https://github.com/google/generative-ai-go

# Gemini CLIをインストール
go install github.com/google/generative-ai-go/cmd/gemini@latest

# インストール確認
gemini --version
```

**出力例**:
```
gemini version v0.5.0
```

### 2. Google API Keyの取得

1. [Google AI Studio](https://makersuite.google.com/app/apikey) にアクセス
2. "Create API Key" をクリック
3. APIキーをコピー

### 3. 環境変数の設定

```bash
# GOOGLE_API_KEYを設定
export GOOGLE_API_KEY="your-google-api-key-here"

# 永続化（bashの場合）
echo 'export GOOGLE_API_KEY="your-google-api-key-here"' >> ~/.bashrc
source ~/.bashrc

# PowerShellの場合
$env:GOOGLE_API_KEY="your-google-api-key-here"
# 永続化
[Environment]::SetEnvironmentVariable("GOOGLE_API_KEY", "your-google-api-key-here", "User")
```

---

## 🚀 セットアップ

### Codexのビルド

```bash
cd codex-rs
cargo build --release -p codex-deep-research
cargo build --release -p codex-cli

# CLIをグローバルインストール
cd ../codex-cli
npm install -g .
```

---

## 💻 使用方法

### 基本的な使い方

```bash
# Gemini CLIを使用してDeep Researchを実行
codex research "Rust async best practices" --gemini
```

**実行結果**:
```
🔍 Starting deep research on: Rust async best practices
   Depth: 3, Breadth: 8
   Budget: 60000 tokens

🤖 Using Gemini CLI with Google Search (Grounding)
   ✅ GOOGLE_API_KEY detected

📊 Research Report:
   Sources found: 12
   Confidence: High

💾 Report saved to: artifacts/report.md
```

### オプション付き使用

```bash
# 深度と幅を指定
codex research "WebAssembly performance" \
  --gemini \
  --depth 5 \
  --breadth 15

# トークン予算を指定
codex research "Machine Learning frameworks" \
  --gemini \
  --budget 100000 \
  --out ml-research.md

# 軽量版フォールバックと組み合わせ
codex research "Quick topic" \
  --gemini \
  --depth 2 \
  --lightweight-fallback
```

### 通常のWeb検索との比較

```bash
# 通常のWeb検索（DuckDuckGo/Brave/Google Custom Search）
codex research "Rust async" --depth 3

# Gemini CLI経由（Google Search + Gemini AI）
codex research "Rust async" --depth 3 --gemini
```

---

## 🔧 仕組み

### アーキテクチャ

```
┌─────────────────┐
│   Codex CLI     │
│  research cmd   │
└────────┬────────┘
         │
         │ --gemini flag
         │
         ▼
┌─────────────────┐
│ GeminiSearch    │
│   Provider      │
└────────┬────────┘
         │
         │ subprocess call
         │
         ▼
┌─────────────────┐        ┌──────────────┐
│   Gemini CLI    │───────>│ Gemini API   │
│                 │        │ (Grounding)  │
└─────────────────┘        └──────┬───────┘
                                  │
                                  │ Google Search
                                  │
                                  ▼
                           ┌──────────────┐
                           │ Search       │
                           │ Results      │
                           └──────────────┘
```

### 検索フロー

1. **コマンド実行**: `codex research "query" --gemini`
2. **GeminiSearchProvider起動**: 環境変数チェック
3. **Gemini CLI呼び出し**: 
   ```bash
   gemini "Search for: <query>" \
     --api-key $GOOGLE_API_KEY \
     --model gemini-1.5-pro \
     --grounding \
     --json
   ```
4. **結果パース**: JSON/テキストからSearchResultsを抽出
5. **レポート生成**: Markdown形式で保存

### 優先順位

Codex Deep Researchの検索バックエンド優先順位：

```
1. Gemini CLI (--gemini指定時)
2. MCP Search Provider (--mcp指定時)
3. Web Search Provider
   ├─ Brave Search API (BRAVE_API_KEY)
   ├─ Google Custom Search (GOOGLE_API_KEY + GOOGLE_CSE_ID)
   ├─ Bing Search API (BING_API_KEY)
   └─ DuckDuckGo (APIキー不要)
```

---

## 🐛 トラブルシューティング

### Q1: `gemini: command not found`

**原因**: Gemini CLIがインストールされていない

**解決策**:
```bash
# Goがインストールされているか確認
go version

# Gemini CLIをインストール
go install github.com/google/generative-ai-go/cmd/gemini@latest

# PATHを確認
echo $PATH | grep go/bin

# 必要に応じてPATHに追加
export PATH="$PATH:$HOME/go/bin"
```

### Q2: `GOOGLE_API_KEY is required for Gemini CLI`

**原因**: GOOGLE_API_KEYが設定されていない

**解決策**:
```bash
# 環境変数を設定
export GOOGLE_API_KEY="your-api-key"

# 設定確認
echo $GOOGLE_API_KEY

# または.envファイルに追加
echo 'GOOGLE_API_KEY=your-api-key' >> .env
```

### Q3: Gemini CLI実行に失敗する

**原因**: APIキーが無効、またはクォータ超過

**解決策**:
```bash
# Gemini CLIを直接テスト
gemini "Hello" --api-key $GOOGLE_API_KEY

# エラーメッセージを確認
# "Invalid API key" → APIキーを再確認
# "Quota exceeded" → [Google AI Studio](https://makersuite.google.com/)で確認

# フォールバック検索を使用
codex research "query" --depth 3
# （Gemini CLIなしでDuckDuckGoにフォールバック）
```

### Q4: 検索結果が空

**原因**: HTMLパースに失敗、またはGrounding機能がオフ

**解決策**:
```bash
# --groundingフラグが正しく渡されているか確認
# codex-rs/deep-research/src/gemini_search_provider.rsを確認

# デバッグモードで実行
RUST_LOG=debug codex research "query" --gemini

# または通常のWeb検索を使用
codex research "query" --depth 3
```

### Q5: タイムアウトエラー

**原因**: Gemini APIの応答が遅い

**解決策**:
```bash
# リトライロジックは実装済み（最大3回）
# タイムアウト設定を確認:
# codex-rs/deep-research/src/gemini_search_provider.rs
# デフォルト: 30秒

# より軽量な検索を使用
codex research "query" --depth 2 --breadth 5 --gemini
```

---

## 📊 比較: Gemini CLI vs 他の検索バックエンド

| 検索方法 | APIキー | 品質 | 速度 | コスト |
|---------|---------|------|------|-------|
| **Gemini CLI** | GOOGLE_API_KEY | ⭐⭐⭐⭐⭐ | 1-2秒 | 有料（クォータあり） |
| Google Custom Search | GOOGLE_API_KEY + CSE_ID | ⭐⭐⭐⭐ | 0.3-0.8秒 | 有料 |
| Brave Search | BRAVE_API_KEY | ⭐⭐⭐⭐ | 0.5-1秒 | 有料 |
| DuckDuckGo | 不要 | ⭐⭐⭐ | 1-3秒 | 無料 |

### Gemini CLIの利点

1. **AI強化検索**: Gemini APIによる自然言語理解
2. **Google Search統合**: 最新かつ信頼性の高い情報
3. **Grounding機能**: 事実に基づいた回答
4. **コンテンツ要約**: URLから直接コンテンツを要約可能

---

## 🎯 実践例

### 例1: 技術調査

```bash
# Gemini CLIで最新技術を調査
codex research "Rust 2024 edition new features" \
  --gemini \
  --depth 5 \
  --breadth 12 \
  --out rust-2024-research.md

# レポート確認
cat rust-2024-research.md
```

### 例2: 競合分析

```bash
# 複数のフレームワークを比較
codex research "React vs Vue vs Svelte performance 2025" \
  --gemini \
  --depth 4 \
  --budget 80000 \
  --citations
```

### 例3: 軽量クエリ

```bash
# クイックリサーチ（トークン節約）
codex research "TypeScript 5.4 changes" \
  --gemini \
  --depth 2 \
  --breadth 5 \
  --lightweight-fallback
```

---

## 🔗 関連リソース

- **Gemini API ドキュメント**: https://ai.google.dev/docs
- **Gemini CLI リポジトリ**: https://github.com/google/generative-ai-go
- **Google AI Studio**: https://makersuite.google.com/
- **Codex Deep Research README**: `codex-rs/deep-research/README.md`

---

## 📝 まとめ

Gemini CLI統合により、Codex Deep Researchは以下を実現しました：

✅ **Google Search + Gemini AI**の強力な組み合わせ  
✅ **ターミナルから直接**Gemini CLIを呼び出し  
✅ **高品質な検索結果**と自然言語理解  
✅ **フォールバック機能**でAPIキーなしでも動作  

---

**作成日**: 2025-10-11  
**バージョン**: 0.47.0-alpha.1  
**Status**: ✅ Production Ready

