# Technical Architecture: Context Window Usage Breakdown Command

## Architecture Overview
Add a `/context` slash command following the existing slash command pattern in codex-rs. Leverages existing TokenUsage infrastructure and conversation history to provide detailed token breakdown without external dependencies.

## Components

### Component 1: Slash Command Extension
**Type:** UI Command
**File Path:** `/workspace/repo/codex/codex-rs/tui/src/slash_command.rs`
**Purpose:** Add Context variant to SlashCommand enum
**Implementation Details:**
- Add `Context` variant after line 22 in enum
- Add description in match statement: "show detailed context window usage breakdown"
- Set `available_during_task()` to return true

### Component 2: Context Analyzer
**Type:** Core Service  
**File Path:** `/workspace/repo/codex/codex-rs/core/src/context_analyzer.rs`
**Purpose:** Calculate token usage per component
**Implementation Details:**
- Simple character-based estimation (4 chars ≈ 1 token)
- Reuse existing conversation_history and config structures

#### Interfaces
```rust
pub struct ContextBreakdown {
    pub system_prompt: u64,
    pub user_instructions: u64,
    pub conversation: u64,
    pub tools: u64,
    pub environment: u64,
    pub total: u64,
    pub max_window: u64,
}

pub fn analyze_context(
    config: &Config,
    history: &ConversationHistory,
    mcp_mgr: &McpConnectionManager
) -> ContextBreakdown;

fn estimate_tokens(text: &str) -> u64;
```

### Component 3: UI Renderer
**Type:** UI Display
**File Path:** `/workspace/repo/codex/codex-rs/tui/src/history_cell.rs`
**Purpose:** Format and display context breakdown
**Implementation Details:**
- Add `new_context_output()` function similar to `new_status_output()`
- Display progress bar and percentages
- Show recommendations when >70% usage

## Implementation Files

| File Path | Change Type | Description |
|-----------|-------------|-------------|
| `/workspace/repo/codex/codex-rs/tui/src/slash_command.rs` | Modify | Add Context enum variant |
| `/workspace/repo/codex/codex-rs/core/src/context_analyzer.rs` | Create | Token analysis logic |
| `/workspace/repo/codex/codex-rs/tui/src/history_cell.rs` | Modify | Add context display function |
| `/workspace/repo/codex/codex-rs/tui/src/chatwidget.rs` | Modify | Handle Context command |
| `/workspace/repo/codex/codex-rs/core/src/lib.rs` | Modify | Export context_analyzer module |

## Technical Challenges & Solutions
1. **Token accuracy:** Use simple 4-char estimation for MVP, upgrade to tiktoken-rs later
2. **Performance:** Cache static components (system prompt, tools) between calls

## Deployment Strategy
1. Compile with cargo build
2. Test locally with existing conversations
3. Deploy via standard release process

### Rollback Procedure
1. Revert commit
2. Rebuild and redeploy