# 🔄 Dynamic Model Selection - Implementation Guide

**Goal**: CLI で選択したモデルを MCP サーバーとサブエージェントに動的に伝播  
**Date**: 2025-10-13  
**Status**: 📋 **Design & Implementation Plan**

---

## 🎯 目標

### 現在の挙動
```bash
# CLI実行
codex --model gpt-4o "task"
  ↓
# MCPサーバー起動時
# ❌ gpt-4o が伝わらない
# ❌ config.toml のデフォルトを使用
```

### 理想の挙動
```bash
# CLI実行
codex --model gpt-4o "task"
  ↓
# MCPサーバー起動時
# ✅ gpt-4o が自動的に伝播
# ✅ サブエージェントも gpt-4o を使用
```

---

## 🏗️ アーキテクチャ設計

### Method 1: 環境変数による伝播（推奨）

```rust
// cli/src/main.rs
async fn main() -> Result<()> {
    let args = Args::parse();
    let model = args.model.unwrap_or_else(|| "gpt-4o".to_string());
    
    // MCP サーバー起動時に環境変数を設定
    if args.subcommand == Some(SubCommand::McpServer) {
        env::set_var("CODEX_RUNTIME_MODEL", &model);
    }
    
    // 通常の実行
    run(model, args).await
}
```

```rust
// mcp-server/src/main.rs
async fn start_mcp_server() -> Result<()> {
    // 実行時の環境変数を優先
    let model = env::var("CODEX_RUNTIME_MODEL")
        .or_else(|_| config.model)
        .unwrap_or_else(|_| "gpt-4o".to_string());
    
    // このモデルを使ってサブエージェントを起動
    spawn_subagents(model).await
}
```

**優先順位**:
1. `CODEX_RUNTIME_MODEL` 環境変数（最優先）
2. `config.toml` の `model` 設定
3. デフォルト値 `"gpt-4o"`

---

### Method 2: MCP プロトコル拡張

```json
// MCP リクエストにモデル情報を含める
{
  "jsonrpc": "2.0",
  "method": "tools/call",
  "params": {
    "name": "codex",
    "arguments": {
      "prompt": "...",
      "model": "gpt-4o"  // ← 追加
    }
  }
}
```

```rust
// mcp-server/src/tools.rs
async fn handle_codex_tool(args: ToolArgs) -> Result<ToolResult> {
    let model = args.model
        .or_else(|| env::var("CODEX_RUNTIME_MODEL").ok())
        .or_else(|| config.model)
        .unwrap_or_else(|| "gpt-4o".to_string());
    
    execute_codex(model, args.prompt).await
}
```

---

## 📝 実装手順

### Phase 1: 環境変数サポート（即時実装可能）

#### Step 1: CLI で環境変数を設定

**File**: `codex-rs/cli/src/main.rs`

```rust
// Before
async fn main() -> Result<()> {
    let args = Args::parse();
    run(args).await
}

// After
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // モデルが指定されている場合、環境変数に設定
    if let Some(ref model) = args.model {
        env::set_var("CODEX_RUNTIME_MODEL", model);
    }
    
    run(args).await
}
```

---

#### Step 2: MCP サーバーで環境変数を読み取る

**File**: `codex-rs/mcp-server/src/main.rs`

```rust
// Before
let model = config.model.unwrap_or_else(|| "gpt-4o".to_string());

// After
let model = env::var("CODEX_RUNTIME_MODEL")
    .ok()
    .or(config.model)
    .unwrap_or_else(|| "gpt-4o".to_string());

info!("Using model: {}", model);
```

---

#### Step 3: サブエージェントに伝播

**File**: `codex-rs/core/src/agents/runtime.rs`

```rust
pub async fn execute_agent(
    agent: &AgentDefinition,
    config: &Config,
) -> Result<AgentResult> {
    // 環境変数からモデルを取得
    let model = env::var("CODEX_RUNTIME_MODEL")
        .ok()
        .or(config.model.clone())
        .unwrap_or_else(|| "gpt-4o".to_string());
    
    info!("Subagent using model: {}", model);
    
    // サブエージェントを起動（環境変数を継承）
    spawn_codex_with_model(model, agent).await
}
```

---

### Phase 2: MCP プロトコル拡張（将来的な改善）

#### Step 1: Tool Definition 更新

**File**: `codex-rs/mcp-server/src/tools.rs`

```rust
pub fn get_tools() -> Vec<Tool> {
    vec![
        Tool {
            name: "codex".to_string(),
            description: "Execute Codex with full capabilities".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "The task to execute"
                    },
                    "model": {
                        "type": "string",
                        "description": "Optional model to use (overrides default)",
                        "enum": ["gpt-4o", "gpt-4o-mini", "o1-preview", "o1-mini"]
                    }
                },
                "required": ["prompt"]
            }),
        },
        // ... other tools
    ]
}
```

---

#### Step 2: Tool Handler 実装

```rust
async fn handle_tool_call(
    tool_name: &str,
    arguments: serde_json::Value,
) -> Result<ToolResult> {
    match tool_name {
        "codex" => {
            let prompt = arguments["prompt"]
                .as_str()
                .ok_or_else(|| anyhow!("Missing prompt"))?;
            
            // モデルを優先順位で決定
            let model = arguments["model"]
                .as_str()
                .or_else(|| env::var("CODEX_RUNTIME_MODEL").ok().as_deref())
                .or_else(|| config.model.as_deref())
                .unwrap_or("gpt-4o");
            
            execute_codex(model, prompt).await
        },
        // ... other tools
    }
}
```

---

## 🧪 テストケース

### Test 1: CLI からのモデル伝播

```bash
# Terminal 1: CLI実行
codex --model gpt-4o-mini "Use codex-agent to list files"

# Expected:
# - Main Codex: gpt-4o-mini ✅
# - MCP Server: gpt-4o-mini ✅
# - Subagent: gpt-4o-mini ✅
```

**検証方法**:
```bash
# ログを確認
tail -f ~/.codex/logs/codex.log | grep "Using model"
```

---

### Test 2: デフォルトモデルのフォールバック

```bash
# モデル指定なし
codex "Use codex-agent to list files"

# Expected:
# - config.toml のデフォルト（gpt-4o）を使用 ✅
```

---

### Test 3: Cursor IDE からの使用

```
Use codex tool with prompt='List files' model='gpt-4o-mini'
```

**Expected**:
- Cursor → Codex MCP → gpt-4o-mini を使用 ✅

---

## 📊 優先順位の決定ロジック

```rust
fn resolve_model(
    args_model: Option<&str>,      // CLI --model フラグ
    env_model: Option<&str>,        // CODEX_RUNTIME_MODEL 環境変数
    config_model: Option<&str>,     // config.toml の model
    default: &str,                  // デフォルト値
) -> String {
    args_model
        .or(env_model)
        .or(config_model)
        .unwrap_or(default)
        .to_string()
}
```

**優先順位**:
1. **CLI引数** (`--model`): 最優先
2. **環境変数** (`CODEX_RUNTIME_MODEL`): CLI起動時に設定
3. **設定ファイル** (`config.toml`): デフォルト
4. **ハードコード** (`"gpt-4o"`): 最終フォールバック

---

## 🔧 現在の暫定的な解決策

### config.toml で統一

```toml
# デフォルトモデルを設定
model = "gpt-4o"
```

```bash
# 異なるモデルを使いたい場合は、config.toml を編集
# または --model で上書き
codex --model gpt-4o-mini "task"
```

**制限**: サブエージェントは常に `config.toml` のモデルを使用

---

## 📋 実装チェックリスト

### Phase 1: 環境変数サポート（推奨）

- [ ] CLI で `CODEX_RUNTIME_MODEL` を設定
  - **File**: `codex-rs/cli/src/main.rs`
  - **Effort**: 5分

- [ ] MCP サーバーで環境変数を読み取る
  - **File**: `codex-rs/mcp-server/src/main.rs`
  - **Effort**: 5分

- [ ] サブエージェントに伝播
  - **File**: `codex-rs/core/src/agents/runtime.rs`
  - **Effort**: 10分

- [ ] テストケース作成
  - **Effort**: 15分

- [ ] ドキュメント更新
  - **Effort**: 10分

**Total Effort**: 約45分

---

### Phase 2: MCP プロトコル拡張（将来）

- [ ] Tool Definition に `model` パラメータ追加
- [ ] Tool Handler でモデルをサポート
- [ ] Cursor IDE 連携テスト
- [ ] ドキュメント更新

**Total Effort**: 約2-3時間

---

## 🎯 推奨アプローチ

### 今すぐ実装: Phase 1

**理由**:
- ✅ シンプル（環境変数のみ）
- ✅ 既存コードへの影響が小さい
- ✅ すぐに実装可能（45分）
- ✅ CLI と MCP の両方で動作

**実装順序**:
1. `cli/src/main.rs` を修正（5分）
2. `mcp-server/src/main.rs` を修正（5分）
3. `core/src/agents/runtime.rs` を修正（10分）
4. テスト実行（15分）
5. ドキュメント更新（10分）

---

### 将来的に検討: Phase 2

**理由**:
- より柔軟な制御が可能
- Cursor IDE からのモデル指定がより明示的
- MCP プロトコルの標準的な拡張

---

## 💡 使用例（Phase 1 実装後）

### CLI から実行

```bash
# モデルを明示的に指定
codex --model gpt-4o "Use codex-agent to analyze project"

# → Main Codex: gpt-4o
# → Subagent: gpt-4o (自動伝播) ✅

# 異なるモデルで実行
codex --model o1-preview "Solve complex algorithm"

# → Main Codex: o1-preview
# → Subagent: o1-preview (自動伝播) ✅
```

---

### デフォルトモデル使用

```bash
# config.toml の model を使用
codex "Simple task"

# → Main Codex: gpt-4o (config.toml)
# → Subagent: gpt-4o (継承) ✅
```

---

## 🎉 まとめ

### 現在の状態
- ✅ `config.toml` にデフォルトモデル設定
- ⚠️ CLI の `--model` がサブエージェントに伝わらない

### Phase 1 実装後
- ✅ CLI の `--model` が環境変数経由で伝播
- ✅ サブエージェントも同じモデルを使用
- ✅ 完全な動的モデル選択が可能

### 実装コスト
- **Phase 1**: 45分
- **Phase 2**: 2-3時間（将来）

### 推奨アクション
**今すぐ Phase 1 を実装すべき！** 🚀

---

**Author**: zapabob  
**Date**: 2025-10-13  
**Status**: 📋 **Ready for Implementation**  
**Estimated Time**: 45 minutes

