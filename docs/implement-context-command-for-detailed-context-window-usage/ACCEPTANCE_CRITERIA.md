# Context Window Usage Breakdown Command

## Acceptance Criteria

### AC1: Slash Command Registration
**Given:** User has an active Codex session  
**When:** User types `/context` in the chat  
**Then:** Command is recognized and executes without error  
**Affected Files:**
- `/workspace/repo/codex/codex-rs/tui/src/slash_command.rs`
- `/workspace/repo/codex/codex-rs/tui/src/chatwidget.rs`
**Test Type:** CI TESTABLE  
**Test Implementation:** Unit test in `slash_command.rs` verifying `Context` variant parses correctly and `available_during_task()` returns true

### AC2: Token Calculation
**Given:** An active conversation with system prompt and history  
**When:** Context analyzer processes the session  
**Then:** Returns breakdown with system_prompt, conversation, and tools token counts  
**Affected Files:**
- `/workspace/repo/codex/codex-rs/core/src/context_analyzer.rs`
- `/workspace/repo/codex/codex-rs/core/src/lib.rs`
**Test Type:** CI TESTABLE  
**Test Implementation:** Unit test in `context_analyzer.rs` with mock data verifying `estimate_tokens("test")` returns 1 and `analyze_context()` returns valid ContextBreakdown struct

### AC3: Display Formatting
**Given:** Valid ContextBreakdown data  
**When:** UI renders the context output  
**Then:** Shows total usage percentage, component breakdown, and progress bar  
**Affected Files:**
- `/workspace/repo/codex/codex-rs/tui/src/history_cell.rs`
**Test Type:** CI TESTABLE  
**Test Implementation:** Unit test in `history_cell.rs` verifying `new_context_output()` returns formatted string with expected sections

### AC4: High Usage Warning
**Given:** Context usage exceeds 70% of window  
**When:** Context breakdown is displayed  
**Then:** Shows warning message suggesting `/compact` command  
**Affected Files:**
- `/workspace/repo/codex/codex-rs/tui/src/history_cell.rs`
**Test Type:** CI TESTABLE  
**Test Implementation:** Unit test passing ContextBreakdown with 90% usage, verify output contains "Consider using /compact"

### AC5: Empty Session Handling
**Given:** New session with no conversation history  
**When:** User runs `/context`  
**Then:** Displays breakdown showing only system components (no crash)  
**Affected Files:**
- `/workspace/repo/codex/codex-rs/core/src/context_analyzer.rs`
**Test Type:** CI TESTABLE  
**Test Implementation:** Unit test with empty ConversationHistory, verify returns valid breakdown with conversation=0

## UI Requirements

### UI1: Progress Bar Display
**Description:** ASCII progress bar showing context usage  
**File Path:** `/workspace/repo/codex/codex-rs/tui/src/history_cell.rs`  
**Behavior:** Fills proportionally to usage percentage  
**Layout:** `[████░░░░] 45,231/128,000 (35%)`  
**Test Type:** CI TESTABLE