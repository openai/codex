# Codex Chrome Extension - Implementation Tasks

## Overview
Convert codex-rs terminal agent to Chrome extension, preserving the SQ/EQ architecture and exact protocol types.

## Phase 1: Project Setup & Protocol Port

### T001: Initialize Chrome Extension Project
**Priority**: Critical
**Files**: package.json, tsconfig.json, manifest.json
```bash
mkdir codex-chrome && cd codex-chrome
npm init -y
npm install typescript vite svelte zod
npm install -D @types/chrome @sveltejs/vite-plugin-svelte
```

### T002: Port Protocol Types
**Priority**: Critical
**Source**: codex-rs/protocol/src/protocol.rs
**Target**: src/protocol/types.ts
- Port `Submission` and `Op` enum exactly
- Port `Event` and `EventMsg` enum exactly
- Preserve all type names without changes

### T003: Port Event Types
**Priority**: Critical
**Source**: codex-rs/protocol/src/protocol.rs (lines 200-1400)
**Target**: src/protocol/events.ts
- Port all event data structures (TaskStartedEvent, AgentMessageEvent, etc.)
- Maintain exact field names and types

### T004: Port Model Types
**Priority**: High
**Source**: codex-rs/protocol/src/models.rs
**Target**: src/protocol/models.ts
- Port ContentItem, ResponseItem types
- Port conversation and message types

## Phase 2: Core Agent Implementation

### T005: Implement CodexAgent Class
**Priority**: Critical
**Source**: codex-rs/core/src/codex.rs (Codex struct)
**Target**: src/core/CodexAgent.ts
```typescript
class CodexAgent {
  private nextId: AtomicU64 equivalent
  private submissionQueue: Submission[]
  private eventQueue: Event[]

  async submitOperation(op: Op): Promise<string>
  async getNextEvent(): Promise<Event | null>
}
```

### T006: Implement Session Management
**Priority**: Critical
**Source**: codex-rs/core/src/codex.rs (Session struct)
**Target**: src/core/Session.ts
- Port Session struct with Chrome adaptations
- Replace MCP with Chrome tool registry
- Adapt state management for browser

### T007: Implement Queue Processing
**Priority**: Critical
**Target**: src/core/Queue.ts
- Implement SQ processing logic
- Implement EQ emission logic
- Handle async message passing

### T008: Port Turn Context Management
**Priority**: High
**Source**: codex-rs/protocol/src/protocol.rs (TurnContext)
**Target**: src/core/TurnContext.ts
- Port TurnContext structure
- Adapt sandbox policies for browser
- Implement override logic

## Phase 3: Chrome Extension Infrastructure

### T009: Setup Background Service Worker
**Priority**: Critical
**Target**: src/background/index.ts
- Initialize CodexAgent instance
- Setup Chrome message listeners
- Handle extension lifecycle events

### T010: Implement Message Router
**Priority**: Critical
**Target**: src/background/MessageRouter.ts
- Route messages between components
- Handle Submission forwarding
- Handle Event distribution

### T011: Create Storage Manager
**Priority**: High
**Target**: src/background/StorageManager.ts
- Implement Chrome storage wrapper
- Handle conversation persistence
- Manage session state

### T012: Setup Content Script Base
**Priority**: High
**Target**: src/content/index.ts
- Create message listener
- Setup DOM injection framework
- Handle page context isolation

## Phase 4: Tool System (Interfaces Only)

### T013: Create Tool Base Interface
**Priority**: High
**Target**: src/tools/Tool.ts
```typescript
interface Tool {
  name: string;
  description: string;
  execute(params: any): Promise<ToolResult>;
}
```

### T014: Create Tab Management Tool Interface
**Priority**: Medium
**Target**: src/tools/TabManager.ts
- Define openTab, closeTab, switchTab methods
- Define getAllTabs, getCurrentTab methods
- Create stub implementations

### T015: Create Page Interaction Tool Interface
**Priority**: Medium
**Target**: src/tools/PageInteractor.ts
- Define click, type, submit methods
- Define scroll, screenshot methods
- Create stub implementations

### T016: Create Data Extraction Tool Interface
**Priority**: Medium
**Target**: src/tools/DataExtractor.ts
- Define getText, getHTML methods
- Define getAttribute, getAllElements methods
- Create stub implementations

### T017: Create Navigation Tool Interface
**Priority**: Medium
**Target**: src/tools/Navigator.ts
- Define goto, back, forward, refresh methods
- Define waitForNavigation method
- Create stub implementations

### T018: Implement Tool Registry
**Priority**: High
**Source**: Pattern from codex-rs/core/src/openai_tools.rs
**Target**: src/core/ToolRegistry.ts
- Port tool registration logic
- Implement tool discovery
- Handle tool execution dispatch

## Phase 5: UI Implementation

### T019: Create Side Panel HTML
**Priority**: High
**Target**: src/sidepanel/index.html
- Basic HTML structure
- Load Svelte app
- Include Tailwind CSS

### T020: Implement Main Svelte App
**Priority**: High
**Target**: src/sidepanel/App.svelte
- Create input interface
- Display event stream
- Handle user submissions

### T021: Create Event Display Component
**Priority**: Medium
**Target**: src/sidepanel/components/EventDisplay.svelte
- Render different event types
- Handle AgentMessage events
- Show execution status

### T022: Create Input Component
**Priority**: Medium
**Target**: src/sidepanel/components/QueryInput.svelte
- Text input with validation
- Submit handling
- Keyboard shortcuts

### T023: Implement Event Polling
**Priority**: High
**Target**: src/sidepanel/lib/EventPoller.ts
- Poll background for events
- Handle event queue
- Update UI state

## Phase 6: Message Flow Integration

### T024: Connect Side Panel to Background
**Priority**: Critical
**Dependencies**: T009, T020
- Implement submission sending
- Implement event receiving
- Handle connection lifecycle

### T025: Connect Background to Content Scripts
**Priority**: High
**Dependencies**: T009, T012
- Implement tab messaging
- Handle script injection
- Manage response routing

### T026: Implement Op Handlers
**Priority**: Critical
**Dependencies**: T005, T002
- Handle UserInput op
- Handle UserTurn op
- Handle Interrupt op
- Handle other ops

### T027: Implement Event Emitters
**Priority**: Critical
**Dependencies**: T005, T003
- Emit TaskStarted/Complete events
- Emit AgentMessage events
- Emit Error events
- Emit tool execution events

## Phase 7: Testing

### T028: Test Protocol Types
**Priority**: High
**Target**: tests/protocol.test.ts
- Test type serialization/deserialization
- Test type guards
- Test Zod validation

### T029: Test Queue Architecture
**Priority**: High
**Target**: tests/queue.test.ts
- Test submission queuing
- Test event dequeuing
- Test async processing

### T030: Test Message Flow
**Priority**: High
**Target**: tests/integration/messageFlow.test.ts
- Test end-to-end message flow
- Test error handling
- Test interruption

### T031: Test Chrome APIs
**Priority**: Medium
**Target**: tests/chrome.test.ts
- Mock Chrome APIs
- Test storage operations
- Test tab operations

## Phase 8: Build and Package

### T032: Configure Vite Build
**Priority**: High
**Target**: vite.config.ts
- Setup multiple entry points
- Configure Chrome extension build
- Handle asset copying

### T033: Create Build Scripts
**Priority**: Medium
**Target**: package.json scripts
```json
"scripts": {
  "dev": "vite",
  "build": "vite build",
  "preview": "vite preview",
  "test": "vitest"
}
```

### T034: Setup Development Workflow
**Priority**: Low
**Target**: .github/workflows/ci.yml
- Automated testing
- Build verification
- Type checking

## Critical Path

Must complete in order:
1. T001 → T002 → T003 → T005 → T006 → T009 → T020 → T024

Can parallelize:
- Protocol types (T002-T004)
- Tool interfaces (T014-T017)
- UI components (T021-T023)
- Tests (T028-T031)

## Success Criteria

- [ ] SQ/EQ architecture working
- [ ] Protocol types match Rust exactly
- [ ] Basic user input → agent response flow
- [ ] Chrome extension loads without errors
- [ ] Side panel accepts input
- [ ] Events display in UI
- [ ] Tool interfaces defined (not implemented)

## Notes

1. **DO NOT** rename any protocol types - keep exact Rust names
2. **DO NOT** implement full tool logic yet - interfaces only
3. **FOCUS** on message flow and protocol preservation
4. **PRESERVE** the queue-based architecture