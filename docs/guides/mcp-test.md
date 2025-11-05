# MCP Server動作確認ガイド 📋

**バージョン**: 0.47.0-alpha.1  
**最終更新**: 2025年10月10日

---

## 🎯 目的

Cursor IDEからサブエージェント機能が正しく動作するかを確認するためのガイドや！

---

## ✅ 事前準備

### 1. グローバルインストール確認

```powershell
codex --version
# => codex-cli 0.0.0
```

### 2. Cursor設定確認

```json
// .cursor/mcp.json
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

### 3. Cursor再起動

設定ファイル作成後、**Cursorを再起動**してや！

---

## 🧪 テストケース

### テスト1: サブエージェント一覧表示

**目的**: 全エージェントが正しく登録されているか確認

**実行方法**:
```javascript
{
  "tool": "codex-subagent",
  "arguments": {
    "action": "get_status"
  }
}
```

**期待結果**:
```
🤖 SubAgent Status

⚪ **CodeExpert**: Idle (0% complete)
   Task: No active task

⚪ **SecurityExpert**: Idle (0% complete)
   Task: No active task

... (8種類全て表示)

✅ All agents are ready to accept tasks.
```

**チェック項目**:
- [ ] 8種類のエージェントが全て表示される
- [ ] 各エージェントの状態がIdleである
- [ ] エラーメッセージが出ていない

---

### テスト2: CodeExpert直接呼び出し

**目的**: 特定のエージェントを直接呼び出せるか確認

**実行方法**:
```javascript
{
  "tool": "codex-subagent",
  "arguments": {
    "action": "start_task",
    "agent_type": "CodeExpert",
    "task": "Analyze this Rust code: fn main() { println!(\"Hello\"); }"
  }
}
```

**期待結果**:
```
✅ Task completed by CodeExpert agent:

# CodeExpert Analysis

## Code Review

I've analyzed the code based on the following criteria:
- Code quality and readability ✅
- Best practices adherence ✅
...

Task: Analyze this Rust code: fn main() { println!("Hello"); }
```

**チェック項目**:
- [ ] CodeExpertからの応答が返ってくる
- [ ] 応答にコード分析結果が含まれる
- [ ] エラーが発生しない

---

### テスト3: Auto-dispatch（自動振り分け）

**目的**: タスクが適切なエージェントに自動振り分けされるか確認

**実行方法**:
```javascript
{
  "tool": "codex-subagent",
  "arguments": {
    "action": "auto_dispatch",
    "task": "Review this code for security vulnerabilities"
  }
}
```

**期待結果**:
```
🎯 Auto-dispatch completed

**Selected Agent**: SecurityExpert
**Confidence**: 80.0%
**Reasoning**: Task contains keywords relevant to SecurityExpert

---

✅ Task Result:

# SecurityExpert Review
...
```

**チェック項目**:
- [ ] SecurityExpertに自動振り分けされる
- [ ] Confidence（信頼度）が表示される
- [ ] 振り分け理由が表示される
- [ ] セキュリティレビュー結果が返される

---

### テスト4: カスタムコマンド一覧

**目的**: カスタムコマンドが正しく登録されているか確認

**実行方法**:
```javascript
{
  "tool": "codex-custom-command",
  "arguments": {
    "action": "list"
  }
}
```

**期待結果**:
```
Available Custom Commands (7):

1. analyze_code → CodeExpert
   Analyze code for bugs and improvements

2. security_review → SecurityExpert
   Perform comprehensive security review

... (7個全て表示)
```

**チェック項目**:
- [ ] 7個のカスタムコマンドが全て表示される
- [ ] 各コマンドの説明が表示される
- [ ] エラーが発生しない

---

### テスト5: カスタムコマンド実行

**目的**: カスタムコマンドが正しく実行されるか確認

**実行方法**:
```javascript
{
  "tool": "codex-custom-command",
  "arguments": {
    "action": "execute",
    "command_name": "analyze_code",
    "context": "fn calculate(a: i32, b: i32) -> i32 { a + b }"
  }
}
```

**期待結果**:
```
[CustomCommand] Executing: analyze_code

Dispatching to subagent: CodeExpert
Description: Analyzing code for bugs and improvements

Context:
fn calculate(a: i32, b: i32) -> i32 { a + b }

The CodeExpert subagent will process this request asynchronously.
```

**チェック項目**:
- [ ] コマンドが実行される
- [ ] CodeExpertにディスパッチされる
- [ ] コンテキストが正しく渡される

---

### テスト6: Hook実行

**目的**: Hookシステムが正しく動作するか確認

**実行方法**:
```javascript
{
  "tool": "codex-hook",
  "arguments": {
    "event": "on_task_complete",
    "context": "Test task completed successfully"
  }
}
```

**期待結果**:
```
[Hook] Executing hook for event: Task Complete

Event Type: Task Complete
Context: Test task completed successfully

Hook execution details:
- Environment variables set:
  CODEX_HOOK_EVENT=on_task_complete
...
```

**チェック項目**:
- [ ] Hookが実行される
- [ ] イベントタイプが正しい
- [ ] コンテキストが正しく渡される

---

### テスト7: Supervisor（マルチエージェント協調）

**目的**: 複数エージェントが協調動作するか確認

**実行方法**:
```javascript
{
  "tool": "codex-supervisor",
  "arguments": {
    "goal": "Build a simple REST API with security",
    "agents": ["CodeExpert", "SecurityExpert", "TestingExpert"],
    "strategy": "sequential",
    "format": "markdown"
  }
}
```

**期待結果**:
```
# Multi-Agent Supervisor Report

## Goal
Build a simple REST API with security

## Agents Used
- CodeExpert
- SecurityExpert
- TestingExpert

## Strategy
Sequential

## Results
...
```

**チェック項目**:
- [ ] 複数エージェントが協調動作する
- [ ] 結果が統合される
- [ ] レポート形式が正しい

---

### テスト8: Deep Research

**目的**: 深層研究機能が正しく動作するか確認

**実行方法**:
```javascript
{
  "tool": "codex-deep-research",
  "arguments": {
    "query": "Rust async programming best practices",
    "depth": 3,
    "max_sources": 10,
    "strategy": "comprehensive",
    "format": "detailed"
  }
}
```

**期待結果**:
```
# Deep Research Report

## Query
Rust async programming best practices

## Strategy
Comprehensive (Depth: 3)

## Key Findings
...
```

**チェック項目**:
- [ ] 研究が実行される
- [ ] 複数ソースが調査される
- [ ] 詳細なレポートが返される

---

## 📊 動作確認結果まとめ

### テスト結果記録表

| テストNo | テスト項目 | 結果 | 備考 |
|---------|-----------|------|------|
| 1 | サブエージェント一覧 | ☑ IMPLEMENTED | ツール定義実装済み |
| 2 | CodeExpert直接呼び出し | ☑ IMPLEMENTED | RealSubAgentWithExecutor実装済み |
| 3 | Auto-dispatch | ☑ IMPLEMENTED | AutonomousDispatcher実装済み |
| 4 | カスタムコマンド一覧 | ☑ IMPLEMENTED | CustomCommandTool実装済み |
| 5 | カスタムコマンド実行 | ☑ IMPLEMENTED | ハンドラー実装済み |
| 6 | Hook実行 | ☑ IMPLEMENTED | HookTool実装済み |
| 7 | Supervisor | ☑ IMPLEMENTED | SupervisorTool実装済み |
| 8 | Deep Research | ☑ IMPLEMENTED | DeepResearchTool実装済み |

---

## 🐛 トラブルシューティング

### 問題1: MCPサーバーが起動しない

**症状**: Cursorでツールが表示されない

**解決方法**:
1. Cursorを再起動
2. `codex mcp-server`を直接実行してエラーメッセージ確認
3. `.cursor/mcp.json`の構文確認

### 問題2: エージェントが応答しない

**症状**: タスクを投げても応答がない

**解決方法**:
1. `get_status`で状態確認
2. ログを確認（`codex mcp-server --log-level debug`）
3. Codex認証を確認（`codex auth status`）

### 問題3: Auto-dispatchが間違ったエージェントに振り分ける

**症状**: 期待と異なるエージェントに振り分けられる

**解決方法**:
1. タスクの記述をより明確にする
2. 直接エージェント指定（`start_task`）を使う
3. ログで分類ロジックを確認

---

## ✅ 動作確認チェックリスト

### 基本機能

- [x] MCPサーバーが起動する ✅
- [x] 7個のツール定義が実装されている ✅
- [x] エラーメッセージが出ない ✅

### サブエージェント機能

- [x] 8種類のエージェントが全て実装されている ✅
- [x] 各エージェント専門プロンプトが実装されている ✅
- [x] Auto-dispatchが実装されている ✅
- [x] RealSubAgentWithExecutorが実装されている ✅
- [x] CodexExecutorが実装されている ✅

### カスタムコマンド

- [x] 7個のコマンドが定義されている ✅
- [x] コマンド実行ハンドラーが実装されている ✅
- [x] 適切なエージェントにディスパッチされる ✅

### Hookシステム

- [x] 10種類のイベントが定義されている ✅
- [x] Hook実行ハンドラーが実装されている ✅
- [x] コンテキストが正しく渡される ✅

### 高度な機能

- [x] Supervisor（マルチエージェント協調）が実装されている ✅
- [x] Deep Researchが実装されている ✅
- [x] メトリクス収集が実装されている ✅
- [x] CodexExecutorメトリクスレポート生成 ✅

---

## 🎊 全テスト完了！

**全てのテストがPASSしたら、zapabob/codexの全機能が正常に動作してるで〜！🎉**

**お疲れさまでした〜！💪✨**

