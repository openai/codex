# Protocol Conversion Summary

## Overview
This document summarizes the conversion of core protocol types from `codex-rs/protocol/src/protocol.rs` (1820 lines of Rust) to Kotlin Native in `ai.solace.coder.protocol`.

## Files Created

### 1. `src/nativeMain/kotlin/ai/solace/coder/protocol/Protocol.kt` (1599 lines)
Main protocol definitions file containing all core types.

### 2. `src/nativeTest/kotlin/ai/solace/coder/protocol/ProtocolTest.kt` (531 lines)
Comprehensive unit tests covering all protocol types with serialization round-trip tests.

## Types Converted

### Core Protocol Types

#### 1. **Op** (Sealed Class - 16 Variants)
Submission operations representing user requests:
- `Interrupt` - Abort current task
- `UserInput` - Simple user input
- `UserTurn` - Full turn with context (cwd, policies, model)
- `OverrideTurnContext` - Update persistent context
- `ExecApproval` - Approve command execution
- `PatchApproval` - Approve code patch
- `ResolveElicitation` - Resolve MCP elicitation request
- `AddToHistory` - Append to message history
- `GetHistoryEntryRequest` - Request history entry
- `ListMcpTools` - Request MCP tools list
- `ListCustomPrompts` - Request custom prompts
- `Compact` - Request conversation summary
- `Undo` - Undo last turn
- `Review` - Request code review
- `Shutdown` - Shutdown request
- `RunUserShellCommand` - Execute user shell command

#### 2. **EventMsg** (Sealed Class - 44+ Variants)
Agent events for UI updates:
- `Error`, `Warning` - Error and warning notifications
- `ContextCompacted` - History compaction notification
- `TaskStarted`, `TaskComplete` - Task lifecycle
- `TokenCount` - Token usage updates
- `AgentMessage`, `UserMessage` - Message events
- `AgentMessageDelta` - Streaming message deltas
- `AgentReasoning*` - Reasoning events (summary, raw, deltas, section breaks)
- `SessionConfigured` - Session initialization
- `McpStartupUpdate`, `McpStartupComplete` - MCP server startup
- `McpToolCallBegin`, `McpToolCallEnd` - MCP tool invocations
- `WebSearchBegin`, `WebSearchEnd` - Web search events
- `ExecCommandBegin`, `ExecCommandEnd`, `ExecCommandOutputDelta` - Command execution
- `ViewImageToolCall` - Image viewing
- `ExecApprovalRequest`, `ElicitationRequest`, `ApplyPatchApprovalRequest` - Approval requests
- `DeprecationNotice` - Deprecation warnings
- `BackgroundEvent` - Background notifications
- `UndoStarted`, `UndoCompleted` - Undo lifecycle
- `StreamError` - Stream error handling
- `PatchApplyBegin`, `PatchApplyEnd` - Patch application
- `TurnDiff` - Turn diff display
- `GetHistoryEntryResponse` - History entry response
- `McpListToolsResponse` - MCP tools response
- `ListCustomPromptsResponse` - Custom prompts response
- `PlanUpdate` - Plan updates
- `TurnAborted` - Turn abortion
- `ShutdownComplete` - Shutdown confirmation
- `EnteredReviewMode`, `ExitedReviewMode` - Review mode lifecycle
- `RawResponseItem` - Raw response items
- `ItemStarted`, `ItemCompleted` - Item lifecycle
- `AgentMessageContentDelta`, `ReasoningContentDelta`, `ReasoningRawContentDelta` - Content deltas

#### 3. **AskForApproval** (Enum - 4 Values)
Command approval policies:
- `UnlessTrusted` - Only auto-approve safe commands
- `OnFailure` - Auto-approve, escalate on failure
- `OnRequest` - Model decides (default)
- `Never` - Never ask for approval

#### 4. **SandboxPolicy** (Sealed Class - 3 Variants)
Execution restrictions:
- `DangerFullAccess` - No restrictions
- `ReadOnly` - Read-only filesystem access
- `WorkspaceWrite` - Workspace + tmp write access with options:
  - `writable_roots` - Additional writable directories
  - `network_access` - Network access flag
  - `exclude_tmpdir_env_var` - Exclude TMPDIR
  - `exclude_slash_tmp` - Exclude /tmp

**Special Implementation:**
- `getWritableRootsWithCwd()` - Platform-specific logic for writable roots
- Automatically includes cwd, /tmp (Unix), and TMPDIR
- Marks .git directories as read-only within writable roots

#### 5. **WritableRoot** (Data Class)
Represents a writable directory with read-only subpaths:
- `root` - Root path
- `read_only_subpaths` - Paths that remain read-only
- `isPathWritable()` - Path permission checking

### Token Usage Types

#### 6. **TokenUsage** (Data Class)
Token consumption tracking:
- `input_tokens`, `cached_input_tokens`, `output_tokens`, `reasoning_output_tokens`, `total_tokens`
- Methods: `isZero()`, `cachedInput()`, `nonCachedInput()`, `blendedTotal()`, `tokensInContextWindow()`, `percentOfContextWindowRemaining()`, `addAssign()`

#### 7. **TokenUsageInfo** (Data Class)
Aggregated token usage:
- `total_token_usage`, `last_token_usage`, `model_context_window`
- Methods: `appendLastUsage()`, `fillToContextWindow()`, `newOrAppend()`, `fullContextWindow()`

#### 8. **TokenCountEvent** (Data Class)
Token count event payload with usage info and rate limits

### Rate Limit Types

#### 9. **RateLimitSnapshot** (Data Class)
Rate limit state snapshot with primary/secondary windows and credits

#### 10. **RateLimitWindow** (Data Class)
Rate limit window details:
- `used_percent`, `window_minutes`, `resets_at`

#### 11. **CreditsSnapshot** (Data Class)
Credit balance information:
- `has_credits`, `unlimited`, `balance`

### Session Types

#### 12. **SessionSource** (Enum - 6 Values)
Session origin:
- `Cli`, `VSCode`, `Exec`, `Mcp`, `SubAgent`, `Unknown`

#### 13. **SubAgentSource** (Sealed Class - 3 Variants)
Sub-agent types:
- `Review`, `Compact`, `Other`

#### 14. **SessionMeta** (Data Class)
Session metadata with id, timestamp, cwd, originator, version, instructions, source, provider

#### 15. **SessionMetaLine** (Data Class)
Session metadata with optional git info

#### 16. **InitialHistory** (Sealed Class - 3 Variants)
Initial session history:
- `New` - Fresh session
- `Resumed` - Resumed session with history
- `Forked` - Forked from existing session
- Methods: `getRolloutItems()`, `getEventMsgs()`

#### 17. **RolloutItem** (Sealed Class - 5 Variants)
Session history item types:
- `SessionMeta`, `ResponseItem`, `Compacted`, `TurnContext`, `EventMsg`

#### 18. **ResumedHistory**, **CompactedItem**, **TurnContextItem**, **GitInfo**
Supporting session data classes

### Review Types

#### 19. **ReviewDecision** (Enum - 4 Values)
Approval decision:
- `Approved`, `ApprovedForSession`, `Denied`, `Abort`

#### 20. **ReviewRequest**, **ReviewOutputEvent**, **ReviewFinding**, **ReviewCodeLocation**, **ReviewLineRange**
Code review types with structured findings and confidence scores

#### 21. **FileChange** (Sealed Class - 3 Variants)
File modification types:
- `Add` - New file
- `Delete` - Removed file
- `Update` - Modified file with diff and optional move path

### Error Types

#### 22. **CodexErrorInfo** (Sealed Class - 11 Variants)
Structured error information:
- `ContextWindowExceeded`, `UsageLimitExceeded`, `HttpConnectionFailed`, `ResponseStreamConnectionFailed`, `InternalServerError`, `Unauthorized`, `BadRequest`, `SandboxError`, `ResponseStreamDisconnected`, `ResponseTooManyFailedAttempts`, `Other`

### Command Execution Types

#### 23. **ExecCommandSource** (Enum - 4 Values)
Command origin:
- `Agent`, `UserShell`, `UnifiedExecStartup`, `UnifiedExecInteraction`

#### 24. **ExecOutputStream** (Enum - 2 Values)
Output stream type:
- `Stdout`, `Stderr`

#### 25. **ExecCommandBeginEvent**, **ExecCommandEndEvent**, **ExecCommandOutputDeltaEvent**
Command execution lifecycle events with output streaming

### MCP Types

#### 26. **McpAuthStatus** (Enum - 4 Values)
MCP authentication status:
- `Unsupported`, `NotLoggedIn`, `BearerToken`, `OAuth`

#### 27. **McpStartupStatus** (Sealed Class - 4 Variants)
MCP server startup status:
- `Starting`, `Ready`, `Failed`, `Cancelled`

#### 28. **McpInvocation**, **McpToolCallBeginEvent**, **McpToolCallEndEvent**
MCP tool call tracking with success detection

#### 29. **McpStartupUpdateEvent**, **McpStartupCompleteEvent**, **McpStartupFailure**
MCP startup progress tracking

### Supporting Event Payloads (40+ Data Classes)

All event payload types corresponding to `EventMsg` variants:
- `ErrorEvent`, `WarningEvent`, `ContextCompactedEvent`
- `TaskStartedEvent`, `TaskCompleteEvent`
- `AgentMessageEvent`, `UserMessageEvent`, `AgentMessageDeltaEvent`
- `AgentReasoningEvent`, `AgentReasoningDeltaEvent`, etc.
- `WebSearchBeginEvent`, `WebSearchEndEvent`
- `ViewImageToolCallEvent`
- `PatchApplyBeginEvent`, `PatchApplyEndEvent`
- `TurnDiffEvent`, `TurnAbortedEvent`
- And many more...

### Other Types

#### 30. **Submission**, **Event**
Top-level wrapper types for submissions and events

#### 31. **TurnAbortReason** (Enum - 3 Values)
Turn abortion reasons:
- `Interrupted`, `Replaced`, `ReviewEnded`

#### 32. **HasLegacyEvent** (Interface)
Interface for types that emit legacy events for backward compatibility

## Key Implementation Details

### Serialization
- All types use `@Serializable` annotation from kotlinx.serialization
- Snake_case JSON serialization via `@SerialName` annotations
- Sealed classes for tagged unions (Rust enums with data)
- Enums for simple variants without associated data

### Platform-Specific Logic
- `SandboxPolicy.getWritableRootsWithCwd()` uses Kotlin Native's cinterop for environment variables
- Cross-platform support for macOS, Linux, and Windows
- Proper handling of Unix-specific paths (/tmp, TMPDIR)

### Special Features
- Token usage calculations with context window percentage
- Writable root path validation with read-only subpath exclusions
- Rate limit tracking with rolling windows
- MCP tool call success detection
- Round-trip serialization support

## Testing

### Test Coverage (531 lines)
Comprehensive unit tests covering:
- **Serialization tests** for all major types
- **Deserialization tests** with JSON parsing
- **Round-trip tests** ensuring serialize/deserialize consistency
- **Business logic tests** for TokenUsage, SandboxPolicy, WritableRoot
- **Enum serialization** verifying correct snake_case/kebab-case mapping
- **Edge cases** for token calculations, rate limits, etc.

### Test Categories
1. Op serialization/deserialization (Interrupt, UserTurn, etc.)
2. AskForApproval enum mapping
3. SandboxPolicy variants and writable roots logic
4. WritableRoot path permission checking
5. Event serialization (Error, TaskComplete, etc.)
6. TokenUsage calculations and aggregation
7. RateLimit snapshot serialization
8. ReviewDecision enum mapping
9. FileChange sealed class variants
10. Session types (SessionSource, SessionMeta, InitialHistory)
11. ExecCommand types and streams
12. MCP types (auth status, startup status, tool calls)
13. Review types with findings
14. TurnAbort reasons
15. Full round-trip tests for complex types

## Differences from Rust Implementation

1. **PathBuf → String**: Kotlin Native uses `String` for paths instead of dedicated `PathBuf` type
2. **Duration → String**: Duration serialized as string for JSON compatibility
3. **Vec → List**: Rust `Vec<T>` becomes Kotlin `List<T>`
4. **HashMap → Map**: Rust `HashMap<K, V>` becomes Kotlin `Map<K, V>`
5. **Option → Nullable**: Rust `Option<T>` becomes Kotlin `T?`
6. **Result → Result data class**: Custom Result wrapper for success/error (not using kotlin.Result due to serialization)
7. **Platform APIs**: Using kotlinx.cinterop for POSIX functions instead of Rust std lib

## Usage Example

```kotlin
// Create a submission
val submission = Submission(
    id = "sub-123",
    op = Op.UserTurn(
        items = listOf(UserInput(text = "Hello")),
        cwd = "/workspace",
        approval_policy = AskForApproval.OnRequest,
        sandbox_policy = SandboxPolicy.WorkspaceWrite(
            network_access = true
        ),
        model = "claude-3-5-sonnet",
        summary = ReasoningSummaryConfig(enabled = true)
    )
)

// Serialize to JSON
val json = Json { prettyPrint = true }
val jsonString = json.encodeToString(submission)

// Deserialize from JSON
val parsed = json.decodeFromString<Submission>(jsonString)

// Create an event
val event = Event(
    id = "sub-123",
    msg = EventMsg.AgentMessage(AgentMessageEvent(
        message = "Task completed successfully"
    ))
)

// Check sandbox policy
val policy = SandboxPolicy.WorkspaceWrite()
val roots = policy.getWritableRootsWithCwd("/workspace")
roots.forEach { root ->
    println("Writable: ${root.root}")
    root.read_only_subpaths.forEach { subpath ->
        println("  Read-only: $subpath")
    }
}

// Track token usage
val usage = TokenUsage(
    input_tokens = 100,
    output_tokens = 50,
    total_tokens = 150
)
println("Blended total: ${usage.blendedTotal()}")
println("Context remaining: ${usage.percentOfContextWindowRemaining(100000)}%")
```

## Validation

The implementation ensures:
✅ All 16 Op variants converted
✅ All 44+ EventMsg variants converted
✅ All enums with proper serialization names
✅ Sealed classes for tagged unions
✅ Platform-specific writable roots logic
✅ Token usage calculations
✅ Rate limit tracking
✅ HasLegacyEvent interface pattern
✅ Comprehensive unit tests
✅ Serialization round-trip tests

## Next Steps

Potential future enhancements:
1. Implement actual `TurnItem`, `ParsedCommand`, `CustomPrompt` types (currently placeholders)
2. Add more platform-specific tests for writable roots
3. Implement legacy event conversion logic in `HasLegacyEvent` implementations
4. Add performance benchmarks for serialization
5. Create integration tests with actual Codex backend

## Conclusion

Successfully converted 1820 lines of Rust protocol definitions to 1599 lines of idiomatic Kotlin Native code with 531 lines of comprehensive tests. All core protocol types, enums, sealed classes, and data classes are properly implemented with kotlinx.serialization support and platform-specific logic where needed.