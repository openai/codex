# Compact System V2 Design Document

## Executive Summary

Create a new compact implementation (`codex-rs/core/src/compact/`) that runs **parallel** to the existing `compact.rs` and `compact_remote.rs`. Users switch between old and new implementations via a **Feature flag** (`Feature::CompactV2`).

**Key Design:**
- Old compact (`compact.rs`, `compact_remote.rs`) remains unchanged
- New compact (`compact/`) is a parallel implementation with enhanced features
- `Feature::CompactV2` flag toggles which implementation is used
- New compact fully supports remote compact mode (reuses existing `compact_remote.rs`)

---

## Implementation Comparison

### Claude Code Strengths (to adopt)
| Feature | Benefit |
|---------|---------|
| Two-tier architecture | Micro-compact (fast, no API) → Full compact (LLM) |
| Boundary markers | Multi-round compression with clear history |
| Context restoration | Files (5 max, 50k tokens), todos, plan after compact |
| Structured prompt | XML tags `<analysis>` + `<summary>` for better summaries |
| Cheaper model option | Uses smaller model for summarization |
| Safety multiplier | 1.33x buffer on token estimates |
| ALREADY_COMPACTED set | Idempotent micro-compact (prevents re-compression) |
| Eligible tools filtering | Only compress specific tool types |

### Codex Strengths (to keep)
| Feature | Benefit |
|---------|---------|
| Inline + Remote dual mode | Flexibility for different providers |
| GhostSnapshots | Undo capability preserved |
| Exponential backoff | Robust error recovery |
| Context window overflow handling | Removes oldest items on overflow |
| Rollout persistence | Audit trail for debugging |
| Middle-truncation | Preserves head and tail of content |

---

## New Architecture

```
codex-rs/core/src/compact/
├── mod.rs                  # Module exports, CompactResult, CompactState
├── config.rs               # CompactConfig struct and defaults
├── dispatch.rs             # Entry point, feature flag dispatch
├── strategy.rs             # CompactStrategy enum and selection logic
├── threshold.rs            # Three-tier threshold management
├── token_counter.rs        # Token counting (precise + approximate)
├── micro_compact.rs        # Tier 1: Tool result compression (no API)
├── full_compact.rs         # Tier 2: LLM-based summarization
├── boundary.rs             # Boundary marker creation and detection
├── context_restore.rs      # Post-compact context restoration
├── message_filter.rs       # Message selection and filtering predicates
├── prompt.rs               # Summarization prompt generation
├── summary.rs              # Summary message formatting
└── tests.rs                # Comprehensive tests
```

---

## Key Components

### 1. CompactState (mod.rs)

Session-level state for idempotent micro-compact:

```rust
/// Session-level compact state for idempotent operations
#[derive(Debug, Clone, Default)]
pub struct CompactState {
    /// Tool_use IDs that have already been compacted (prevents re-compression)
    /// Equivalent to Claude Code's DQ0 (ALREADY_COMPACTED) set
    pub compacted_tool_ids: HashSet<String>,

    /// Conversations that have been micro-compacted
    /// Equivalent to Claude Code's CQ0 set
    pub micro_compacted_conversations: HashSet<String>,

    /// Token count cache per tool_use_id (avoids recalculation)
    /// Equivalent to Claude Code's pI2 Map
    pub tool_token_cache: HashMap<String, i64>,

    /// Memory attachment tracking
    /// Equivalent to Claude Code's HQ0 set
    pub memory_attachments: HashSet<String>,
}

/// Read file state entry for context restoration
#[derive(Debug, Clone)]
pub struct ReadFileEntry {
    pub filename: String,
    /// Unix timestamp of last read (for sorting by recency)
    pub timestamp: i64,
    /// Cached token count (approximate)
    pub token_count: i64,
}

pub enum CompactResult {
    /// Compaction disabled or not triggered
    Skipped,
    /// Token count below threshold, no compaction needed
    NotNeeded,
    /// Micro-compact succeeded (fast, no API)
    MicroCompacted(MicroCompactResult),
    /// Full compact succeeded (LLM summarization)
    FullCompacted(CompactMetrics),
    /// Remote compact delegated to compact_remote.rs
    RemoteCompacted,
}
```

### 2. Micro-Compact (micro_compact.rs)

**Eligible Tools Set:**

```rust
/// Tools eligible for micro-compact compression
/// Based on Claude Code's pD5 set
pub static ELIGIBLE_TOOLS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        "shell",           // Bash command output
        "read",            // File content
        "grep",            // Search results
        "glob",            // File list
        "list_dir",        // Directory listing
        "web_fetch",       // Web content
        "web_search",      // Search results
        "task_output",     // Subagent output
    ])
});
```

**Algorithm (detailed):**

```rust
pub struct MicroCompactConfig {
    /// Minimum token savings required (default: 20,000)
    pub min_tokens_to_save: i64,
    /// Number of recent tool results to keep intact (default: 3)
    pub keep_last_n_tools: i32,
    /// Fixed token estimate per image (default: 2,000)
    pub tokens_per_image: i64,
}

pub struct MicroCompactResult {
    pub was_effective: bool,
    pub tools_compacted: i32,
    pub tokens_saved: i64,
    pub compacted_items: Vec<ResponseItem>,
}

/// Micro-compact algorithm (matching Claude Code's Si function)
pub fn try_micro_compact(
    messages: &[ResponseItem],
    compact_state: &mut CompactState,
    config: &MicroCompactConfig,
) -> Option<MicroCompactResult> {
    // Step 1: Scan for tool_use/tool_result pairs
    let mut tool_use_ids: Vec<String> = Vec::new();
    let mut tool_result_tokens: HashMap<String, i64> = HashMap::new();

    for msg in messages {
        if let Some(content) = get_message_content(msg) {
            for block in content {
                // Track eligible tool_use blocks (not already compacted)
                if block.type == "tool_use"
                   && ELIGIBLE_TOOLS.contains(block.name.as_str())
                   && !compact_state.compacted_tool_ids.contains(&block.id) {
                    tool_use_ids.push(block.id.clone());
                }
                // Calculate tokens for matching tool_result
                else if block.type == "tool_result"
                        && tool_use_ids.contains(&block.tool_use_id) {
                    let tokens = count_tool_result_tokens(block, config);
                    tool_result_tokens.insert(block.tool_use_id.clone(), tokens);
                }
            }
        }
    }

    // Step 2: Identify which to KEEP (last N - LRU-like behavior)
    let to_keep: HashSet<_> = tool_use_ids
        .iter()
        .rev()
        .take(config.keep_last_n_tools as usize)
        .cloned()
        .collect();

    // Step 3: Identify ALL non-kept tools to compress
    // (Simplified from Claude Code - compress all eligible, not conditional)
    let to_compress: HashSet<_> = tool_use_ids
        .iter()
        .filter(|id| !to_keep.contains(*id))
        .cloned()
        .collect();

    // Step 4: Calculate total savings
    let savings: i64 = to_compress
        .iter()
        .filter_map(|id| tool_result_tokens.get(id))
        .sum();

    // Step 5: Check if compression is worthwhile
    if savings < config.min_tokens_to_save {
        return None;  // Not enough savings, skip micro-compact
    }

    // Cache token counts for future runs
    for (id, tokens) in &tool_result_tokens {
        compact_state.tool_token_cache.insert(id.clone(), *tokens);
    }

    // Step 6: Replace old tool_result content
    let compacted_items = messages.iter().map(|msg| {
        replace_tool_results(msg, &to_compress, "[Old tool result content cleared]")
    }).collect();

    // Step 7: Track compacted IDs (idempotent - prevents re-compression)
    for id in &to_compress {
        compact_state.compacted_tool_ids.insert(id.clone());
    }

    Some(MicroCompactResult {
        was_effective: true,
        tools_compacted: to_compress.len() as i32,
        tokens_saved: savings,
        compacted_items,
    })
}
```

### 3. Boundary Markers (boundary.rs)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactBoundary {
    pub trigger: CompactTrigger,
    pub pre_compact_tokens: i64,
    pub timestamp: String,
    pub uuid: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CompactTrigger {
    Auto,
    Manual,
}

impl CompactBoundary {
    /// Create a boundary marker as ResponseItem
    /// Matching Claude Code's S91 function
    pub fn create(trigger: CompactTrigger, pre_tokens: i64) -> ResponseItem {
        ResponseItem::System {
            subtype: Some("compact_boundary".to_string()),
            content: "Conversation compacted".to_string(),
            compact_metadata: Some(CompactMetadata {
                trigger: match trigger {
                    CompactTrigger::Auto => "auto".to_string(),
                    CompactTrigger::Manual => "manual".to_string(),
                },
                pre_tokens,
            }),
            timestamp: chrono::Utc::now().to_rfc3339(),
            uuid: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Check if item is a boundary marker
    pub fn is_boundary_marker(item: &ResponseItem) -> bool {
        matches!(item, ResponseItem::System { subtype: Some(s), .. } if s == "compact_boundary")
    }

    /// Find index of last boundary marker (inclusive for extraction)
    pub fn find_last_boundary_index(items: &[ResponseItem]) -> Option<usize> {
        items.iter().rposition(|item| Self::is_boundary_marker(item))
    }

    /// Extract messages from last boundary onward (for re-summarization)
    /// First compaction returns all messages
    pub fn extract_messages_after_boundary(items: &[ResponseItem]) -> Vec<ResponseItem> {
        match Self::find_last_boundary_index(items) {
            Some(idx) => items[idx..].to_vec(),  // Include boundary
            None => items.to_vec(),              // No boundary = first compact
        }
    }
}
```

### 4. Message Filtering (message_filter.rs)

**Attachment Reorganization (matching Claude Code's `WZ()`):**

```rust
/// Reorganize attachments to attach them to their related messages
/// This ensures context is preserved during filtering
/// Matching Claude Code's attachment reorganization step in WZ()
pub fn reorganize_attachments(messages: &[ResponseItem]) -> Vec<ResponseItem> {
    let mut result = Vec::with_capacity(messages.len());
    let mut pending_attachments: Vec<ResponseItem> = Vec::new();

    for msg in messages {
        match msg {
            // Collect file/todo/plan attachments
            ResponseItem::Attachment { .. } => {
                pending_attachments.push(msg.clone());
            }
            // Attach pending attachments to user messages
            ResponseItem::Message { role, .. } if role == "user" => {
                // Add pending attachments before user message
                result.extend(pending_attachments.drain(..));
                result.push(msg.clone());
            }
            other => {
                result.push(other.clone());
            }
        }
    }

    // Append any remaining attachments
    result.extend(pending_attachments);
    result
}
```

**Filtering Predicates (matching Claude Code):**

```rust
/// Check if message is a previous summary (from compact)
pub fn is_summary_message(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { content, .. } => {
            content.iter().any(|c| {
                if let ContentItem::InputText { text } = c {
                    text.starts_with(SUMMARY_PREFIX)
                } else {
                    false
                }
            })
        }
        _ => false,
    }
}

/// Check if assistant message contains only thinking blocks (no output)
/// Matching Claude Code's NQ0 / isThinkingOnlyBlock
pub fn is_thinking_only_block(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message { role, content, .. } if role == "assistant" => {
            content.iter().all(|c| matches!(c, ContentItem::Thinking { .. }))
        }
        _ => false,
    }
}

/// Check if message is a synthetic error placeholder
/// Matching Claude Code's wb3 / isSyntheticErrorMessage
pub fn is_synthetic_error_message(item: &ResponseItem) -> bool {
    match item {
        ResponseItem::Message {
            role,
            is_api_error_message: Some(true),
            model: Some(m),
            ..
        } if role == "assistant" && m == "<synthetic>" => true,
        _ => false,
    }
}

/// Filter messages for LLM summarization
/// Matching Claude Code's WZ / filterAndNormalizeMessages
pub fn filter_for_summarization(items: &[ResponseItem]) -> Vec<ResponseItem> {
    // Step 1: Reorganize attachments first
    let reorganized = reorganize_attachments(items);

    // Step 2: Filter out unwanted message types
    let filtered: Vec<_> = reorganized.iter()
        .filter(|item| {
            // Exclude progress messages
            if matches!(item, ResponseItem::Progress { .. }) { return false; }
            // Exclude system messages (except boundaries)
            if matches!(item, ResponseItem::System { subtype, .. }
                       if subtype.as_deref() != Some("compact_boundary")) {
                return false;
            }
            // Exclude synthetic errors
            if is_synthetic_error_message(item) { return false; }
            // Exclude thinking-only blocks
            if is_thinking_only_block(item) { return false; }
            true
        })
        .cloned()
        .collect();

    // Step 3: Merge consecutive user messages
    merge_consecutive_messages(filtered)
}

/// Merge consecutive user messages into single messages
/// Matching Claude Code's message merging in WZ()
fn merge_consecutive_messages(items: Vec<ResponseItem>) -> Vec<ResponseItem> {
    let mut result: Vec<ResponseItem> = Vec::new();

    for item in items {
        match (&item, result.last_mut()) {
            // Merge consecutive user messages
            (
                ResponseItem::Message { role: r1, content: c1, .. },
                Some(ResponseItem::Message { role: r2, content: c2, .. })
            ) if r1 == "user" && r2 == "user" => {
                c2.extend(c1.clone());
            }
            // Otherwise, add as new item
            _ => {
                result.push(item);
            }
        }
    }

    result
}
```

### 5. Summary Message (summary.rs)

```rust
pub const SUMMARY_PREFIX: &str =
    "This session is being continued from a previous conversation that ran out of context.";

/// Format summary content with optional continue instruction
/// Matching Claude Code's T91 / formatSummaryContent
pub fn format_summary_content(summary_text: &str, continue_without_asking: bool) -> String {
    let cleaned = cleanup_summary_tags(summary_text);
    let base = format!(
        "{}\nThe conversation is summarized below:\n{}",
        SUMMARY_PREFIX,
        cleaned
    );

    if continue_without_asking {
        format!(
            "{}\nPlease continue the conversation from where we left it off without asking the user any further questions. Continue with the last task that you were asked to work on.",
            base
        )
    } else {
        base
    }
}

/// Clean up XML tags from LLM response
/// Matching Claude Code's MD5 / cleanupSummaryTags
pub fn cleanup_summary_tags(raw: &str) -> String {
    let mut result = raw.to_string();

    // Transform <analysis>...</analysis> to "Analysis:\n..."
    if let Some(caps) = regex::Regex::new(r"<analysis>([\s\S]*?)</analysis>")
        .ok()
        .and_then(|re| re.captures(&result))
    {
        if let Some(content) = caps.get(1) {
            result = result.replace(
                caps.get(0).unwrap().as_str(),
                &format!("Analysis:\n{}", content.as_str().trim())
            );
        }
    }

    // Transform <summary>...</summary> to "Summary:\n..."
    if let Some(caps) = regex::Regex::new(r"<summary>([\s\S]*?)</summary>")
        .ok()
        .and_then(|re| re.captures(&result))
    {
        if let Some(content) = caps.get(1) {
            result = result.replace(
                caps.get(0).unwrap().as_str(),
                &format!("Summary:\n{}", content.as_str().trim())
            );
        }
    }

    // Collapse multiple newlines
    regex::Regex::new(r"\n\n+")
        .map(|re| re.replace_all(&result, "\n\n").to_string())
        .unwrap_or(result)
        .trim()
        .to_string()
}

/// Create summary message with proper flags
/// Matching Claude Code's summary message structure
pub fn create_summary_message(
    summary_text: &str,
    continue_without_asking: bool
) -> ResponseItem {
    ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: format_summary_content(summary_text, continue_without_asking),
        }],
        // Additional metadata for detection
        is_compact_summary: Some(true),
        is_visible_in_transcript_only: Some(true),
    }
}
```

### 6. Token Counting (token_counter.rs)

**Two modes: Precise vs Approximate**

```rust
pub struct TokenCounter {
    /// Safety multiplier (default: 1.33)
    pub safety_margin: f64,
    /// Approximate bytes per token (default: 4)
    pub bytes_per_token: i32,
    /// Fixed token estimate per image (default: 2,000)
    pub tokens_per_image: i64,
}

impl TokenCounter {
    /// Quick approximate count (for local decisions)
    /// Matching Claude Code's gG / approximateTokenCount
    pub fn approximate(&self, text: &str) -> i64 {
        (text.len() as i64 + self.bytes_per_token as i64 - 1) / self.bytes_per_token as i64
    }

    /// Approximate with safety margin (for threshold decisions)
    /// Matching Claude Code's EQ0 / countMessageTokensWithSafetyMargin
    pub fn with_safety_margin(&self, text: &str) -> i64 {
        ((self.approximate(text) as f64) * self.safety_margin).ceil() as i64
    }

    /// Count tokens in tool_result block
    /// Matching Claude Code's iI2 / countToolResultTokens
    pub fn count_tool_result(&self, result: &ToolResultContent) -> i64 {
        match result {
            ToolResultContent::Text(text) => self.approximate(text),
            ToolResultContent::Array(blocks) => {
                blocks.iter().map(|block| {
                    match block {
                        ContentBlock::Text { text } => self.approximate(text),
                        ContentBlock::Image { .. } => self.tokens_per_image,
                        other => self.approximate(&serde_json::to_string(other).unwrap_or_default()),
                    }
                }).sum()
            }
        }
    }

    /// Count all message tokens with safety margin
    pub fn count_messages_with_margin(&self, messages: &[ResponseItem]) -> i64 {
        let total: i64 = messages.iter().filter_map(|msg| {
            match msg {
                ResponseItem::Message { content, .. } => {
                    Some(content.iter().map(|c| {
                        match c {
                            ContentItem::InputText { text } | ContentItem::OutputText { text }
                                => self.approximate(text),
                            ContentItem::ToolResult { content, .. }
                                => self.count_tool_result(content),
                            ContentItem::InputImage { .. }
                                => self.tokens_per_image,
                            other => self.approximate(&serde_json::to_string(other).unwrap_or_default()),
                        }
                    }).sum::<i64>())
                }
                _ => None,
            }
        }).sum();

        ((total as f64) * self.safety_margin).ceil() as i64
    }
}
```

### 7. Threshold Management (threshold.rs)

```rust
pub struct ThresholdState {
    /// Percentage of context remaining (0-100)
    pub percent_remaining: i32,
    /// Token count exceeds warning level
    pub is_above_warning: bool,
    /// Token count exceeds error level
    pub is_above_error: bool,
    /// Token count exceeds auto-compact threshold
    pub is_above_auto_compact: bool,
}

/// Calculate all threshold states
/// Matching Claude Code's x1A / calculateThresholds
pub fn calculate_thresholds(
    used_tokens: i64,
    context_limit: i64,
    config: &CompactConfig,
) -> ThresholdState {
    // Calculate effective auto-compact threshold
    let auto_compact_threshold = get_auto_compact_threshold(context_limit, config);

    // Use auto-compact threshold as effective limit when enabled
    let effective_limit = if config.auto_compact_enabled {
        auto_compact_threshold
    } else {
        context_limit
    };

    let percent_remaining = if effective_limit > 0 {
        ((effective_limit - used_tokens).max(0) * 100 / effective_limit) as i32
    } else {
        0
    };

    let warning_level = context_limit - config.warning_threshold;
    let error_level = context_limit - config.warning_threshold;  // Same as warning

    ThresholdState {
        percent_remaining,
        is_above_warning: used_tokens >= warning_level,
        is_above_error: used_tokens >= error_level,
        is_above_auto_compact: config.auto_compact_enabled && used_tokens >= auto_compact_threshold,
    }
}

/// Calculate auto-compact threshold
/// Matching Claude Code's aI2 / getAutoCompactThreshold
pub fn get_auto_compact_threshold(context_limit: i64, config: &CompactConfig) -> i64 {
    // Default: context_limit - free_space_buffer
    let default_threshold = context_limit - config.free_space_buffer;

    // Check for explicit threshold override
    if let Some(explicit) = config.auto_compact_threshold {
        return explicit.min(default_threshold);
    }

    // Check for percentage override
    if let Some(pct) = config.auto_compact_pct_override {
        if pct > 0 && pct <= 100 {
            let custom = (context_limit * pct as i64) / 100;
            return custom.min(default_threshold);  // Never exceed default
        }
    }

    default_threshold
}
```

### 8. Context Restoration Helpers (context_restore.rs)

```rust
/// Check if a file is agent-related and should be excluded from restoration
/// Matching Claude Code's hD5() / isAgentFile
pub fn is_agent_file(filename: &str, agent_id: &str) -> bool {
    // Exclude plan files for this agent
    if filename.contains(".claude/plans/") {
        return true;
    }
    // Exclude agent state files
    if filename.contains(&format!(".claude/agents/{}", agent_id)) {
        return true;
    }
    // Exclude other agent-specific files
    if filename.ends_with(".agent.json") || filename.ends_with(".agent.toml") {
        return true;
    }
    false
}

/// Restore recently read files after compaction
/// Matching Claude Code's bD5() / restoreFileReads
pub async fn restore_file_reads(
    read_file_state: &[ReadFileEntry],
    config: &CompactConfig,
    agent_id: &str,
) -> Vec<FileAttachment> {
    // Sort by timestamp (most recent first)
    let mut entries: Vec<_> = read_file_state
        .iter()
        .filter(|e| !is_agent_file(&e.filename, agent_id))
        .collect();
    entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let mut result = Vec::new();
    let mut total_tokens: i64 = 0;

    for entry in entries.iter().take(config.restore_max_files as usize) {
        // Check per-file limit
        let file_tokens = entry.token_count.min(config.restore_tokens_per_file);

        // Check total budget
        if total_tokens + file_tokens > config.restore_total_file_budget {
            break;
        }

        // Read and truncate file content
        if let Ok(content) = read_file_truncated(&entry.filename, config.restore_tokens_per_file).await {
            result.push(FileAttachment {
                filename: entry.filename.clone(),
                content,
            });
            total_tokens += file_tokens;
        }
    }

    result
}

/// Restore todo list if exists
/// Matching Claude Code's fD5() / restoreTodoList
pub fn restore_todo_list(agent_id: &str) -> Option<TodoAttachment> {
    let todo_path = format!(".claude/todos/{}.json", agent_id);
    if std::path::Path::new(&todo_path).exists() {
        std::fs::read_to_string(&todo_path)
            .ok()
            .map(|content| TodoAttachment { content })
    } else {
        None
    }
}

/// Restore plan file if in plan mode
/// Matching Claude Code's XQ0() / restorePlanFile
pub fn restore_plan_file(agent_id: &str) -> Option<PlanAttachment> {
    // Check for active plan file
    let plan_glob = format!(".claude/plans/*-{}.md", agent_id);
    glob::glob(&plan_glob)
        .ok()
        .and_then(|mut paths| paths.next())
        .and_then(|path| path.ok())
        .and_then(|path| {
            std::fs::read_to_string(&path)
                .ok()
                .map(|content| PlanAttachment {
                    filename: path.to_string_lossy().to_string(),
                    content,
                })
        })
}
```

---

### 9. Message Reconstruction Order

After compaction, history is rebuilt in this **specific order**:

```
┌─────────────────────────────────────────────────────────────────┐
│                    POST-COMPACT MESSAGE ORDER                    │
├─────────────────────────────────────────────────────────────────┤
│  1. [Old messages before boundary - kept as reference]          │
│                          ↓                                       │
│  2. [Boundary Marker] ← S91() - marks compaction point          │
│                          ↓                                       │
│  3. [Summary Message] ← T91() - LLM-generated summary           │
│        └─ isCompactSummary: true                                │
│        └─ isVisibleInTranscriptOnly: true                       │
│        └─ Contains "continue without asking" for auto-compact   │
│                          ↓                                       │
│  4. [Restored File Reads] ← bD5() - up to 5 files, 50k total    │
│                          ↓                                       │
│  5. [Todo List] ← fD5() - current todos if any                  │
│                          ↓                                       │
│  6. [Plan File] ← XQ0() - current plan if in plan mode          │
│                          ↓                                       │
│  7. [GhostSnapshots] ← preserved for undo capability            │
│                          ↓                                       │
│  8. [New Messages] ← messages created after compaction trigger  │
└─────────────────────────────────────────────────────────────────┘
```

**Implementation:**

```rust
pub fn build_compacted_history(
    session: &Session,
    summary_text: &str,
    restored_context: &RestoredContext,
    boundary: ResponseItem,
    is_auto_compact: bool,
) -> Vec<ResponseItem> {
    let mut new_history = Vec::new();

    // 1. Initial context (system prompts, etc.)
    new_history.extend(session.build_initial_context());

    // 2. Boundary marker
    new_history.push(boundary);

    // 3. Summary message (with continue instruction for auto-compact)
    new_history.push(create_summary_message(summary_text, is_auto_compact));

    // 4. Restored files
    for file in &restored_context.files {
        new_history.push(file.to_response_item());
    }

    // 5. Restored todos
    if let Some(todos) = &restored_context.todos {
        new_history.push(todos.to_response_item());
    }

    // 6. Restored plan
    if let Some(plan) = &restored_context.plan {
        new_history.push(plan.to_response_item());
    }

    // 7. GhostSnapshots (preserved from original history)
    let ghost_snapshots: Vec<_> = session.clone_history().await
        .iter()
        .filter(|item| matches!(item, ResponseItem::GhostSnapshot { .. }))
        .cloned()
        .collect();
    new_history.extend(ghost_snapshots);

    new_history
}
```

---

## Comprehensive CompactConfig

```rust
/// Complete compact configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompactConfig {
    // ============ Enable/Disable Controls ============

    /// Master switch to enable/disable all compaction
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Enable auto-compact (triggers automatically when threshold exceeded)
    #[serde(default = "default_true")]
    pub auto_compact_enabled: bool,

    // ============ Trigger Thresholds ============

    /// Token threshold to trigger auto-compact (overrides model default)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_compact_threshold: Option<i64>,

    /// Override as percentage of context window (0-100)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_compact_pct_override: Option<i32>,

    /// Minimum tokens to keep free (default: 13,000)
    #[serde(default = "default_free_space_buffer")]
    pub free_space_buffer: i64,

    /// Warning threshold from limit (default: 20,000)
    #[serde(default = "default_warning_threshold")]
    pub warning_threshold: i64,

    // ============ Micro-Compact Settings ============

    /// Minimum token savings required (default: 20,000)
    #[serde(default = "default_min_tokens_to_save")]
    pub micro_compact_min_tokens_to_save: i64,

    /// Recent tool results to keep intact (default: 3)
    #[serde(default = "default_keep_last_n_tools")]
    pub micro_compact_keep_last_n_tools: i32,

    // ============ Summarization Model ============

    /// Optional cheaper model for summarization
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summarization_model: Option<String>,

    /// Provider ID for summarization model
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summarization_model_provider_id: Option<String>,

    // ============ Context Restoration ============

    /// Maximum files to restore (default: 5)
    #[serde(default = "default_max_files_restore")]
    pub restore_max_files: i32,

    /// Token limit per file (default: 5,000)
    #[serde(default = "default_tokens_per_file")]
    pub restore_tokens_per_file: i64,

    /// Total token budget for files (default: 50,000)
    #[serde(default = "default_total_file_budget")]
    pub restore_total_file_budget: i64,

    /// Restore todo list after compaction
    #[serde(default = "default_true")]
    pub restore_todos: bool,

    /// Restore plan file after compaction
    #[serde(default = "default_true")]
    pub restore_plan: bool,

    // ============ Token Counting ============

    /// Safety multiplier for estimates (default: 1.33)
    #[serde(default = "default_safety_multiplier")]
    pub token_safety_multiplier: f64,

    /// Bytes per token for quick estimates (default: 4)
    #[serde(default = "default_bytes_per_token")]
    pub approx_bytes_per_token: i32,

    /// Token estimate per image (default: 2,000)
    #[serde(default = "default_tokens_per_image")]
    pub tokens_per_image: i64,

    // ============ User Message Preservation ============

    /// Max tokens for preserved user messages (default: 20,000)
    #[serde(default = "default_user_message_max_tokens")]
    pub user_message_max_tokens: i64,
}

// Default functions
fn default_true() -> bool { true }
fn default_free_space_buffer() -> i64 { 13_000 }
fn default_warning_threshold() -> i64 { 20_000 }
fn default_min_tokens_to_save() -> i64 { 20_000 }
fn default_keep_last_n_tools() -> i32 { 3 }
fn default_max_files_restore() -> i32 { 5 }
fn default_tokens_per_file() -> i64 { 5_000 }
fn default_total_file_budget() -> i64 { 50_000 }
fn default_safety_multiplier() -> f64 { 1.33 }
fn default_bytes_per_token() -> i32 { 4 }
fn default_tokens_per_image() -> i64 { 2_000 }
fn default_user_message_max_tokens() -> i64 { 20_000 }
```

### Configuration Summary Table

| Category | Field | Default | Description |
|----------|-------|---------|-------------|
| **Enable** | `enabled` | `true` | Master switch |
| | `auto_compact_enabled` | `true` | Auto-trigger on threshold |
| **Thresholds** | `auto_compact_threshold` | `None` | Explicit token trigger |
| | `auto_compact_pct_override` | `None` | % of context trigger |
| | `free_space_buffer` | 13,000 | Min tokens to keep free |
| | `warning_threshold` | 20,000 | Warning before limit |
| **Micro** | `micro_compact_min_tokens_to_save` | 20,000 | Min savings required |
| | `micro_compact_keep_last_n_tools` | 3 | Recent tools to keep |
| **Model** | `summarization_model` | `None` | Alt model for summary |
| | `summarization_model_provider_id` | `None` | Alt model provider |
| **Restore** | `restore_max_files` | 5 | Files to restore |
| | `restore_tokens_per_file` | 5,000 | Per-file token limit |
| | `restore_total_file_budget` | 50,000 | Total file budget |
| | `restore_todos` | `true` | Restore todo list |
| | `restore_plan` | `true` | Restore plan file |
| **Tokens** | `token_safety_multiplier` | 1.33 | Estimate safety margin |
| | `approx_bytes_per_token` | 4 | Bytes per token |
| | `tokens_per_image` | 2,000 | Image token cost |
| **Messages** | `user_message_max_tokens` | 20,000 | User msg budget |

---

## Dispatch Flow

```rust
/// Main entry point for V2 auto-compact
pub async fn auto_compact_dispatch(
    session: Arc<Session>,
    turn_context: Arc<TurnContext>,
) -> CompactResult {
    let config = session.config().compact.clone();

    // 1. Check if disabled
    if !config.enabled || !config.auto_compact_enabled {
        return CompactResult::Skipped;
    }

    // 2. Check threshold
    let used_tokens = get_current_token_count(&session, &turn_context).await;
    let context_limit = turn_context.client.get_model_context_window();
    let threshold_state = calculate_thresholds(used_tokens, context_limit, &config);

    if !threshold_state.is_above_auto_compact {
        return CompactResult::NotNeeded;
    }

    // 3. Try micro-compact first (if Feature::MicroCompact enabled)
    if session.enabled(Feature::MicroCompact) {
        let compact_state = session.compact_state_mut();
        if let Some(result) = try_micro_compact(
            &session.clone_history().await,
            compact_state,
            &config.into(),
        ) {
            if result.was_effective {
                // Apply micro-compact result
                session.apply_micro_compact(result.compacted_items).await;
                return CompactResult::MicroCompacted(result);
            }
        }
    }

    // 4. Fall back to full compact or remote compact
    let provider = turn_context.client.get_provider();
    if should_use_remote_compact(&session, &provider) {
        // Reuse existing compact_remote.rs
        crate::compact_remote::run_remote_compact_task(session, turn_context).await;
        CompactResult::RemoteCompacted
    } else {
        // Full compact with V2 features
        run_full_compact_v2(session, turn_context, &config, true).await
    }
}

fn should_use_remote_compact(session: &Session, provider: &ModelProviderInfo) -> bool {
    provider.is_openai() && session.enabled(Feature::RemoteCompaction)
}
```

---

## Full Compact 8-Phase Flow

```rust
pub async fn run_full_compact_v2(
    session: Arc<Session>,
    turn_context: Arc<TurnContext>,
    config: &CompactConfig,
    is_auto_compact: bool,
) -> CompactResult {
    // Phase 1: Validation & Setup
    let history = session.clone_history().await;
    if history.is_empty() {
        return CompactResult::Skipped;
    }
    let pre_tokens = count_tokens_from_usage(&turn_context);

    // Phase 2: PreCompact hooks (placeholder for future)
    // let hook_result = run_pre_compact_hooks(&session).await?;

    // Phase 3: Generate summary via LLM
    let messages_to_summarize = CompactBoundary::extract_messages_after_boundary(&history);
    let filtered = filter_for_summarization(&messages_to_summarize);
    let prompt = generate_summarization_prompt(None);  // No custom instructions yet

    let summary_response = stream_summarization(
        &turn_context,
        &prompt,
        &filtered,
        config.summarization_model.as_deref(),
    ).await?;

    // Phase 4: Validate response
    let summary_text = extract_summary_text(&summary_response)?;
    if summary_text.is_empty() || summary_text.starts_with("API_ERROR:") {
        return CompactResult::Skipped;
    }

    // Phase 5: Restore context
    let restored_context = restore_context(&session, config).await;

    // Phase 6: SessionStart hooks (placeholder)
    // run_session_start_hooks(&session, "compact").await?;

    // Phase 7: Build new history
    let boundary = CompactBoundary::create(
        if is_auto_compact { CompactTrigger::Auto } else { CompactTrigger::Manual },
        pre_tokens,
    );
    let new_history = build_compacted_history(
        &session,
        &summary_text,
        &restored_context,
        boundary,
        is_auto_compact,
    );
    session.replace_history(new_history).await;

    // Phase 8: Telemetry & events
    let post_tokens = count_tokens_from_usage(&turn_context);
    let metrics = CompactMetrics {
        pre_compact_tokens: pre_tokens,
        post_compact_tokens: post_tokens,
        strategy_used: CompactStrategy::FullCompact,
        files_restored: restored_context.files.len() as i32,
        ..Default::default()
    };

    session.send_event(&turn_context, EventMsg::ContextCompacted(
        ContextCompactedEvent {}
    )).await;

    CompactResult::FullCompacted(metrics)
}
```

---

## Structured Summarization Prompt

### System Prompt (prompt.rs)

```rust
/// System prompt for summarization LLM call
/// Matching Claude Code's summarization system prompt
pub const SUMMARIZATION_SYSTEM_PROMPT: &str =
    "You are a helpful AI assistant tasked with summarizing conversations.";
```

### User Prompt Template

The prompt instructs the LLM to produce a 9-section summary:

```markdown
Your task is to create a detailed summary of the conversation so far...

Wrap your analysis in <analysis> tags, reviewing each message chronologically:
- User's explicit requests and intents
- Claude's approach to addressing requests
- Key decisions, technical concepts, code patterns
- Specific details (file names, code snippets, function signatures)
- Errors encountered and fixes applied
- User feedback (especially corrections)

Then provide your summary in <summary> tags with these sections:

1. **Primary Request and Intent** - User's explicit requests in detail
2. **Key Technical Concepts** - Technologies and frameworks discussed
3. **Files and Code Sections** - Files examined/modified with code snippets
4. **Errors and Fixes** - Problems encountered and solutions
5. **Problem Solving** - Troubleshooting documentation
6. **All User Messages** - Non-tool-result user messages (critical for intent)
7. **Pending Tasks** - Outstanding work items
8. **Current Work** - What was being worked on immediately before
9. **Optional Next Step** - Recommended continuation (with verbatim quotes)

IMPORTANT: Use verbatim quotes from the conversation to prevent task drift.
```

---

## Telemetry Events (telemetry.rs)

Telemetry events follow codex's `EventMsg` naming convention (`PascalCase` + `{Name}Event`):

```rust
/// Compact telemetry event - matches EventMsg naming pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactCompletedEvent {
    /// Token count before compaction
    pub pre_compact_tokens: i64,
    /// Token count after compaction
    pub post_compact_tokens: i64,
    /// Tokens used for summarization input
    pub compaction_input_tokens: i64,
    /// Tokens generated in summary
    pub compaction_output_tokens: i64,
    /// Number of files restored
    pub files_restored: i32,
    /// Duration in milliseconds
    pub duration_ms: i64,
    /// Compact strategy used
    pub strategy: String,  // "full" | "remote"
}

/// Micro-compact telemetry event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicroCompactCompletedEvent {
    /// Number of tool results compacted
    pub tools_compacted: i32,
    /// Tokens saved by compression
    pub tokens_saved: i64,
    /// Duration in milliseconds
    pub duration_ms: i64,
}

/// Compact failed telemetry event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactFailedEvent {
    /// Failure reason: "no_summary", "api_error", "prompt_too_long", "interrupted"
    pub reason: String,
    /// Optional error message
    pub error_message: Option<String>,
}

/// Post-compact threshold exceeded warning event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactThresholdExceededEvent {
    /// Token count after compaction
    pub post_compact_tokens: i64,
    /// Threshold that was exceeded
    pub threshold: i64,
}

/// Extension to EventMsg for compact telemetry
/// Add to protocol/src/protocol.rs EventMsg enum:
///
/// ```rust
/// // In EventMsg enum:
/// CompactCompleted(CompactCompletedEvent),
/// MicroCompactCompleted(MicroCompactCompletedEvent),
/// CompactFailed(CompactFailedEvent),
/// CompactThresholdExceeded(CompactThresholdExceededEvent),
/// ```

/// Log compact telemetry via session event system
pub async fn emit_compact_event(
    session: &Session,
    turn_context: &TurnContext,
    event: EventMsg,
) {
    session.send_event(turn_context, event).await;
}
```

### EventMsg Extensions

Add these variants to `protocol/src/protocol.rs`:

```rust
// In pub enum EventMsg { ... }

/// Full compact completed successfully
CompactCompleted(CompactCompletedEvent),

/// Micro-compact completed successfully
MicroCompactCompleted(MicroCompactCompletedEvent),

/// Compact operation failed
CompactFailed(CompactFailedEvent),

/// Post-compact token count still above threshold (warning)
CompactThresholdExceeded(CompactThresholdExceededEvent),
```

---

## Files to Create/Modify

### New Files (in `codex-rs/core/src/compact/`)

| File | Lines | Description |
|------|-------|-------------|
| `mod.rs` | ~120 | Module exports, CompactResult, CompactState, ReadFileEntry |
| `config.rs` | ~120 | CompactConfig struct and defaults |
| `dispatch.rs` | ~120 | Entry point, feature flag dispatch |
| `strategy.rs` | ~50 | CompactStrategy enum |
| `threshold.rs` | ~80 | ThresholdState calculation |
| `token_counter.rs` | ~120 | Token counting (precise + approximate) |
| `micro_compact.rs` | ~280 | Tier 1: Tool result compression with caching |
| `full_compact.rs` | ~300 | Tier 2: LLM-based summarization |
| `boundary.rs` | ~100 | Boundary marker creation/detection |
| `context_restore.rs` | ~200 | File/todo/plan restoration with is_agent_file |
| `message_filter.rs` | ~200 | Filtering predicates + reorganize + merge |
| `prompt.rs` | ~100 | Summarization prompt + system prompt |
| `summary.rs` | ~100 | Summary message formatting |
| `telemetry.rs` | ~80 | Telemetry events (tengu_*) |
| `tests.rs` | ~450 | Comprehensive unit tests |

**Total: ~2,420 lines** (estimated)

### Template Files

1. `templates/compact/structured_prompt.md` - 9-section summarization prompt

### Files to Modify

| File | Changes | Description |
|------|---------|-------------|
| `core/src/lib.rs` | +1 line | `pub mod compact;` |
| `core/src/features.rs` | +6 lines | `CompactV2` + `MicroCompact` flags |
| `core/src/config/mod.rs` | +20 lines | `compact: CompactConfig` field |
| `core/src/codex.rs` | +15 lines | Feature flag dispatch |
| `core/src/tasks/compact.rs` | +10 lines | Feature flag dispatch |
| `core/src/state/session.rs` | +10 lines | Add `CompactState` field |
| `protocol/src/protocol.rs` | +15 lines | Add 4 EventMsg variants + event structs |

### Files Unchanged (Legacy)

- `core/src/compact.rs` - Legacy inline compact
- `core/src/compact_remote.rs` - Legacy remote compact

---

## Implementation Order

### Phase 1: Foundation (~5 files)
1. `compact/mod.rs` - Module structure, CompactResult, CompactState
2. `compact/config.rs` - CompactConfig struct
3. `compact/token_counter.rs` - Token counting
4. `compact/threshold.rs` - ThresholdState calculation
5. `compact/strategy.rs` - CompactStrategy enum

### Phase 2: Message Handling (~3 files)
6. `compact/message_filter.rs` - Filtering predicates
7. `compact/boundary.rs` - Boundary markers
8. `compact/summary.rs` - Summary formatting

### Phase 3: Micro-Compact (~1 file)
9. `compact/micro_compact.rs` - Tool result compression

### Phase 4: Full Compact (~3 files)
10. `compact/prompt.rs` - Summarization prompt
11. `templates/compact/structured_prompt.md` - Prompt template
12. `compact/full_compact.rs` - LLM summarization

### Phase 5: Context Restoration (~1 file)
13. `compact/context_restore.rs` - File/todo/plan restoration

### Phase 6: Integration (~modifications)
14. `compact/dispatch.rs` - Entry point
15. Update `core/src/features.rs` - Feature flags
16. Update `core/src/config/mod.rs` - Config integration
17. Update `core/src/codex.rs` - Dispatch hook
18. Update `core/src/tasks/compact.rs` - Task dispatch
19. Update `core/src/state/session.rs` - CompactState field

### Phase 7: Tests
20. `compact/tests.rs` - Comprehensive tests

---

## Detailed Task List

### Task 1: Foundation Module Setup
**File:** `compact/mod.rs`
**Lines:** ~100

**Subtasks:**
1.1. Define `CompactState` struct with `compacted_tool_ids: HashSet<String>`
1.2. Define `CompactResult` enum with all variants
1.3. Define `CompactMetrics` struct
1.4. Define public module exports and re-exports

**Acceptance Criteria:**
- [ ] `CompactState` tracks compacted tool IDs for idempotency
- [ ] `CompactResult` covers all compact outcomes
- [ ] All public types accessible via `compact::*`
- [ ] Compiles without errors

---

### Task 2: CompactConfig Implementation
**File:** `compact/config.rs`
**Lines:** ~120

**Subtasks:**
2.1. Define `CompactConfig` struct with 18 fields
2.2. Implement `Default` trait with Claude Code values
2.3. Implement serde with `#[serde(default)]`
2.4. Add validation (e.g., pct_override 0-100)

**Acceptance Criteria:**
- [ ] All 18 fields implemented with correct defaults
- [ ] TOML parsing works with partial config
- [ ] Validation rejects invalid values
- [ ] Unit tests for serialization

---

### Task 3: Token Counter
**File:** `compact/token_counter.rs`
**Lines:** ~120

**Subtasks:**
3.1. Define `TokenCounter` struct
3.2. Implement `approximate(text) -> i64`
3.3. Implement `with_safety_margin(text) -> i64`
3.4. Implement `count_tool_result(result) -> i64`
3.5. Implement `count_messages_with_margin(messages) -> i64`

**Acceptance Criteria:**
- [ ] Estimates match expected values (1 token ≈ 4 bytes)
- [ ] Safety margin (1.33x) applied correctly
- [ ] Image tokens fixed at configured value
- [ ] Unit tests for all methods

---

### Task 4: Threshold Management
**File:** `compact/threshold.rs`
**Lines:** ~80

**Subtasks:**
4.1. Define `ThresholdState` struct
4.2. Implement `calculate_thresholds()`
4.3. Implement `get_auto_compact_threshold()`
4.4. Support explicit and percentage overrides

**Acceptance Criteria:**
- [ ] All 4 threshold states calculated correctly
- [ ] Percentage override capped at default
- [ ] Unit tests with various token counts

---

### Task 5: Message Filter
**File:** `compact/message_filter.rs`
**Lines:** ~150

**Subtasks:**
5.1. Implement `is_summary_message()`
5.2. Implement `is_thinking_only_block()`
5.3. Implement `is_synthetic_error_message()`
5.4. Implement `filter_for_summarization()`
5.5. Implement `collect_user_messages()`

**Acceptance Criteria:**
- [ ] All predicates correctly identify message types
- [ ] Filter excludes progress, system (except boundaries), synthetic errors
- [ ] Thinking-only blocks filtered
- [ ] Summary messages detected by prefix

---

### Task 6: Boundary Markers
**File:** `compact/boundary.rs`
**Lines:** ~100

**Subtasks:**
6.1. Define `CompactBoundary` struct
6.2. Define `CompactTrigger` enum
6.3. Implement `create()` -> ResponseItem
6.4. Implement `is_boundary_marker()`
6.5. Implement `find_last_boundary_index()`
6.6. Implement `extract_messages_after_boundary()`

**Acceptance Criteria:**
- [ ] Boundary markers created with correct metadata
- [ ] Detection finds correct marker in history
- [ ] Extraction includes boundary and all after
- [ ] First compact returns all messages

---

### Task 7: Summary Message
**File:** `compact/summary.rs`
**Lines:** ~100

**Subtasks:**
7.1. Define `SUMMARY_PREFIX` constant
7.2. Implement `format_summary_content()`
7.3. Implement `cleanup_summary_tags()`
7.4. Implement `create_summary_message()`

**Acceptance Criteria:**
- [ ] Summary has correct prefix
- [ ] Auto-compact includes continue instruction
- [ ] XML tags converted to plain headers
- [ ] Message has `is_compact_summary` flag

---

### Task 8: Micro-Compact Implementation
**File:** `compact/micro_compact.rs`
**Lines:** ~250

**Subtasks:**
8.1. Define `ELIGIBLE_TOOLS` set
8.2. Define `MicroCompactConfig` struct
8.3. Define `MicroCompactResult` struct
8.4. Implement tool_use/tool_result scanning
8.5. Implement LRU-like keeping (last N)
8.6. Implement token savings calculation
8.7. Implement content replacement
8.8. Implement idempotent tracking via CompactState

**Algorithm:**
```
1. Scan for tool_use/tool_result pairs (eligible tools only)
2. Skip already-compacted IDs
3. Calculate tokens per tool_result
4. Keep last N results
5. Sum potential savings from older results
6. If savings >= min_threshold:
   a. Replace old content with placeholder
   b. Track compacted IDs in CompactState
   c. Return MicroCompactResult
7. Else return None
```

**Acceptance Criteria:**
- [ ] Only eligible tools considered
- [ ] Last N tool results preserved
- [ ] Older results compressed to placeholder
- [ ] Token savings calculated correctly
- [ ] Minimum threshold respected
- [ ] Idempotent (compacted IDs tracked)
- [ ] Unit tests with sample tool results

---

### Task 9: Structured Prompt
**Files:** `compact/prompt.rs`, `templates/compact/structured_prompt.md`
**Lines:** ~80 + template

**Subtasks:**
9.1. Create template with 9 sections
9.2. Include `<analysis>` and `<summary>` instructions
9.3. Define `SummarizationPrompt` struct
9.4. Implement `generate() -> String`
9.5. Support custom instructions parameter

**9 Required Sections:**
1. Primary request and intent
2. Key technical concepts
3. Files and code sections
4. Errors and fixes
5. Problem solving documentation
6. All user messages
7. Pending tasks
8. Current work
9. Optional next step

**Acceptance Criteria:**
- [ ] Template includes all 9 sections
- [ ] XML tag instructions included
- [ ] Template loaded via `include_str!`
- [ ] Custom instructions appended correctly

---

### Task 10: Full Compact Implementation
**File:** `compact/full_compact.rs`
**Lines:** ~300

**Subtasks:**
10.1. Implement 8-phase flow
10.2. Support optional summarization model
10.3. Stream summarization response
10.4. Build history with correct order
10.5. Preserve GhostSnapshots
10.6. Handle context overflow
10.7. Emit events

**Acceptance Criteria:**
- [ ] All 8 phases implemented
- [ ] Context restored correctly
- [ ] Boundary marker created
- [ ] GhostSnapshots preserved
- [ ] Overflow handled gracefully
- [ ] Model override works

---

### Task 11: Context Restoration
**File:** `compact/context_restore.rs`
**Lines:** ~150

**Subtasks:**
11.1. Define `RestoredContext` struct
11.2. Implement file restoration (sorted by access time)
11.3. Implement per-file and total budget limits
11.4. Implement todo restoration
11.5. Implement plan restoration

**Acceptance Criteria:**
- [ ] Files sorted by access time
- [ ] Budget limits respected
- [ ] Todos restored when file exists
- [ ] Plan restored when in plan mode

---

### Task 12: Dispatch Logic
**File:** `compact/dispatch.rs`
**Lines:** ~120

**Subtasks:**
12.1. Implement `auto_compact_dispatch()`
12.2. Check config enabled flags
12.3. Calculate and check thresholds
12.4. Try micro-compact first
12.5. Fall back to full/remote compact

**Acceptance Criteria:**
- [ ] Config checks work correctly
- [ ] Threshold calculation correct
- [ ] Micro-compact tried first when enabled
- [ ] Remote compact for OpenAI + RemoteCompaction
- [ ] Full compact otherwise

---

### Task 13: Feature Flags
**File:** `core/src/features.rs`
**Changes:** +6 lines

**Subtasks:**
13.1. Add `CompactV2` feature flag (default: disabled)
13.2. Add `MicroCompact` feature flag (default: disabled)
13.3. Add feature specs with descriptions

**Acceptance Criteria:**
- [ ] Both flags in Feature enum
- [ ] Default disabled for gradual rollout
- [ ] Can be enabled via config

---

### Task 14: Config Integration
**File:** `core/src/config/mod.rs`
**Changes:** +20 lines

**Subtasks:**
14.1. Add `compact: CompactConfig` to Config
14.2. Add TOML loading
14.3. Add default when not specified

**Acceptance Criteria:**
- [ ] CompactConfig accessible via `config.compact`
- [ ] Partial TOML works
- [ ] Full TOML works

---

### Task 15: Session State Integration
**File:** `core/src/state/session.rs`
**Changes:** +10 lines

**Subtasks:**
15.1. Add `compact_state: CompactState` field
15.2. Add accessor methods
15.3. Initialize in session creation

**Acceptance Criteria:**
- [ ] CompactState accessible from Session
- [ ] Initialized correctly
- [ ] Persists across turns

---

### Task 16: Integration Updates
**Files:** `core/src/codex.rs`, `core/src/tasks/compact.rs`
**Changes:** +25 lines

**Subtasks:**
16.1. Update auto_compact call with feature flag
16.2. Update CompactTask with feature flag
16.3. Keep legacy path when V2 disabled

**Acceptance Criteria:**
- [ ] Feature flag toggle works
- [ ] Legacy path unchanged when V2 disabled
- [ ] New path used when V2 enabled

---

### Task 17: Comprehensive Tests
**File:** `compact/tests.rs`
**Lines:** ~400

**Subtasks:**
17.1. Unit tests for TokenCounter
17.2. Unit tests for ThresholdState
17.3. Unit tests for MessageFilter predicates
17.4. Unit tests for BoundaryMarker
17.5. Unit tests for MicroCompact (with mock data)
17.6. Unit tests for Summary formatting
17.7. Integration tests for dispatch flow
17.8. Integration tests for V2 enabled/disabled

**Acceptance Criteria:**
- [ ] >80% code coverage
- [ ] All edge cases tested
- [ ] Mock LLM for integration tests
- [ ] `cargo test -p codex-core` passes

---

## Acceptance Criteria Summary

### Functional Requirements
- [ ] Feature flag `CompactV2` toggles between legacy and new
- [ ] Feature flag `MicroCompact` enables/disables micro-compact
- [ ] Micro-compact compresses old tool results (idempotent via CompactState)
- [ ] Full compact creates LLM summary with 9 sections
- [ ] Remote compact reuses `compact_remote.rs`
- [ ] Context restored (files, todos, plan)
- [ ] Boundary markers track multi-round compression
- [ ] All CompactConfig fields configurable via TOML
- [ ] Summary message has correct format and flags

### Non-Functional Requirements
- [ ] No changes to legacy `compact.rs` and `compact_remote.rs`
- [ ] All code follows CLAUDE.md conventions (i32, no unwrap)
- [ ] `cargo build` succeeds
- [ ] `cargo test -p codex-core` passes
- [ ] `just fmt` formatting applied

### Testing Requirements
- [ ] Unit tests for each module
- [ ] Integration tests for dispatch flow
- [ ] Tests for both V2 enabled/disabled paths
- [ ] Edge cases: empty history, overflow, no files

---

## Quick Reference

```
compact/
├── mod.rs              → Exports, CompactResult, CompactState, ReadFileEntry
├── config.rs           → CompactConfig (18 fields)
├── dispatch.rs         → Entry point (auto_compact_dispatch)
├── strategy.rs         → CompactStrategy enum
├── threshold.rs        → ThresholdState (4 booleans)
├── token_counter.rs    → Precise + approximate counting
├── micro_compact.rs    → Tier 1: Tool result compression
├── full_compact.rs     → Tier 2: LLM summarization
├── boundary.rs         → Boundary markers
├── context_restore.rs  → Restore files/todos/plan + is_agent_file
├── message_filter.rs   → Filtering + reorganize + merge
├── prompt.rs           → 9-section prompt + system prompt
├── summary.rs          → Summary message formatting
├── telemetry.rs        → Telemetry events (tengu_*)
└── tests.rs            → Comprehensive tests

Feature Flags:
├── CompactV2           → Toggle new vs legacy
└── MicroCompact        → Enable tool result compression

Key Types:
├── CompactState        → compacted_tool_ids, tool_token_cache, memory_attachments
├── ReadFileEntry       → filename, timestamp, token_count
├── CompactCompletedEvent → pre/post tokens, files_restored, duration_ms
├── MicroCompactCompletedEvent → tools_compacted, tokens_saved
├── CompactFailedEvent  → reason, error_message
├── CompactThresholdExceededEvent → post_compact_tokens, threshold

Key Functions (matching Claude Code):
├── ELIGIBLE_TOOLS      → pD5 - tools eligible for micro-compact
├── is_agent_file()     → hD5 - exclude agent files from restore
├── reorganize_attachments() → WZ step 1 - reorder attachments
├── merge_consecutive_messages() → WZ step 3 - merge user messages
├── is_thinking_only_block() → NQ0 - filter thinking-only
├── is_synthetic_error_message() → wb3 - filter synthetic errors
├── restore_file_reads() → bD5 - restore files
├── restore_todo_list() → fD5 - restore todos
├── restore_plan_file() → XQ0 - restore plan

EventMsg Extensions (protocol/src/protocol.rs):
├── CompactCompleted(CompactCompletedEvent)           → Full compact success
├── MicroCompactCompleted(MicroCompactCompletedEvent) → Micro-compact success
├── CompactFailed(CompactFailedEvent)                 → Compact failure
└── CompactThresholdExceeded(CompactThresholdExceededEvent) → Threshold warning
```
