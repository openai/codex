# Cursor IDE Integration Guide - zapabob/codex

**バージョン**: 0.47.0-alpha.1  
**最終更新**: 2025年10月10日

---

## 🎯 概要

zapabob/codexをCursor IDEで使用するための完全なガイドや！  
MCPサーバー経由でサブエージェント、カスタムコマンド、Hookシステムが使えるで〜💪

---

## 📦 インストール

### 1. Codexをグローバルインストール

```bash
# 既にインストール済みの場合はスキップ
npm install -g @openai/codex

# または
.\global-install.ps1  # Windows PowerShell
```

### 2. 動作確認

```bash
codex --version
# => codex-cli 0.0.0

codex mcp-server --help
# => MCPサーバーのヘルプ表示
```

---

## ⚙️ Cursor設定

### 設定ファイル: `.cursor/mcp.json`

```json
{
  "mcpServers": {
    "codex": {
      "command": "codex",
      "args": ["mcp-server"],
      "env": {},
      "description": "Codex MCP Server with SubAgent, CustomCommand, and Hook support",
      "disabled": false
    }
  }
}
```

### Cursorの再起動

設定ファイルを作成したら、**Cursorを再起動**してや！

---

## 🛠️ 利用可能なツール（7個）

### 1. `codex` - 基本的なCodex呼び出し

```javascript
// 使用例
{
  "tool": "codex",
  "arguments": {
    "prompt": "Implement a binary search function in Rust"
  }
}
```

### 2. `codex-supervisor` - マルチエージェント協調

```javascript
{
  "tool": "codex-supervisor",
  "arguments": {
    "goal": "Build a REST API with authentication",
    "agents": ["CodeExpert", "SecurityExpert", "TestingExpert"],
    "strategy": "parallel",
    "format": "markdown"
  }
}
```

### 3. `codex-deep-research` - 深層研究

```javascript
{
  "tool": "codex-deep-research",
  "arguments": {
    "query": "Rust async runtime comparison",
    "depth": 5,
    "max_sources": 30,
    "strategy": "comprehensive",
    "format": "detailed"
  }
}
```

### 4. `codex-subagent` ⭐ - サブエージェント管理

```javascript
// タスクを開始
{
  "tool": "codex-subagent",
  "arguments": {
    "action": "start_task",
    "agent_type": "CodeExpert",
    "task": "Analyze this code for potential bugs"
  }
}

// 自動ディスパッチ（推奨）
{
  "tool": "codex-subagent",
  "arguments": {
    "action": "auto_dispatch",
    "task": "Review code for security vulnerabilities"
  }
}

// 受信トレイチェック
{
  "tool": "codex-subagent",
  "arguments": {
    "action": "check_inbox"
  }
}

// 状態確認
{
  "tool": "codex-subagent",
  "arguments": {
    "action": "get_status"
  }
}

// 思考プロセス確認
{
  "tool": "codex-subagent",
  "arguments": {
    "action": "get_thinking"
  }
}

// トークンレポート
{
  "tool": "codex-subagent",
  "arguments": {
    "action": "get_token_report"
  }
}
```

### 5. `codex-custom-command` ⭐ - カスタムコマンド

```javascript
// コマンド一覧
{
  "tool": "codex-custom-command",
  "arguments": {
    "action": "list"
  }
}

// コマンド実行
{
  "tool": "codex-custom-command",
  "arguments": {
    "action": "execute",
    "command_name": "analyze_code",
    "context": "fn main() { unsafe { ... } }"
  }
}

// コマンド詳細
{
  "tool": "codex-custom-command",
  "arguments": {
    "action": "info",
    "command_name": "security_review"
  }
}
```

### 6. `codex-hook` ⭐ - Hookシステム

```javascript
{
  "tool": "codex-hook",
  "arguments": {
    "event": "on_task_complete",
    "context": "Task finished successfully"
  }
}
```

### 7. `codex-reply` - セッション返信

```javascript
{
  "tool": "codex-reply",
  "arguments": {
    "conversation_id": "...",
    "input": "Continue with the implementation"
  }
}
```

---

## 💡 使用例

### 例1: コード分析

```
You: このコードをanalze_codeコマンドで分析して

Cursor (codex-custom-commandツールを呼び出し):
{
  "tool": "codex-custom-command",
  "arguments": {
    "action": "execute",
    "command_name": "analyze_code",
    "context": "fn main() { ... }"
  }
}

Result:
[CustomCommand] Executing: analyze_code
Dispatching to subagent: CodeExpert
...
The CodeExpert subagent will process this request asynchronously.
```

### 例2: セキュリティレビュー

```
You: セキュリティレビューをしてほしい

Cursor (codex-custom-commandツールを呼び出し):
{
  "tool": "codex-custom-command",
  "arguments": {
    "action": "execute",
    "command_name": "security_review",
    "context": "let password = user_input;"
  }
}

Result:
[CustomCommand] Executing: security_review
Dispatching to subagent: SecurityExpert
...
```

### 例3: 自動ディスパッチ

```
You: データベースのパフォーマンスを最適化して

Cursor (codex-subagentツールを呼び出し):
{
  "tool": "codex-subagent",
  "arguments": {
    "action": "auto_dispatch",
    "task": "Optimize database query performance"
  }
}

Result:
Auto-dispatching task...
Based on keyword analysis, this will be dispatched to the most appropriate subagent.
→ PerformanceExpertに自動ディスパッチ
```

---

## 🔧 トラブルシューティング

### MCPサーバーが起動しない

```bash
# MCPサーバーを直接起動してテスト
codex mcp-server

# エラーログを確認
codex mcp-server --log-level debug
```

### Cursorでツールが表示されない

1. Cursorを再起動
2. `.cursor/mcp.json`の場所を確認（プロジェクトルート）
3. JSON構文エラーがないか確認

### サブエージェントが動作しない

```bash
# サブエージェント状態を確認
{
  "tool": "codex-subagent",
  "arguments": {
    "action": "get_status"
  }
}
```

---

## 📚 デフォルトカスタムコマンド

| コマンド名 | サブエージェント | 説明 |
|-----------|----------------|------|
| `analyze_code` | CodeExpert | コード分析・改善提案 |
| `security_review` | SecurityExpert | セキュリティレビュー |
| `generate_tests` | TestingExpert | テストスイート生成 |
| `deep_research` | DeepResearcher | 深層研究 |
| `debug_issue` | DebugExpert | デバッグ・修正 |
| `optimize_performance` | PerformanceExpert | パフォーマンス最適化 |
| `generate_docs` | DocsExpert | ドキュメント生成 |

---

## 🎯 推奨ワークフロー

### ステップ1: コマンド一覧を確認

```javascript
{
  "tool": "codex-custom-command",
  "arguments": {
    "action": "list"
  }
}
```

### ステップ2: コマンドを実行

```javascript
{
  "tool": "codex-custom-command",
  "arguments": {
    "action": "execute",
    "command_name": "analyze_code",
    "context": "your code here"
  }
}
```

### ステップ3: 結果を確認

```javascript
{
  "tool": "codex-subagent",
  "arguments": {
    "action": "check_inbox"
  }
}
```

---

## 🌟 zapabob/codexの独自機能（Cursor経由で使用可能）

### 1. 非同期サブエージェント（8種類）

- `codex-subagent`ツールで管理
- 非ブロッキング処理
- 受信トレイパターン

### 2. カスタムコマンド（7個）

- `codex-custom-command`ツールで実行
- ワンコマンドでサブエージェント呼び出し
- Pre/Post-hookサポート

### 3. Hookシステム（10イベント）

- `codex-hook`ツールで実行
- ライフサイクルイベント
- 自動通知

### 4. 深層研究

- `codex-deep-research`ツールで実行
- 多層リサーチ（1-10レベル）
- 3種類の戦略

### 5. マルチエージェント協調

- `codex-supervisor`ツールで実行
- 複数エージェント並行処理
- 結果の自動マージ

---

## 🎊 完成や〜！

**Cursor IDEでzapabob/codexの全機能が使えるようになったで〜！🎉**

### 利用可能なツール: 7個

✅ `codex` - 基本Codex  
✅ `codex-supervisor` - マルチエージェント  
✅ `codex-deep-research` - 深層研究  
✅ `codex-subagent` ⭐ - サブエージェント管理  
✅ `codex-custom-command` ⭐ - カスタムコマンド  
✅ `codex-hook` ⭐ - Hookシステム  
✅ `codex-reply` - セッション返信  

---

**Cursorでなんでもできるようになったで〜！💪✨**

