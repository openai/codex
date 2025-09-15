# Context Command Implementation Instructions

## Overview
This document contains the implementation for adding the `/context` slash command to Codex, which will show detailed context window usage.

## Implementation Status
✅ SlashCommand enum variant added
✅ Command parsing implemented
✅ available_during_task() returns true
⚠️ Command handling needs to be added to chatwidget.rs
⚠️ History cell implementation needed

## Files Modified

### 1. `/workspace/repo/codex/codex-rs/tui/src/slash_command.rs`
**Status**: ✅ COMPLETED
- Added `Context` variant to SlashCommand enum
- Added description: "show detailed context window usage"
- Set available_during_task() to return true

### 2. `/workspace/repo/codex/codex-rs/tui/src/chatwidget.rs`
**Status**: ⚠️ NEEDS MANUAL UPDATE

#### Add Context command handler (after line 872):
```rust
            SlashCommand::Context => {
                self.add_context_output();
            }
```

#### Add new method (after add_status_output method, around line 1168):
```rust
    pub(crate) fn add_context_output(&mut self) {
        // Show detailed context window usage information
        let default_usage;
        let usage_ref = if let Some(ti) = &self.token_info {
            &ti.total_token_usage
        } else {
            default_usage = TokenUsage::default();
            &default_usage
        };
        
        // For now, delegate to status output with context focus
        // This will be expanded to show more detailed context information
        self.add_to_history(history_cell::new_context_output(
            &self.config,
            usage_ref,
            &self.conversation_id,
        ));
    }
```

### 3. `/workspace/repo/codex/codex-rs/tui/src/history_cell.rs`
**Status**: ⚠️ NEEDS IMPLEMENTATION

Add a new function to create context output (can be added after new_status_output):
```rust
pub(crate) fn new_context_output(
    config: &Config,
    usage: &TokenUsage,
    session_id: &Option<ConversationId>,
) -> PlainHistoryCell {
    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push("/context".magenta().into());

    // 📊 Context Window Usage
    lines.push(vec![padded_emoji("📊").into(), "Context Window Usage".bold()].into());
    
    // Show token usage details
    if usage.total_tokens > 0 {
        let percentage = (usage.total_tokens as f64 / 128000.0 * 100.0) as u32;
        lines.push(format!("  Total tokens used: {} ({:.1}% of context window)", 
            usage.total_tokens, percentage).into());
        lines.push(format!("  • Input tokens: {}", usage.input_tokens).into());
        lines.push(format!("  • Output tokens: {}", usage.output_tokens).into());
        
        if usage.cache_read_tokens > 0 {
            lines.push(format!("  • Cache read: {} tokens", usage.cache_read_tokens).into());
        }
        if usage.cache_write_tokens > 0 {
            lines.push(format!("  • Cache write: {} tokens", usage.cache_write_tokens).into());
        }
    } else {
        lines.push("  No tokens used yet in this session".dim().into());
    }
    
    // Show session info if available
    if let Some(id) = session_id {
        lines.push(Line::from(""));
        lines.push(vec![padded_emoji("🔗").into(), "Session".bold()].into());
        lines.push(format!("  ID: {}", id).dim().into());
    }
    
    // Show model context window size
    lines.push(Line::from(""));
    lines.push(vec![padded_emoji("🤖").into(), "Model Info".bold()].into());
    lines.push(format!("  Model: {}", config.model).into());
    lines.push("  Context window: 128,000 tokens".into());
    
    PlainHistoryCell { lines }
}
```

## Testing Instructions

1. Build the project:
   ```bash
   cd /workspace/repo/codex/codex-rs
   cargo build
   ```

2. Run Codex and test the command:
   ```bash
   cargo run
   # In the Codex interface, type: /context
   ```

3. Verify that:
   - The command is recognized
   - It shows context window usage information
   - It works during an active task

## Alternative Simple Implementation

If the full implementation is complex, you can start with a simpler version that reuses the status output:

In `chatwidget.rs`, for the add_context_output method:
```rust
    pub(crate) fn add_context_output(&mut self) {
        // Temporarily use status output with a context prefix
        self.add_to_history(history_cell::new_info_event(
            "/context - Context Window Usage".to_string(),
            Some(format!("Tokens used: {} / 128000", 
                self.token_info
                    .as_ref()
                    .map(|ti| ti.total_token_usage.total_tokens)
                    .unwrap_or(0)
            ))
        ));
        // Call regular status for now
        self.add_status_output();
    }
```

## Next Steps

1. Manually apply the changes to chatwidget.rs
2. Implement the history_cell function (or use the simple version)
3. Build and test
4. Enhance with more detailed context information as needed