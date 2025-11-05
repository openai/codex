# 🔧 MCP エラー修正ガイド

**問題**: Cursor IDE で以下のエラーが表示される
```
■ MCP client for `web-search` failed to start: program not found
■ MCP client for `playwright` failed to start: program not found
■ unexpected status 400 Bad Request: {"detail":"Unsupported model"}
```

---

## 🎯 解決策（ベストプラクティス）

### ステップ1: デフォルトモデルを設定 ✅ **完了**

`~/.codex/config.toml` に以下を追加：

```toml
# デフォルトモデル（CLI実行時に --model オプションで上書き可能）
model = "gpt-4o-mini"  # 軽量で高速なデフォルトモデル
# 利用可能なモデル: gpt-4o, gpt-4o-mini, gpt-5-codex, gpt-5-codex-medium, o1-preview, o1-mini
```

**メリット**:
- デフォルトで高速なモデルを使用
- 必要に応じて `--model gpt-5-codex` で変更可能
- エラー "Unsupported model" を解消

---

### ステップ2: Cursor IDE を再起動 🔄

**理由**: Cursor IDE がキャッシュしている古い MCP 設定をクリアする必要がある

**手順**:
1. Cursor IDE を完全に終了（タスクバーからも終了）
2. 5秒待つ
3. Cursor IDE を再起動
4. エラーが消えているか確認

---

### ステップ3: 必要に応じて MCP サーバーを追加

#### オプションA: playwright を有効化（必要な場合のみ）

```bash
# パッケージをグローバルインストール
npm install -g @playwright/mcp

# Codex に登録
codex mcp add playwright -- npx -y @playwright/mcp

# 確認
codex mcp list
```

---

#### オプションB: web-search を有効化（必要な場合のみ）

```bash
# パッケージをグローバルインストール
npm install -g @modelcontextprotocol/server-brave-search

# Codex に登録
codex mcp add web-search -- npx -y @modelcontextprotocol/server-brave-search

# 確認
codex mcp list
```

**注意**: Brave Search API キーが必要です

---

### ステップ4: モデル指定方法（ベストプラクティス）

#### 方法1: デフォルトモデルを使用
```bash
codex "Create a Rust function"
# → gpt-4o-mini を使用（高速・安価）
```

#### 方法2: 実行時にモデルを指定
```bash
# 高性能モデルを使用
codex --model gpt-5-codex "Complex refactoring task"
codex --model gpt-4o "Advanced code generation"

# 推論モデルを使用
codex --model o1-preview "Solve complex algorithm problem"
```

#### 方法3: 設定ファイルで永続的に変更
```toml
# ~/.codex/config.toml
model = "gpt-5-codex-medium"  # いつもこのモデルを使う
```

---

## 📋 推奨設定（ベストプラクティス）

### config.toml
```toml
# デフォルトは軽量モデル
model = "gpt-4o-mini"

# MCP サーバーは必要なものだけ
[mcp_servers.codex-agent]
command = "codex"
args = ["mcp-server"]
env.CODEX_CONFIG_PATH = "C:\\Users\\downl\\.codex\\config.toml"
env.RUST_LOG = "info"
```

### 使用時
```bash
# 通常タスク（軽量モデル）
codex "Fix this bug"

# 複雑なタスク（高性能モデル）
codex --model gpt-5-codex "Implement complex feature"

# 推論タスク（推論モデル）
codex --model o1-preview "Solve algorithm problem"
```

---

## 🎯 現在の状態（修正後）

### ✅ 修正完了
- [x] デフォルトモデル設定（gpt-4o-mini）
- [x] 未インストールMCPサーバー削除
- [x] codex-agent のみ有効化

### 現在の MCP サーバー
```
Name         Command  Args        Env                                Status   
codex-agent  codex    mcp-server  CODEX_CONFIG_PATH=..., RUST_LOG=...  enabled
```

**ステータス**: ✅ **クリーン**

---

## 🚀 次のアクション

### 今すぐ実行

#### 1. Cursor IDE を再起動
- 完全に終了
- 5秒待つ
- 再起動
- エラーが消えているか確認

#### 2. デモを実行してテスト
```bash
# デフォルトモデル（gpt-4o-mini）で実行
codex "Create a simple Hello World in Rust"

# 高性能モデルで実行
codex --model gpt-5-codex "Analyze the demo_scripts.md file"

# codex-agent を使用
codex "Use codex-agent to review the config.toml file"
```

---

## 📊 期待される結果

### Before（修正前）
```
■ MCP client for `web-search` failed to start
■ MCP client for `playwright` failed to start
■ unexpected status 400 Bad Request: {"detail":"Unsupported model"}
```

### After（修正後）
```
✅ エラーなし
✅ codex-agent が正常動作
✅ モデルが正しく指定される
```

---

## 🎉 まとめ

### 実施した修正

1. ✅ **デフォルトモデル設定**
   - `gpt-4o-mini` をデフォルトに
   - 実行時に `--model` で変更可能

2. ✅ **未使用MCPサーバー削除**
   - playwright, web-search を削除
   - codex-agent のみ有効

3. ✅ **設定のクリーンアップ**
   - 不要な設定を削除
   - シンプルで明確な構成

### ベストプラクティス

**原則**: 
- デフォルトは軽量・高速なモデル
- 必要に応じて高性能モデルを指定
- MCPサーバーは必要なものだけ登録
- 設定はシンプルに保つ

**使用例**:
```bash
# 通常タスク
codex "Fix bug"

# 複雑なタスク  
codex --model gpt-5-codex "Complex refactoring"
```

---

**作成日**: 2025-10-13  
**Status**: ✅ **修正完了**  
**Next**: Cursor IDE を再起動して確認

