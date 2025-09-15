# 🎯 Context Window Usage Command - Integration Guide

## 🎉 What's New

We're excited to introduce the **`/context` command** - your new best friend for understanding exactly how your AI conversation is using the available context window! This powerful feature gives you unprecedented visibility into token consumption with a beautiful, detailed breakdown that helps you optimize your interactions with Codex.

Gone are the days of wondering "how much context do I have left?" - now you get:
- 📊 **Visual progress bars** showing your usage at a glance
- 🎨 **Color-coded indicators** that warn you when approaching limits
- 📈 **Component breakdowns** revealing what's consuming your tokens
- ⚡ **Real-time updates** as your conversation progresses

## ⚡ Quick Start

Get up and running in literally 10 seconds:

1. **Start Codex** as you normally would
2. **Type `/context`** at any point in your conversation
3. **Marvel** at the beautiful, detailed breakdown of your token usage!

That's it! No configuration needed, no setup required. The command is available immediately in all sessions.

## 🎨 For End Users

### Step 1: Access the Command
Simply type `/context` in the chat input and press Enter. The command works:
- ✅ During active conversations
- ✅ While tasks are running
- ✅ In new or existing sessions
- ✅ Even with empty conversations (shows 0 usage)

### Step 2: Understanding the Output

When you run `/context`, you'll see a gorgeous display like this:

```
/context

📊 Context Window Usage
  • Total: 45,250 / 128,000 (35%)
  [████████████░░░░░░░░░░░░░░░░] 35%

📝 Component Breakdown
  • Input: 35,000 (77%)
  • Output: 8,250 (18%)
  • Reasoning: 2,000 (4%)
  • Cached: 5,000

🔗 Session
  • ID: 0193a5c4-8e7b-7f2d-9abc-123456789def
```

### Step 3: Visual Indicators

The progress bar changes color based on your usage:
- 🟢 **Green** (0-49%): Plenty of room, code away!
- 🟡 **Yellow** (50-69%): Getting warm, consider using `/compact` soon
- 🟠 **Orange** (70-89%): High usage warning - time to optimize!
- 🔴 **Red** (90-100%): Critical - immediate action needed

### Step 4: Pro Usage Tips

- **Check regularly**: Run `/context` periodically during long sessions
- **Watch for warnings**: At 70% usage, you'll see automatic warnings
- **Compare with `/status`**: Use `/status` for basic info, `/context` for details
- **Monitor components**: See if input, output, or reasoning dominates your usage

## 🚀 For Developers

### Using the Context Analyzer Module

The new `context_analyzer` module provides powerful token estimation capabilities:

```rust
use codex_core::context_analyzer::{analyze_context, estimate_tokens, ContextBreakdown};
use codex_protocol::models::ResponseItem;

// Estimate tokens for any text
let token_count = estimate_tokens("Your text here");
println!("Estimated tokens: {}", token_count);

// Analyze full conversation context
let system_prompt = Some("You are a helpful assistant");
let conversation_history: Vec<ResponseItem> = vec![
    // Your conversation items
];
let tools_definition = Some("{ \"tools\": [...] }");

let breakdown = analyze_context(
    system_prompt,
    &conversation_history,
    tools_definition
);

println!("System prompt tokens: {}", breakdown.system_prompt);
println!("Conversation tokens: {}", breakdown.conversation);
println!("Tools tokens: {}", breakdown.tools);
println!("Total tokens: {}", breakdown.total());
```

### Implementing Custom Token Display

Create beautiful token displays using the history cell module:

```rust
use codex_core::protocol::TokenUsage;
use crate::history_cell;

// Create token usage data
let usage = TokenUsage {
    input_tokens: 50000,
    output_tokens: 10000,
    total_tokens: 60000,
    cached_input_tokens: 5000,
    reasoning_output_tokens: 2000,
};

// Generate context output display
let context_display = history_cell::new_context_output(
    &config,
    &usage,
    &session_id
);

// Add to conversation history
self.add_to_history(context_display);
```

### Progress Bar Rendering

The feature includes a sophisticated progress bar renderer:

```rust
use crate::history_cell::render_progress_bar;

// Render a progress bar with automatic coloring
let progress_line = render_progress_bar(
    percentage,  // 0-100
    width,       // Bar width in characters
    Some(format!("{}/{}", current, total))  // Optional label
);

// The bar automatically applies colors:
// - Green: 0-49%
// - Yellow: 50-69%
// - Orange: 70-89%
// - Red: 90-100%
```

### Integrating with Slash Commands

Add the context command to your slash command handler:

```rust
match command {
    SlashCommand::Context => {
        self.add_context_output();
    }
    // ... other commands
}
```

### Token Estimation Algorithm

The token estimator uses a sophisticated hybrid approach:

```rust
pub fn estimate_tokens(text: &str) -> usize {
    let char_count = text.chars().count();
    let word_count = text.split_whitespace().count();
    
    // Character-based: ~4 chars per token
    let char_estimate = char_count / 4;
    
    // Word-based: ~1.33 tokens per word
    let word_estimate = (word_count as f64 * 1.33) as usize;
    
    // Average both estimates for accuracy
    (char_estimate + word_estimate) / 2
}
```

## ⚙️ Configuration

### Default Settings

The `/context` command works out of the box with these defaults:
- **Context window**: 128,000 tokens (Claude's default)
- **Warning threshold**: 70% usage
- **Progress bar width**: 30 characters
- **Color coding**: Automatic based on percentage

### Available During Tasks

Unlike some commands, `/context` is specifically configured to be available while tasks are running:

```rust
pub fn available_during_task(self) -> bool {
    match self {
        SlashCommand::Context => true,  // Always available!
        // ... other commands
    }
}
```

### Component Breakdown

The command tracks these token categories:
- **Input tokens**: User messages and system prompts
- **Output tokens**: Assistant responses
- **Reasoning tokens**: Internal reasoning (when applicable)
- **Cached tokens**: Previously seen content (reduces cost)

## 🧪 Test Drive

### Manual Testing

1. **Empty session test**:
   ```
   codex
   /context
   # Should show 0 tokens used
   ```

2. **Active conversation test**:
   ```
   codex
   > Tell me about Rust
   [Assistant responds...]
   /context
   # Should show actual usage with breakdown
   ```

3. **High usage simulation**:
   ```
   codex
   > [Paste a very long document]
   /context
   # Should show high percentage with color warning
   ```

### Automated Testing

The implementation includes comprehensive test coverage:

```rust
#[test]
fn test_context_command_shows_token_usage() {
    let mut chat = create_test_chatwidget();
    
    // Set up token usage
    chat.token_info = Some(TokenUsageInfo {
        total_token_usage: TokenUsage {
            input_tokens: 50000,
            output_tokens: 10000,
            total_tokens: 60000,
            // ...
        }
    });
    
    // Dispatch context command
    chat.dispatch_command(SlashCommand::Context);
    
    // Verify output
    let output = get_last_history_cell(&chat);
    assert!(output.contains("60,000"));
    assert!(output.contains("46%"));
}
```

### Validation Checklist

✅ Command appears in slash command menu  
✅ Progress bar renders correctly at all percentages  
✅ Colors change at threshold boundaries  
✅ Component breakdown adds up to total  
✅ Works during active tasks  
✅ Handles edge cases (0%, 100%, over capacity)  

## 💡 Pro Tips

### Optimize Your Context Usage

1. **Monitor regularly**: Check `/context` every 10-15 messages in long conversations
2. **Use `/compact`**: When hitting 70%, use `/compact` to summarize and free up space
3. **Clear unnecessary context**: Start new sessions with `/new` when changing topics
4. **Watch cached tokens**: Higher cache ratios mean lower costs!

### Power User Tricks

- **Combine commands**: Use `/status` for quick checks, `/context` for deep dives
- **Track patterns**: Notice which operations consume the most tokens
- **Optimize prompts**: Shorter, clearer prompts leave more room for responses
- **Leverage caching**: Repeated content gets cached, reducing effective usage

### Understanding the Numbers

- **Input tokens** include:
  - Your messages
  - System instructions
  - File contents you share
  - Previous conversation context

- **Output tokens** include:
  - Assistant responses
  - Generated code
  - Explanations and analysis

- **Reasoning tokens** (when applicable):
  - Internal chain-of-thought processing
  - Usually hidden but counted

## 🆘 Need Help?

### Common Questions

**Q: Why does my percentage seem high even with few messages?**  
A: System prompts, tools definitions, and conversation history all consume tokens. The context includes more than just visible messages.

**Q: What's the difference between `/status` and `/context`?**  
A: `/status` shows basic session info and total usage. `/context` provides detailed breakdown with visual progress bars and component analysis.

**Q: Can I increase the context window size?**  
A: The context window is model-specific (128k for Claude). You can't increase it, but you can optimize usage with `/compact`.

**Q: Why are cached tokens shown separately?**  
A: Cached tokens reduce API costs but still count toward context limits. Seeing them helps you understand cost vs. capacity.

### Troubleshooting

| Issue | Solution |
|-------|----------|
| Command not recognized | Update to the latest Codex version |
| No token data shown | Make at least one interaction first |
| Percentage seems wrong | Remember: includes system prompts and tools |
| Progress bar not colored | Check terminal color support |

### Edge Cases

The implementation gracefully handles:
- **Empty sessions**: Shows 0% usage
- **No session ID**: Omits session section
- **Over capacity**: Caps at 100% display
- **Missing token data**: Shows sensible defaults

---
*Powered by [Kanpredict](https://kanpredict.com) - Premium AI Development Platform*