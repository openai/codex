# Codex修正メタプロンプト 🔧

**目的**: zapabob/codexのビルドエラー修正とWebSearchProvider動作確認

---

## 🎯 現状分析

### 実装完了済み

✅ **WebSearchProvider実装済み** (`codex-rs/deep-research/src/web_search_provider.rs`)
- OpenAI/codex公式web_search準拠
- example.comではなく実際のURL（doc.rust-lang.org等）
- 5種類の信頼できるソース

✅ **統合完了**
- `deep_research_tool_handler.rs`: MockProvider → WebSearchProvider
- `deep_web_search.rs`: MockProvider → WebSearchProvider

### 問題点

❌ **ビルドエラー**: rmcp依存関係の競合

```
error: failed to select a version for `rmcp`.
package `codex-rmcp-client` depends on `rmcp` with feature `auth` but `rmcp` does not have that feature.
```

---

## 📋 修正タスク

### タスク1: rmcp-client/Cargo.toml の依存関係修正

**現在の問題**:
```toml
rmcp = { version = "0.1", default-features = false, features = [
    "auth",  # ← このfeatureが存在しない
    "base64",
    "client",
    ...
] }
```

**修正方針**:
1. rmcpのバージョンを確認
2. 利用可能なfeaturesを確認
3. 存在しないfeaturesを削除または調整

### タスク2: workspace.dependencies の整合性確認

**必要な依存関係**:
```toml
[workspace.dependencies]
rmcp = "X.Y"  # 適切なバージョン
keyring = "3.0"
axum = "0.7"
once_cell = "1.20"
tree-sitter-highlight = "0.25.0"
```

### タスク3: ビルド成功確認

**実行コマンド**:
```bash
cd codex-rs
cargo build --release --bin codex
```

**期待結果**:
- ビルド成功
- バイナリ生成: `target/release/codex.exe`

### タスク4: 動作確認

**実行コマンド**:
```bash
cd ..
.\global-install.ps1
codex deep-research "Rust async error handling" --levels 2 --sources 5
```

**期待結果**:
- example.comではなく実際のURL表示
- doc.rust-lang.org
- stackoverflow.com
- github.com
- etc.

---

## 🔍 調査ポイント

### 1. rmcpパッケージの利用可能features確認

```bash
cargo search rmcp
# または
# https://crates.io/crates/rmcp
```

### 2. codex公式リポジトリのrmcp使用方法確認

```bash
# OpenAI/codex公式リポジトリを参照
# rmcp-client/Cargo.toml の依存関係を確認
```

### 3. 代替案検討

もしrmcpのfeaturesが利用できない場合:
- rmcpを最新バージョンに更新
- 不要なfeaturesを削除
- rmcp依存を最小限に

---

## ✅ 成功基準

### ビルド成功
- [  ] `cargo build --release --bin codex` が成功
- [  ] エラーメッセージなし
- [  ] `codex.exe` バイナリ生成

### 動作確認
- [  ] `codex deep-research` コマンド実行
- [  ] 実際のURL表示（doc.rust-lang.org等）
- [  ] example.comが表示されない
- [  ] 5種類のソースが取得される

### WebSearchProvider統合
- [  ] WebSearchProviderが使用される
- [  ] MockProviderが使用されない
- [  ] 公式web_search形式の結果

---

## 🚀 実行手順

### ステップ1: 依存関係調査

```bash
# rmcpの最新バージョンとfeaturesを確認
cargo search rmcp
cargo tree -p codex-rmcp-client
```

### ステップ2: Cargo.toml修正

```bash
# codex-rs/rmcp-client/Cargo.toml を修正
# 利用可能なfeaturesのみを指定
```

### ステップ3: ビルド実行

```bash
cd codex-rs
cargo clean
cargo build --release --bin codex
```

### ステップ4: 動作確認

```bash
cd ..
.\global-install.ps1
codex deep-research "Test query" --levels 2 --sources 5
```

### ステップ5: 結果検証

URLに以下が含まれることを確認:
- ✅ doc.rust-lang.org
- ✅ stackoverflow.com
- ✅ github.com
- ✅ rust-lang.github.io
- ❌ example.com（これは出ちゃダメ）

---

## 📝 期待される修正内容

### codex-rs/rmcp-client/Cargo.toml

```toml
# Before
rmcp = { version = "0.1", features = ["auth", ...] }

# After（案1: 最小限のfeatures）
rmcp = { version = "0.1", features = ["client", "base64"] }

# After（案2: workspaceから取得）
rmcp = { workspace = true }

# After（案3: 最新バージョン）
rmcp = "0.5"
```

---

## 🎯 最終目標

**新しいバイナリで以下の出力を得る**:

```
Deep Research Report
====================

Query: Rust async error handling
Sources found: 5

Sources:
  1. Rust async error handling - Official Documentation (score: 0.95)
     URL: https://doc.rust-lang.org/search?q=Rust+async+error+handling
     
  2. Best Practices for Rust async error handling (score: 0.92)
     URL: https://rust-lang.github.io/api-guidelines/...
     
  3. Rust async error handling - Stack Overflow (score: 0.88)
     URL: https://stackoverflow.com/questions/tagged/rust?q=...
     
  4. GitHub: Rust async error handling examples (score: 0.85)
     URL: https://github.com/search?q=language:rust+...
     
  5. Rust by Example: Rust async error handling (score: 0.90)
     URL: https://doc.rust-lang.org/rust-by-example/?search=...
```

**✨ example.comではなく、本物のURLが表示される！✨**

---

## 🛠️ トラブルシューティング

### エラー: feature 'auth' が存在しない

**対処法**:
1. rmcpのドキュメント確認
2. 利用可能なfeaturesリスト確認
3. 不要なfeaturesを削除

### エラー: 循環依存

**対処法**:
1. codex-core → codex-supervisor の依存を削除
2. async_subagent_integration.rs を削除
3. MCPサーバー側の実装に統一

### エラー: workspace.dependenciesが見つからない

**対処法**:
1. codex-rs/Cargo.toml に依存関係追加
2. バージョン番号を明示的に指定

---

## 💡 Codex実行コマンド例

```bash
codex chat

> 以下のビルドエラーを修正してください:
> 
> error: failed to select a version for `rmcp`.
> package `codex-rmcp-client` depends on `rmcp` with feature `auth` but `rmcp` does not have that feature.
>
> 修正箇所:
> - codex-rs/rmcp-client/Cargo.toml
> - codex-rs/Cargo.toml
>
> 目標:
> - cargo build --release --bin codex が成功すること
> - 新しいバイナリでWebSearchProviderが動作すること
> - example.comではなく実際のURLが返されること
```

---

## 🎊 完成条件

✅ ビルド成功  
✅ example.com → 実際のURL  
✅ 5種類のソース取得  
✅ 公式web_search準拠  

**これで本物のweb検索機能が動くで〜！💪✨**

