# Implementation Tasks: Codex Chrome Extension (Missing Components)

## Overview
Implement missing components for the existing Codex Chrome extension skeleton: ModelClient implementations, TaskRunner, TurnManager, ToolsRegistry, BrowserTools, ApprovalManager, and DiffTracker while preserving SQ/EQ architecture from codex-rs.

## Task Execution Order

### Phase 1: Testing Infrastructure & Contracts
Establish contract tests before implementation (TDD approach).

#### T001: Setup Test Utilities [P] ✅
**Files**: `codex-chrome/src/tests/utils/chrome-mocks.ts`, `codex-chrome/src/tests/utils/test-helpers.ts`
**Deps**: None
```typescript
// Chrome API mocks for testing
// Helper functions for async testing
// Mock message passing between components
```

#### T002: Create Contract Tests - ModelClient [P] ✅
**Files**: `codex-chrome/src/tests/contracts/model-client.test.ts`
**Deps**: T001
- Test OpenAIClient contract (CompletionRequest/Response)
- Test AnthropicClient contract (AnthropicRequest/Response)
- Validate message format and tool calls
- Test streaming responses

#### T003: Create Contract Tests - TaskRunner [P] ✅
**Files**: `codex-chrome/src/tests/contracts/task-runner.test.ts`
**Deps**: T001
- Test TaskExecutionRequest/Response
- Test task cancellation
- Validate progress events
- Test error handling

#### T004: Create Contract Tests - TurnManager [P] ✅
**Files**: `codex-chrome/src/tests/contracts/turn-manager.test.ts`
**Deps**: T001
- Test TurnRequest/Response
- Test conversation state management
- Validate turn context updates
- Test retry logic

#### T005: Create Contract Tests - ToolRegistry [P] ✅
**Files**: `codex-chrome/src/tests/contracts/tool-registry.test.ts`
**Deps**: T001
- Test tool registration
- Test tool discovery
- Test parameter validation
- Test execution dispatch

#### T006: Create Contract Tests - Browser Tools [P] ✅
**Files**: `codex-chrome/src/tests/contracts/browser-tools.test.ts`
**Deps**: T001
- Test TabToolRequest/Response
- Test DOMToolRequest/Response
- Test StorageToolRequest/Response
- Test NavigationToolRequest/Response

#### T007: Create Contract Tests - ApprovalManager [P] ✅
**Files**: `codex-chrome/src/tests/contracts/approval-manager.test.ts`
**Deps**: T001
- Test ApprovalRequest/Response
- Test ReviewDecision handling
- Validate approval policies
- Test timeout scenarios

#### T008: Create Contract Tests - DiffTracker [P] ✅
**Files**: `codex-chrome/src/tests/contracts/diff-tracker.test.ts`
**Deps**: T001
- Test AddChangeRequest/GetChangesRequest
- Test DiffResult format
- Validate change tracking
- Test rollback operations

### Phase 2: Core Implementation - Model Clients

#### T009: Create ModelClient Base Interface ✅
**Files**: `codex-chrome/src/models/ModelClient.ts`
**Deps**: Protocol types exist
```typescript
interface ModelClient {
  complete(request: CompletionRequest): Promise<CompletionResponse>;
  stream(request: CompletionRequest): AsyncGenerator<StreamChunk>;
  countTokens(text: string, model: string): number;
}
```

#### T010: Implement OpenAI ModelClient ✅
**Files**: `codex-chrome/src/models/OpenAIClient.ts`
**Deps**: T002, T009
- Implement complete() method with API key support
- Implement stream() generator for streaming responses
- Add token counting logic
- Handle tool calls and function calling
- Add retry logic with exponential backoff

#### T011: Implement Anthropic ModelClient ✅
**Files**: `codex-chrome/src/models/AnthropicClient.ts`
**Deps**: T002, T009
- Implement complete() method with Claude API
- Implement stream() generator
- Handle content blocks and tool use
- Add proper error handling
- Support system prompts

#### T012: Create ModelClient Factory ✅
**Files**: `codex-chrome/src/models/ModelClientFactory.ts`
**Deps**: T010, T011
- Create factory to instantiate correct client
- Load API keys from Chrome storage
- Handle provider selection
- Cache client instances

### Phase 3: Core Implementation - Task & Turn Management

#### T013: Implement TaskRunner ✅
**Files**: `codex-chrome/src/core/TaskRunner.ts`
**Deps**: T003, T009
- Port run_task equivalent from Rust
- Handle task execution lifecycle
- Emit progress events through EQ
- Support task cancellation
- Integrate with TurnManager

#### T014: Implement TurnManager ✅
**Files**: `codex-chrome/src/core/TurnManager.ts`
**Deps**: T004, T009
- Port run_turn equivalent from Rust
- Manage conversation flow
- Handle turn context
- Process user inputs
- Coordinate with model clients

#### T015: Implement Turn Context Manager ✅
**Files**: `codex-chrome/src/core/TurnContext.ts`
**Deps**: T014
- Manage turn state
- Handle context switching
- Store conversation history
- Apply approval and sandbox policies

### Phase 4: Core Implementation - Tools System

#### T016: Implement ToolRegistry ✅
**Files**: `codex-chrome/src/tools/ToolRegistry.ts`
**Deps**: T005
- Create tool registration system
- Handle tool discovery
- Implement execution dispatch
- Add validation logic
- Support dynamic tool loading

#### T017: Create Base Tool Class ✅
**Files**: `codex-chrome/src/tools/BaseTool.ts`
**Deps**: T016
```typescript
abstract class BaseTool implements Tool {
  abstract name: string;
  abstract execute(params: any): Promise<ToolResult>;
}
```

#### T018: Implement TabTool [P] ✅
**Files**: `codex-chrome/src/tools/TabTool.ts`
**Deps**: T006, T017
- Implement openTab, closeTab, switchTab
- Implement getAllTabs, getCurrentTab
- Add screenshot capability
- Handle tab events

#### T019: Implement DOMTool [P] ✅
**Files**: `codex-chrome/src/tools/DOMTool.ts`
**Deps**: T006, T017
- Implement click, type, submit methods
- Implement querySelector, extractText
- Handle cross-frame communication
- Add element waiting logic

#### T020: Implement StorageTool [P] ✅
**Files**: `codex-chrome/src/tools/StorageTool.ts`
**Deps**: T006, T017
- Implement get/set/remove for chrome.storage
- Support local, session, and sync storage
- Add data migration support
- Handle storage quotas

#### T021: Implement NavigationTool [P] ✅
**Files**: `codex-chrome/src/tools/NavigationTool.ts`
**Deps**: T006, T017
- Implement goto, back, forward, refresh
- Add waitForNavigation support
- Handle navigation errors
- Track navigation history

### Phase 5: Core Implementation - Approval & Tracking

#### T022: Implement ApprovalManager ✅
**Files**: `codex-chrome/src/core/ApprovalManager.ts`
**Deps**: T007
- Implement requestApproval method
- Handle approval policies
- Create approval queue
- Store approval history
- Add timeout handling

#### T023: Create Approval UI Component ✅
**Files**: `codex-chrome/src/sidepanel/components/ApprovalDialog.svelte`
**Deps**: T022
- Create Svelte component for approvals
- Display tool details and risks
- Handle user decisions
- Show approval history

#### T024: Implement DiffTracker ✅
**Files**: `codex-chrome/src/core/DiffTracker.ts`
**Deps**: T008
- Track DOM changes
- Track storage changes
- Generate diff reports
- Implement undo functionality
- Store change history

### Phase 6: Integration - Wire Components Together

#### T025: Update CodexAgent Integration ✅
**Files**: `codex-chrome/src/core/CodexAgent.ts`
**Deps**: T013, T014, T016, T022, T024
- Integrate TaskRunner
- Wire up TurnManager
- Connect ToolRegistry
- Add ApprovalManager
- Enable DiffTracker

#### T026: Update Background Service Worker ✅
**Files**: `codex-chrome/src/background/index.ts`
**Deps**: T025, T012
- Initialize ModelClientFactory
- Setup message routing for new components
- Handle Chrome runtime events
- Manage component lifecycle

#### T027: Update Message Router ✅
**Files**: `codex-chrome/src/core/MessageRouter.ts`
**Deps**: T026
- Add routes for model client messages
- Handle tool execution messages
- Route approval requests
- Distribute diff events

#### T028: Update Content Script Integration ✅
**Files**: `codex-chrome/src/content/index.ts`
**Deps**: T019, T027
- Connect DOM tool execution
- Setup message listeners
- Handle page isolation
- Inject necessary scripts

#### T029: Update Session Management ✅
**Files**: `codex-chrome/src/core/Session.ts`
**Deps**: T015, T016
- Integrate turn context
- Register all browser tools
- Setup tool permissions
- Initialize tracking

### Phase 7: UI Components

#### T030: Create Tool Execution Display [P]
**Files**: `codex-chrome/src/sidepanel/components/ToolExecution.svelte`
**Deps**: Sidepanel exists
- Display tool execution status
- Show tool parameters
- Display results
- Handle errors

#### T031: Create Diff Display Component [P]
**Files**: `codex-chrome/src/sidepanel/components/DiffDisplay.svelte`
**Deps**: T024
- Show changes made
- Display before/after states
- Enable undo operations
- Group changes by type

#### T032: Update Main App Component
**Files**: `codex-chrome/src/sidepanel/App.svelte`
**Deps**: T023, T030, T031
- Integrate approval dialog
- Add tool execution display
- Show diff tracking
- Update event handling

### Phase 8: Polish & Documentation

#### T033: Add Error Boundaries [P]
**Files**: Multiple component files
**Deps**: All UI components
- Add try-catch to all async operations
- Implement error recovery
- Create error reporting
- Add user-friendly error messages

#### T034: Add Logging System [P]
**Files**: `codex-chrome/src/utils/logger.ts`
**Deps**: Core components complete
- Implement debug logging
- Add performance monitoring
- Create log persistence
- Add log levels

#### T035: Create Integration Tests [P]
**Files**: `codex-chrome/src/tests/integration/*.test.ts`
**Deps**: All implementation complete
- Test end-to-end flows
- Test error scenarios
- Validate Chrome API usage
- Test message passing

#### T036: Add TypeScript Strict Checks [P]
**Files**: `codex-chrome/tsconfig.json`, multiple .ts files
**Deps**: All TypeScript files
- Enable strict mode
- Fix type errors
- Add missing type annotations
- Remove any types

#### T037: Create User Documentation [P]
**Files**: `codex-chrome/README.md`, `codex-chrome/docs/*.md`
**Deps**: All features complete
- Installation guide
- API documentation
- Tool usage examples
- Troubleshooting guide

## Parallel Execution Examples

Group 1 - Contract Tests (after T001):
```bash
# All contract tests can run in parallel
Task T002 && Task T003 && Task T004 && Task T005 && Task T006 && Task T007 && Task T008
```

Group 2 - Model Clients (after contracts):
```bash
# Independent implementations
Task T010 && Task T011
```

Group 3 - Browser Tools (after T017):
```bash
# All tools are independent
Task T018 && Task T019 && Task T020 && Task T021
```

Group 4 - UI Components (independent):
```bash
# Different components
Task T030 && Task T031
```

Group 5 - Polish Tasks (at the end):
```bash
# Documentation and quality
Task T033 && Task T034 && Task T035 && Task T036 && Task T037
```

## Task Dependencies Graph
```
T001 → T002-T008 (contracts)
     ↓
T009 → T010, T011 → T012 (model clients)
     ↓
T013, T014 → T015 (task/turn management)
     ↓
T016 → T017 → T018-T021 (tools)
     ↓
T022 → T023 (approval)
T024 (diff tracking)
     ↓
T025-T029 (integration)
     ↓
T030-T032 (UI updates)
     ↓
T033-T037 (polish)
```

## Success Criteria
- All contract tests pass before implementation
- SQ/EQ architecture preserved from codex-rs
- All protocol type names match exactly
- Chrome extension loads without errors
- All tools execute successfully
- Approval flow works correctly
- Changes are tracked and reversible

## Notes
- The codex-chrome skeleton already exists with basic structure
- Focus only on missing components, not recreating the entire extension
- Preserve exact type names from codex-rs protocol
- Tasks marked [P] can be executed in parallel
- Each task is self-contained and immediately executable