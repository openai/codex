# Codex Chrome Extension - Implementation Plan (Corrected)

## Executive Summary

This implementation plan converts the Codex terminal agent (codex-rs) to a Chrome extension while preserving the core **SQ/EQ (Submission Queue/Event Queue)** architecture and exact protocol types found in the actual codebase.

## Actual Architecture Analysis

### What Actually Exists in codex-rs

1. **Core Components**:
   - `Codex` struct with queue-based message passing (not "AgentSystem")
   - `Session` struct for conversation management (not separate managers)
   - Protocol with `Submission`/`Op` and `Event`/`EventMsg` types
   - Tool system via `openai_tools.rs` (not separate tool classes)

2. **Message Flow**:
   - User submits `Op` via `Submission` to SQ
   - Core processes and emits `EventMsg` via `Event` to EQ
   - Async processing with tokio channels

3. **Key Files**:
   - `codex-rs/core/src/codex.rs` - Main orchestration (4,600+ lines)
   - `codex-rs/protocol/src/protocol.rs` - Protocol definitions (1,400+ lines)
   - `codex-rs/core/src/client.rs` - Model client
   - `codex-rs/core/src/openai_tools.rs` - Tool definitions

## Implementation Strategy

### Core Principles

1. **Preserve Exact Type Names**: All protocol types keep their Rust names
2. **Maintain SQ/EQ Pattern**: Queue-based async architecture
3. **Adapt for Browser**: Replace file/shell operations with browser operations
4. **Tool Interfaces First**: Define interfaces now, implement details later

### Technology Stack

- **TypeScript 5.x** - With strict mode
- **Svelte 4.x** - Side panel UI
- **Tailwind CSS** - Styling
- **Vite** - Build system
- **Chrome Manifest V3** - Extension framework
- **Zod** - Runtime validation

## Implementation Phases

### Phase 1: Protocol Port (Week 1)

**Goal**: Port exact protocol types from Rust

1. **Protocol Types** (`protocol.rs` → `protocol/types.ts`)
   - `Submission` and `Op` enum
   - `Event` and `EventMsg` enum
   - All event data types

2. **Model Types** (`models.rs` → `protocol/models.ts`)
   - ContentItem, ResponseItem
   - Message types

**Deliverables**:
- `src/protocol/` directory with exact type definitions
- Type guards and validators
- Zero deviation from Rust naming

### Phase 2: Core Agent (Week 2)

**Goal**: Implement queue-based agent architecture

1. **CodexAgent Class** (port of `Codex` struct)
   ```typescript
   class CodexAgent {
     submitOperation(op: Op): Promise<string>
     getNextEvent(): Promise<Event | null>
   }
   ```

2. **Session Management** (port of `Session` struct)
   - Conversation state
   - Tool registry
   - Turn context

3. **Queue Processing**
   - Handle submissions asynchronously
   - Emit events to queue
   - Message routing

**Deliverables**:
- `src/core/` with agent implementation
- Working SQ/EQ pattern
- Event emission system

### Phase 3: Chrome Infrastructure (Week 3)

**Goal**: Setup Chrome extension framework

1. **Background Service Worker**
   - Initialize CodexAgent
   - Handle Chrome messages
   - Manage lifecycle

2. **Content Scripts**
   - Injection framework
   - DOM interaction setup
   - Message passing

3. **Storage Manager**
   - Chrome storage wrapper
   - State persistence
   - Session management

**Deliverables**:
- Working Chrome extension structure
- Message routing between components
- Storage integration

### Phase 4: Tool Interfaces (Week 4)

**Goal**: Define tool interfaces (implementation later)

1. **Tool Base Interface**
   ```typescript
   interface Tool {
     name: string
     execute(params: any): Promise<ToolResult>
   }
   ```

2. **Browser Tools** (interfaces only):
   - TabManager
   - PageInteractor
   - DataExtractor
   - Navigator

3. **Tool Registry**
   - Registration system
   - Discovery mechanism
   - Execution dispatch

**Deliverables**:
- `src/tools/` with interface definitions
- Tool registry implementation
- Stub implementations for testing

### Phase 5: UI Implementation (Week 5)

**Goal**: Create Svelte-based side panel

1. **Side Panel App**
   - Input interface
   - Event display
   - Status indicators

2. **Event Streaming**
   - Poll for events
   - Update UI reactively
   - Handle different event types

3. **User Interaction**
   - Submit operations
   - View responses
   - Control execution

**Deliverables**:
- Working side panel UI
- Event display system
- User input handling

### Phase 6: Integration (Week 6)

**Goal**: Connect all components

1. **Message Flow**
   - Side panel → Background → Content
   - Full SQ/EQ flow working
   - Error handling

2. **Testing**
   - Protocol tests
   - Integration tests
   - Chrome API mocks

3. **Build System**
   - Vite configuration
   - Multi-entry build
   - Asset handling

**Deliverables**:
- Full message flow working
- Test suite passing
- Buildable extension

## File Mapping

### Direct Ports from Rust

| Rust File | TypeScript File | Changes |
|-----------|----------------|---------|
| protocol/src/protocol.rs | src/protocol/types.ts | Exact type names |
| protocol/src/models.rs | src/protocol/models.ts | Exact type names |
| core/src/codex.rs (Codex) | src/core/CodexAgent.ts | Class syntax |
| core/src/codex.rs (Session) | src/core/Session.ts | Chrome adaptations |

### New Chrome-Specific Files

- `src/background/index.ts` - Service worker entry
- `src/sidepanel/App.svelte` - UI entry
- `src/content/index.ts` - Content script entry
- `src/tools/*.ts` - Browser tool interfaces

## Risk Mitigation

### Identified Risks

1. **Protocol Complexity**: 1,400+ lines of protocol types
   - Mitigation: Systematic line-by-line port
   - Use TypeScript code generation where possible

2. **Queue Architecture**: Async patterns differ from Rust
   - Mitigation: Use Chrome's message passing
   - Implement proper async/await patterns

3. **Tool Implementation**: Complex browser operations
   - Mitigation: Start with interfaces only
   - Implement incrementally in later phases

## Success Metrics

### Phase Gates

Each phase must meet criteria before proceeding:

1. **Protocol Port**: All types compile, match Rust exactly
2. **Core Agent**: SQ/EQ pattern working with test messages
3. **Chrome Infrastructure**: Extension loads, messages route
4. **Tool Interfaces**: All tools defined, registry working
5. **UI**: User can submit input, see events
6. **Integration**: End-to-end flow working

### Final Success Criteria

- [ ] Protocol types match Rust 100%
- [ ] SQ/EQ architecture preserved
- [ ] Basic query → response flow working
- [ ] Chrome extension installable
- [ ] Side panel functional
- [ ] Tool interfaces defined (not fully implemented)

## Timeline

- **Week 1**: Protocol port
- **Week 2**: Core agent
- **Week 3**: Chrome infrastructure
- **Week 4-5**: Tool interfaces (interfaces only, no implementation)
- **Week 5**: UI implementation
- **Week 6**: Integration and testing
- **Week 7**: Polish and documentation

## Next Steps

1. Create `codex-chrome` directory
2. Initialize TypeScript project
3. Begin protocol type port from `protocol.rs`
4. Set up Chrome manifest
5. Start with T001-T004 tasks

## Important Notes

1. **DO NOT** create fictional components (no "AgentSystem", "QueryProcessor", etc.)
2. **DO NOT** implement full tool logic yet - interfaces only
3. **DO** preserve exact type and function names from Rust
4. **DO** maintain the queue-based architecture
5. **DO** focus on message flow first, features later