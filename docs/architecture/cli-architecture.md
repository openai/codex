# 🎯 Codex CLI-First Architecture

**Design Philosophy**: All model selection and configuration flows through Codex CLI  
**Date**: 2025-10-13  
**Version**: 0.47.0-alpha.1

---

## 🌟 Core Concept

**Single Source of Truth**: Codex CLI の `--model` オプションが唯一のモデル選択方法

```
User → Codex CLI (--model) → All Features
                │
                ├─→ Direct Execution
                ├─→ MCP Server (for Cursor IDE)
                ├─→ Subagents (via MCP)
                └─→ Deep Research (via MCP)
```

---

## ✅ Design Principles

### 1. CLI-First
- **モデルは CLI 実行時に指定**
- 設定ファイルでモデルを固定**しない**
- 柔軟性と明示性を重視

### 2. MCP-Based Integration
- Codex 自身を MCP サーバーとして登録
- Subagents, Deep Research, Custom Commands 全てを MCP 経由で実現
- 統一されたインターフェース

### 3. No Model Hardcoding
- `config.toml` にモデル指定なし（オプション）
- `mcp.json` にモデル環境変数なし
- 全てランタイムで決定

---

## 📝 Configuration Files

### config.toml (Minimal Configuration)

```toml
# Codex Configuration
# サブエージェント用の設定

# モデル設定
# 注意: モデルは CLI 実行時に --model オプションで指定してください
# デフォルト値を設定したい場合のみ、以下のコメントを外してください
# model = "gpt-4o"  # 例: デフォルトモデル
model_reasoning_summary = "detailed"
windows_wsl_setup_acknowledged = true

# OpenAI Provider設定
[model_providers.openai]
base_url = "https://api.openai.com/v1"
env_key = "OPENAI_API_KEY"
name = "OpenAI (Chat Completions API)"
requires_openai_auth = true
wire_api = "chat"

# MCP Servers Configuration
[mcp_servers.codex-agent]
args = ["mcp-server"]
command = "codex"
env.CODEX_CONFIG_PATH = "C:\\Users\\downl\\.codex\\config.toml"
env.RUST_LOG = "info"
```

**Key Points**:
- ✅ No `model` field (commented out)
- ✅ MCP server configured
- ✅ Simple and flexible

---

### mcp.json (Cursor IDE Integration)

```json
{
  "mcpServers": {
    "codex": {
      "command": "codex",
      "args": ["mcp-server"],
      "env": {
        "RUST_LOG": "info",
        "CODEX_CONFIG_PATH": "C:\\Users\\downl\\.codex\\config.toml"
      },
      "description": "Codex MCP Server - All features via CLI"
    }
  }
}
```

**Key Points**:
- ✅ No `CODEX_MODEL` env var
- ✅ Inherits model from Codex CLI context
- ✅ Clean and minimal

---

## 🚀 Usage Patterns

### Pattern 1: Direct CLI Execution

```bash
# Basic execution with model selection
codex --model gpt-4o "Create a Rust function"

# Using different models for different tasks
codex --model gpt-4o-mini "Simple refactoring"
codex --model o1-preview "Complex algorithm design"

# Subagent via MCP (model inherited)
codex --model gpt-4o "Use codex-agent to analyze project"
```

**Flow**:
```
User specifies --model gpt-4o
  ↓
Codex CLI uses gpt-4o
  ↓
MCP server (if called) uses gpt-4o
  ↓
All subagents use gpt-4o
```

---

### Pattern 2: From Cursor IDE (Composer)

```
Use codex tool with prompt='Create a Rust function'
```

**Flow**:
```
Cursor Composer calls Codex MCP
  ↓
Codex MCP Server starts
  ↓
Uses model from runtime context
  ↓
(If no model specified, uses OpenAI default or errors)
```

**Important**: Cursor IDE での使用時は、Codex CLI の設定を継承**しない**ため、`mcp.json` で明示的に設定が必要な場合がある（検討中）

---

### Pattern 3: Subagents and Deep Research

#### From CLI
```bash
# Subagent execution
codex --model gpt-4o "Use codex-agent to review code"

# Deep Research
codex --model gpt-4o "Research Rust async best practices"

# Supervisor (parallel agents)
codex --model gpt-4o "Use codex-supervisor for security and testing"
```

#### From Cursor IDE
```
Use codex tool with prompt='Use codex-agent to review code'
```

**All subagents and research tools inherit the model from parent Codex instance** ✅

---

## 🎯 Benefits

### 1. Flexibility 🔄
- 毎回異なるモデルを使用可能
- タスクに最適なモデルを選択

### 2. Transparency 👁️
- モデル選択が明示的
- 隠れた設定なし

### 3. Simplicity 🎨
- 設定ファイルがシンプル
- モデル管理が一元化

### 4. Consistency ✅
- CLI と MCP で同じ動作
- 予測可能な挙動

---

## 📊 Architecture Diagram

```
┌─────────────────────────────────────────────────────────┐
│                  User Interface Layer                   │
├─────────────────────────────────────────────────────────┤
│  Terminal                     │  Cursor IDE Composer    │
│  └─ codex --model gpt-4o ...  │  └─ Use codex tool ...  │
└────────────┬────────────────────┴─────────────┬─────────┘
             │                                   │
             ▼                                   ▼
┌────────────────────────┐         ┌────────────────────────┐
│   Codex CLI (Main)     │◄────────┤  Codex MCP Server      │
│   - Model: gpt-4o      │   MCP   │  - Inherits model      │
│   - Handles requests   │ Protocol│  - Spawns subagents    │
└────────┬───────────────┘         └────────────────────────┘
         │
         │ Spawns subagents via MCP
         │
    ┌────┴────────────────┐
    │                     │
    ▼                     ▼
┌──────────┐         ┌──────────┐
│ Codex    │         │ Codex    │
│ Subagent │         │ Subagent │
│ (Model:  │         │ (Model:  │
│  gpt-4o) │         │  gpt-4o) │
└──────────┘         └──────────┘
```

**Key Points**:
- Model flows from top to bottom
- All instances share the same model
- MCP protocol enables recursive spawning

---

## 🧪 Testing

### Test 1: Basic Execution
```bash
codex --model gpt-4o "Create a simple Rust function"
```
**Expected**: Uses gpt-4o, no errors

---

### Test 2: Model Switching
```bash
codex --model gpt-4o-mini "Quick task"
codex --model o1-preview "Complex reasoning"
```
**Expected**: Each uses specified model

---

### Test 3: Subagent Inheritance
```bash
codex --model gpt-4o "Use codex-agent to list files"
```
**Expected**: Both main and subagent use gpt-4o

---

### Test 4: From Cursor IDE
```
Use codex tool with prompt='List all .rs files'
```
**Expected**: Uses runtime model (implementation-dependent)

---

## 🔧 Implementation Details

### CLI Entry Point
```rust
// cli/src/main.rs (conceptual)
let model = args.model.or(config.model).unwrap_or("gpt-4o");
codex_core::run(model, prompt, config)?;
```

### MCP Server
```rust
// mcp-server/src/main.rs (conceptual)
// Model is inherited from parent Codex context
let model = env::var("CODEX_MODEL")
    .or_else(|_| config.model)
    .unwrap_or_else(|_| "gpt-4o".to_string());
```

### Subagent Spawning
```rust
// core/src/agents/runtime.rs (conceptual)
pub async fn execute_agent(
    agent: &AgentDefinition,
    parent_model: &str, // Inherited from parent
    budgeter: &TokenBudgeter,
) -> Result<AgentResult> {
    // Spawn new Codex instance with same model
    spawn_codex_instance(parent_model, agent.task).await
}
```

---

## 📋 Migration Guide

### From Fixed Model to CLI-First

#### Step 1: Update config.toml
```diff
- model = "gpt-5-codex-medium"
+ # model = "gpt-4o"  # Optional default
```

#### Step 2: Update mcp.json
```diff
  "env": {
    "RUST_LOG": "info",
    "CODEX_CONFIG_PATH": "...",
-   "CODEX_MODEL": "gpt-5-codex-medium"
  }
```

#### Step 3: Update CLI Usage
```diff
- codex "Create function"
+ codex --model gpt-4o "Create function"
```

---

## 🎉 Summary

### Design Philosophy
**"Let the user decide, every time"**

### Key Features
- ✅ CLI-first model selection
- ✅ MCP-based subagent architecture
- ✅ No hardcoded models
- ✅ Maximum flexibility

### Benefits
- 🔄 Flexible model selection per task
- 👁️ Transparent and explicit
- 🎨 Simple configuration
- ✅ Consistent behavior

---

**Author**: zapabob  
**Date**: 2025-10-13  
**Status**: ✅ **Architecture Defined**  
**Next**: Implement and test with real tasks

