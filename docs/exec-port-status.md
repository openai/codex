# Exec Package Port Status

## Summary

The exec package was partially ported by another AI with significant gaps. Created missing core files to support the test suite.

## Files Created

### 1. src/nativeMain/kotlin/ai/solace/coder/exec/process/SandboxType.kt
**Source**: codex-rs/core/src/exec/mod.rs

Enum for platform-specific sandbox types:
- `None` - No sandbox, direct execution
- `MacosSeatbelt` - macOS Seatbelt sandbox
- `LinuxSeccomp` - Linux seccomp sandbox via codex-linux-sandbox
- `WindowsRestrictedToken` - Windows restricted token sandbox

### 2. src/nativeMain/kotlin/ai/solace/coder/exec/sandbox/SandboxManager.kt
**Source**: codex-rs/core/src/sandboxing/mod.rs

Main sandbox orchestration class:
- `selectInitialSandbox()` - Chooses sandbox type based on policy and preference
- `getPlatformSandbox()` - Platform detection (TODO: needs implementation)

Mirrors Rust's SandboxManager which handles:
- Sandbox selection based on SandboxPolicy (ReadOnly, WorkspaceWrite, DangerFullAccess)
- Command transformation to wrap with sandbox executables
- Permission checks and enforcement

### 3. src/nativeMain/kotlin/ai/solace/coder/exec/sandbox/Approvals.kt
**Source**: codex-rs/core/src/tools/sandboxing.rs

Approval and tool execution types:
- `ApprovalStore` - Caches approval decisions using serialized keys
- `ApprovalRequirement` - Skip/NeedsApproval/Forbidden
- `SandboxCommandAssessment` - Safety assessment for commands
- `ToolError` - Rejected or Codex error types
- `SandboxRetryData` - Command metadata for re-execution

## Existing Files

### src/nativeMain/kotlin/ai/solace/coder/exec/shell/
- `ShellDetector.kt` - Shell detection logic (has unresolved platform function references)
- `CommandParser.kt` - Command parsing utilities

## Missing Files (Not Yet Ported)

### From codex-rs/exec/src/

1. **exec_events.rs** (188 lines) - Core event system
   - `ThreadEvent` enum - All execution events
   - `ThreadStartedEvent`, `TurnStartedEvent`, `TurnCompletedEvent`, `TurnFailedEvent`
   - `ItemStartedEvent`, `ItemCompletedEvent`, `ItemUpdatedEvent`
   - `ThreadErrorEvent`
   - `ThreadItem` and `ThreadItemDetails` - Item representation
   - `CommandExecutionStatus` enum

2. **event_processor.rs** - Event processing trait/interface

3. **event_processor_with_human_output.rs** - Human-readable output processor

4. **event_processor_with_jsonl_output.rs** (58 lines) - JSONL output processor
   - `EventProcessorWithJsonOutput` struct

5. **cli.rs** - CLI argument parsing
   - `Cli` struct
   - `Command` enum
   - `ResumeArgs` struct
   - `Color` enum

6. **main.rs** - Entry point for exec binary

### From codex-rs/core/src/sandboxing/

1. **assessment.rs** - Command safety assessment logic
   - Risk analysis for commands
   - Pattern matching for dangerous operations

### From codex-rs/core/src/exec/

Files defining execution primitives:
- Execution environment setup
- Process spawning
- Output capture
- Timeout handling
- Platform-specific execution (Linux sandbox, macOS Seatbelt, Windows restricted token)

## Issues Found

### 1. SandboxManagerTest.kt Errors
The test file has incorrect syntax:
- Uses `SandboxPolicy.ReadOnly()` but ReadOnly is an `object`, not a class
- Should be `SandboxPolicy.ReadOnly`  (no parens)
- Same issue with `SandboxPolicy.DangerFullAccess`

### 2. Missing Platform Functions
`ShellDetector.kt` references undefined platform functions:
- `platformGetUserShellPath()`
- `platformFileExists()`
- `platformFindInPath()`
- `platformIsWindows()`
- `platformIsMacOS()`

These need to be implemented using Kotlin Native platform APIs.

### 3. Missing Core Types
- `TurnContext` - Referenced in UnifiedExec.kt but not defined
- Execution environment types
- Process management types

## Priority for Completion

### High Priority (Needed for Basic Functionality)
1. Fix test file syntax (ReadOnly() → ReadOnly)
2. Implement platform detection functions for ShellDetector
3. Port exec_events.rs - Core event system needed by all tools
4. Port execution primitives from core/src/exec/

### Medium Priority (Needed for Full Tool Support)
5. Port event processors for output formatting
6. Port command assessment logic
7. Implement SandboxManager.transform() for command wrapping

### Low Priority (CLI/Standalone Features)
8. Port CLI argument parsing
9. Port main.rs entry point

## Recommendations

1. **Fix the test first** - Update SandboxManagerTest.kt to use correct syntax
2. **Complete core exec types** - Port exec_events.rs as it's fundamental
3. **Implement platform functions** - Create a platform utilities package
4. **Add missing execution primitives** - Port from core/src/exec/
5. **Test incrementally** - Get tests passing after each major addition

## Notes

- The sandbox system is tightly integrated with platform-specific code
- Kotlin Native platform APIs will need to replace Rust's std::process
- Platform detection can use `Platform.osFamily` from Kotlin Native
- File operations can use kotlinx-io or okio for cross-platform support

## Function Mapping Status

| Rust Function | Kotlin Status | Notes |
|---------------|---------------|-------|
| SandboxManager::new() | ✅ Complete | Constructor |
| SandboxManager::select_initial() | ✅ Complete | selectInitialSandbox() |
| SandboxManager::transform() | ❌ Missing | Command transformation logic |
| SandboxManager::denied() | ❌ Missing | Sandbox denial detection |
| ApprovalStore::get() | ✅ Complete | Generic with reified types |
| ApprovalStore::put() | ✅ Complete | Generic with reified types |
| with_cached_approval() | ❌ Missing | Async approval caching |
| default_approval_requirement() | ❌ Missing | Policy-based requirement |

## Type Mapping Status

| Rust Type | Kotlin Status | File |
|-----------|---------------|------|
| SandboxType | ✅ Complete | process/SandboxType.kt |
| SandboxManager | ✅ Partial | sandbox/SandboxManager.kt |
| SandboxablePreference | ✅ Complete | sandbox/SandboxManager.kt (as SandboxPreference) |
| ApprovalStore | ✅ Complete | sandbox/Approvals.kt |
| ApprovalRequirement | ✅ Complete | sandbox/Approvals.kt |
| SandboxCommandAssessment | ✅ Complete | sandbox/Approvals.kt |
| ToolError | ✅ Complete | sandbox/Approvals.kt |
| SandboxRetryData | ✅ Complete | sandbox/Approvals.kt |
| CommandSpec | ❌ Missing | Needs porting |
| ExecEnv | ❌ Missing | Needs porting |
| ThreadEvent | ❌ Missing | Needs porting from exec_events.rs |
| ThreadItem | ❌ Missing | Needs porting from exec_events.rs |
| Usage | ❌ Missing | Needs porting from exec_events.rs |

---

**Status**: Foundation laid, core event system and execution primitives still needed.

