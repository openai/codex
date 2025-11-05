# 🔌 Codex MCP 統合セットアップガイド

**Version**: 0.47.0-alpha.1  
**Last Updated**: 2025-10-13  
**Status**: 🚧 Phase 2 実装中

> **概要**: Codex 自身を MCP サーバー化し、サブエージェントから Codex の全機能を使えるようにする

---

## 🎯 目的

Codex を Model Context Protocol (MCP) サーバーとして動作させ、サブエージェントが以下の機能を使えるようにする：

- ✅ `codex_read_file` - Codex 経由でファイル読み取り
- ✅ `codex_grep` - Codex 経由で grep 検索
- ✅ `codex_codebase_search` - セマンティック検索
- ✅ `codex_apply_patch` - パッチ適用（書き込み権限必要）
- ⚠️ `codex_shell` - シェルコマンド実行（危険なため通常は許可しない）

---

## 📋 前提条件

### 必要なもの

1. ✅ Codex CLI がインストール済み
   ```bash
   codex --version
   # 期待: codex-cli 0.47.0-alpha.1
   ```

2. ✅ OpenAI API キーが設定済み
   ```bash
   echo $OPENAI_API_KEY  # Linux/macOS
   echo $env:OPENAI_API_KEY  # Windows PowerShell
   ```

3. ✅ Rust ツールチェインがインストール済み（開発用）
   ```bash
   rustc --version
   cargo --version
   ```

---

## 🚀 セットアップ手順

### ステップ 1: 設定ファイルの作成

`~/.codex/config.toml` に Codex MCP サーバーの設定を追加：

```toml
# ~/.codex/config.toml

# ==================== 基本設定 ====================
model = "gpt-4o"

[model_providers.openai]
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
wire_api = "chat"

# ==================== セキュリティ ====================
[sandbox]
default_mode = "read-only"

[approval]
policy = "on-request"

# ==================== MCP サーバー ====================
# Codex 自身を MCP サーバーとして使用
[mcp_servers.codex-agent]
command = "codex"
args = ["mcp-server"]
env.RUST_LOG = "info"
env.CODEX_CONFIG_PATH = "~/.codex/config.toml"

# サブエージェント用の追加設定
[subagents]
enabled = true
max_parallel = 4
token_budget = 40000
inherit_model = true
# 🆕 MCP 経由で Codex ツールを使用
use_codex_mcp = true  # ← これを有効化
```

### ステップ 2: エージェント定義の更新

`.codex/agents/code-reviewer.yaml` を更新して Codex MCP ツールを追加：

```yaml
name: code-reviewer
version: "1.0.0"
description: "Multi-language code reviewer with Codex MCP integration"

# 🆕 Codex MCP ツールの追加
tools:
  mcp:
    # Codex 専用 MCP ツール（完全な Codex 機能）
    - codex_read_file       # ✅ Codex 経由でファイル読み取り
    - codex_grep            # ✅ Codex 経由で grep
    - codex_codebase_search # ✅ セマンティック検索
    # codex_apply_patch は書き込み権限が必要なため、レビューには含めない
    # codex_shell は危険なため含めない

capabilities:
  languages:
    - typescript
    - python
    - rust
    - csharp_unity

checks:
  - type_safety
  - security_vulnerabilities
  - performance_optimization
  - best_practices

token_budget: 40000
sandbox_mode: read-only
approval_policy: never  # レビューは自動承認
```

### ステップ 3: MCP サーバーの動作確認

```bash
# Codex MCP サーバーが起動するか確認
codex mcp-server

# 別のターミナルで
echo '{"jsonrpc":"2.0","method":"tools/list","id":1}' | nc localhost 9000
# 期待: Codex MCP ツール一覧が返ってくる
```

---

## 🧪 動作テスト

### テスト 1: 基本的なレビュー

```bash
# Codex MCP 統合でコードレビュー
codex delegate code-reviewer --scope ./src

# 期待される動作:
# 1. Codex MCP サーバーが自動起動
# 2. code-reviewer が codex_read_file を呼び出してファイル読み取り
# 3. code-reviewer が codex_grep でパターン検索
# 4. code-reviewer が codex_codebase_search でセマンティック検索
# 5. レビューレポート生成
```

### テスト 2: セマンティック検索

```bash
# Codex MCP 経由でセマンティック検索をテスト
codex exec "Use codex_codebase_search to find authentication code"

# 期待: 認証関連のコードが見つかる
```

### テスト 3: 並列実行

```bash
# 複数エージェントを並列実行（すべて Codex MCP 使用）
codex delegate-parallel code-reviewer,test-gen \
  --scopes ./src,./tests \
  --budgets 40000,30000

# 期待: 両エージェントが独立した Codex MCP セッションを持つ
```

---

## 🔒 セキュリティ設定

### ツール権限の階層化

```yaml
# Level 1: Safe (デフォルト許可)
tools:
  mcp:
    - codex_read_file       # ✅ 読み取りのみ
    - codex_grep            # ✅ 読み取りのみ
    - codex_codebase_search # ✅ 読み取りのみ

# Level 2: Moderate (明示的許可必要)
tools:
  mcp:
    - codex_apply_patch     # ⚠️ 書き込み可能
    - codex_write_file      # ⚠️ 書き込み可能

# Level 3: Dangerous (厳格な審査必要)
tools:
  mcp:
    - codex_shell           # 🔴 シェルコマンド実行（通常は許可しない）
```

### 監査ログの有効化

```toml
# ~/.codex/config.toml
[audit]
enabled = true
log_dir = "~/.codex/audit-logs"
include_mcp_calls = true  # 🆕 MCP 呼び出しをログ
include_tool_args = true
format = "json"
```

監査ログ例：

```json
{
  "timestamp": "2025-10-13T01:15:00Z",
  "agent_name": "code-reviewer",
  "event_type": "mcp_tool_call",
  "tool": "codex_read_file",
  "args": {
    "path": "src/main.rs"
  },
  "result": "success",
  "tokens_used": 150
}
```

---

## 🐛 トラブルシューティング

### 問題 1: MCP サーバーが起動しない

**症状**:
```
Error: Failed to spawn Codex MCP server
```

**解決策**:
```bash
# Codex CLI が正しくインストールされているか確認
which codex  # Linux/macOS
where codex  # Windows

# PATH が正しく設定されているか確認
echo $PATH | grep codex

# 手動で MCP サーバーを起動してテスト
codex mcp-server
```

### 問題 2: ツールが見つからない

**症状**:
```
Error: Tool 'codex_read_file' not found
```

**解決策**:
```bash
# Phase 1 が実装されているか確認
ls codex-rs/mcp-server/src/codex_tools.rs

# エージェント定義を確認
cat .codex/agents/code-reviewer.yaml | grep codex_read_file

# MCP サーバーのツール一覧を確認
codex mcp-server --list-tools
```

### 問題 3: 権限エラー

**症状**:
```
Error: Tool 'codex_shell' is not permitted for this agent
```

**解決策**:

これは正常な動作です。`codex_shell` は危険なツールなので、デフォルトで許可されていません。

もし必要な場合は、エージェント定義で明示的に許可：

```yaml
tools:
  mcp:
    - codex_shell  # ⚠️ 危険！本当に必要か確認
sandbox_mode: workspace-write  # 書き込み権限も必要
approval_policy: on-request    # 実行前に確認
```

### 問題 4: トークン予算超過

**症状**:
```
Error: Token budget exceeded (used: 42000, limit: 40000)
```

**解決策**:

```yaml
# エージェント定義でトークン予算を増やす
token_budget: 60000  # 40000 → 60000
```

または、タスクを小分けにする：

```bash
# タスクを分割
codex delegate code-reviewer --scope ./src/auth
codex delegate code-reviewer --scope ./src/api
```

---

## 📊 実装ステータス

### ✅ 完了 (Phase 1)

- Codex MCP Tools 定義
  - `codex_read_file`
  - `codex_grep`
  - `codex_codebase_search`
  - `codex_apply_patch`
  - `codex_shell`

### 🚧 実装中 (Phase 2)

- AgentRuntime に MCP Client 統合
- エージェント定義の更新
- 権限チェック統合

### 🔜 今後 (Phase 3+)

- 完全な権限チェック実装
- 監査ログ統合
- パフォーマンス最適化
- 並列実行サポート

---

## 🎯 次のステップ

### 開発者向け

Phase 2 の実装を進める：

```bash
cd codex-rs

# AgentRuntime の変更
vi core/src/agents/runtime.rs

# MCP Client 統合
cargo add codex-mcp-client

# ビルド & テスト
cargo build --release -p codex-cli
cargo test -p codex-core
```

### ユーザー向け

現在の実装で使える機能を試す：

```bash
# エージェント定義を更新
vi .codex/agents/code-reviewer.yaml

# 動作テスト
codex delegate code-reviewer --scope ./src

# フィードバック
# GitHub Issues にバグ報告や機能要望を投稿
```

---

## 📚 関連ドキュメント

- [Codex MCP化設計書](_docs/2025-10-11_CodexMCP化設計書.md) - 詳細な設計
- [SUBAGENTS_QUICKSTART.md](SUBAGENTS_QUICKSTART.md) - サブエージェント基本ガイド
- [INSTALL_SUBAGENTS.md](INSTALL_SUBAGENTS.md) - インストール手順
- [PROJECT_RULES.md](PROJECT_RULES.md) - プロジェクトルール

---

## 🎉 期待される効果

### Before (MCP 統合前)

```
❌ Private API 制限でツール実行不可
❌ プロンプトでツール説明のみ
❌ サブエージェントは LLM 呼び出ししかできない
```

### After (MCP 統合後)

```
✅ 標準 MCP プロトコルでツール実行
✅ Codex の全機能をサブエージェントで使用可能
✅ 権限ベースの安全な制御
✅ 監査ログで完全なトレーサビリティ
```

---

## 🔗 リンク

- [GitHub: zapabob/codex](https://github.com/zapabob/codex)
- [MCP Protocol Spec](https://modelcontextprotocol.io)
- [OpenAI/codex](https://github.com/openai/codex)

---

**Version**: 0.47.0-alpha.1  
**Status**: 🚧 Phase 2 実装中  
**Completion Target**: 2025-10-15  
**Maintained by**: zapabob

