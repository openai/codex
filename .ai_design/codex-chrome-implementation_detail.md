# Codex Chrome Implementation Detail

## Executive Summary

This document provides implementation details for completing the codex-chrome extension, which is a partial conversion from the codex-rs terminal agent. The existing skeleton has ~3,275 lines of code with the SQ/EQ architecture, protocol types, and Chrome extension infrastructure already in place. This document focuses on implementing the **missing core functionality** that makes the agent operational.

## Current State Assessment

### ✅ Already Implemented (Existing Skeleton)
- **Protocol Layer**: Types, schemas, guards, events (preserving exact Rust names)
- **Queue Architecture**: SQ/EQ message passing system
- **Chrome Infrastructure**: Manifest V3, service worker, content script, side panel
- **Basic Agent Shell**: CodexAgent, Session, MessageRouter, QueueProcessor classes
- **UI Foundation**: Svelte-based side panel with Tailwind CSS
- **Build System**: Vite configuration, TypeScript setup

### ❌ Missing Components (Need Implementation)
1. **Model Client Integration** - No API communication with AI providers
2. **Task Execution Logic** - `run_task()` equivalent not implemented
3. **Turn Management** - `run_turn()` equivalent missing
4. **Browser Tools** - No actual tool implementations
5. **Approval System** - No user consent mechanism
6. **Diff Tracking** - No change monitoring
7. **Integration Wiring** - Components not connected

## Implementation Tasks

### Task Group 1: Model Client Layer [Priority: HIGH]

#### Task 1.1: Create Model Client Base Architecture
**Files to create:**
- `codex-chrome/src/models/types.ts` - Model client interfaces
- `codex-chrome/src/models/ModelClient.ts` - Base abstract class

**Implementation:**
```typescript
// types.ts
export interface CompletionRequest {
  model: string;
  messages: Message[];
  temperature?: number;
  maxTokens?: number;
  tools?: ToolDefinition[];
  stream?: boolean;
}

export interface ModelClient {
  complete(request: CompletionRequest): Promise<CompletionResponse>;
  stream(request: CompletionRequest): AsyncGenerator<StreamChunk>;
  countTokens(text: string, model: string): number;
}
```

#### Task 1.2: Implement OpenAI Client
**Files to create:**
- `codex-chrome/src/models/OpenAIClient.ts`

**Features:**
- API key management from Chrome storage
- Complete and streaming responses
- Tool/function calling support
- Token counting with tiktoken
- Retry logic with exponential backoff

#### Task 1.3: Implement Anthropic Client
**Files to create:**
- `codex-chrome/src/models/AnthropicClient.ts`

**Features:**
- Claude API integration
- Content blocks handling
- Tool use support
- System prompts
- Streaming support

#### Task 1.4: Model Client Factory
**Files to create:**
- `codex-chrome/src/models/ModelClientFactory.ts`

**Features:**
- Dynamic client selection based on config
- Caching of client instances
- API key loading from storage
- Provider switching

### Task Group 2: Task Execution Core [Priority: HIGH]

#### Task 2.1: Implement TaskRunner
**Files to create:**
- `codex-chrome/src/core/TaskRunner.ts`

**Implementation outline:**
```typescript
export class TaskRunner {
  constructor(
    private session: Session,
    private modelClient: ModelClient,
    private toolRegistry: ToolRegistry
  ) {}

  async execute(submission: Submission): Promise<TaskResult> {
    // 1. Initialize task context
    // 2. Build initial prompt
    // 3. Execute conversation turns
    // 4. Handle tool calls
    // 5. Track changes
    // 6. Return results
  }

  async interrupt(): Promise<void> {
    // Handle task interruption
  }
}
```

**Key responsibilities:**
- Port `run_task` logic from codex-rs
- Manage conversation flow
- Emit progress events to EQ
- Handle errors and retries

#### Task 2.2: Implement TurnManager
**Files to create:**
- `codex-chrome/src/core/TurnManager.ts`

**Features:**
- Single turn execution
- Tool call processing
- Response streaming
- History management
- Context building

#### Task 2.3: Turn Context Management
**Files to create:**
- `codex-chrome/src/core/TurnContext.ts`

**Features:**
- Conversation state tracking
- Context window management
- Token budget tracking
- History pruning

### Task Group 3: Browser Tools System [Priority: HIGH]

#### Task 3.1: Tool Registry Implementation
**Files to create:**
- `codex-chrome/src/tools/ToolRegistry.ts`
- `codex-chrome/src/tools/types.ts`

**Implementation:**
```typescript
export class ToolRegistry {
  private tools: Map<string, Tool> = new Map();

  register(tool: Tool): void {
    this.tools.set(tool.name, tool);
  }

  async execute(name: string, params: any): Promise<ToolResult> {
    const tool = this.tools.get(name);
    if (!tool) throw new Error(`Tool not found: ${name}`);
    return await tool.execute(params);
  }

  getToolDefinitions(): ToolDefinition[] {
    // Return OpenAI-format tool definitions
  }
}
```

#### Task 3.2: Base Tool Implementation
**Files to create:**
- `codex-chrome/src/tools/BaseTool.ts`

**Features:**
- Abstract base class
- Parameter validation
- Error handling
- Result formatting

#### Task 3.3: Browser-Specific Tools
**Files to create:**
- `codex-chrome/src/tools/TabTool.ts` - Tab management
- `codex-chrome/src/tools/DOMTool.ts` - DOM interaction
- `codex-chrome/src/tools/StorageTool.ts` - Browser storage
- `codex-chrome/src/tools/NavigationTool.ts` - Page navigation
- `codex-chrome/src/tools/ScreenshotTool.ts` - Screen capture

**TabTool implementation example:**
```typescript
export class TabTool extends BaseTool {
  name = 'browser_tab';

  async execute(params: TabParams): Promise<ToolResult> {
    switch (params.action) {
      case 'create':
        const tab = await chrome.tabs.create({ url: params.url });
        return { success: true, data: tab };
      case 'close':
        await chrome.tabs.remove(params.tabId);
        return { success: true };
      // ... other actions
    }
  }
}
```

### Task Group 4: Approval System [Priority: MEDIUM]

#### Task 4.1: Approval Manager
**Files to create:**
- `codex-chrome/src/core/ApprovalManager.ts`

**Features:**
- Queue approval requests
- Apply approval policies
- Handle timeouts
- Store decisions

#### Task 4.2: Approval UI Components
**Files to create:**
- `codex-chrome/src/sidepanel/components/ApprovalDialog.svelte`
- `codex-chrome/src/sidepanel/components/ApprovalQueue.svelte`

**Features:**
- Display pending approvals
- Show operation details
- Risk level indicators
- Accept/reject controls

### Task Group 5: Diff Tracking [Priority: MEDIUM]

#### Task 5.1: Diff Tracker Implementation
**Files to create:**
- `codex-chrome/src/core/DiffTracker.ts`

**Features:**
- DOM mutation observer
- Storage change tracking
- Navigation history
- Change summaries
- Rollback support

#### Task 5.2: Diff UI Components
**Files to create:**
- `codex-chrome/src/sidepanel/components/DiffViewer.svelte`
- `codex-chrome/src/sidepanel/components/ChangeHistory.svelte`

### Task Group 6: Integration & Wiring [Priority: HIGH]

#### Task 6.1: Update CodexAgent
**Files to modify:**
- `codex-chrome/src/core/CodexAgent.ts`

**Changes needed:**
```typescript
// Add to handleUserTurn method:
private async handleUserTurn(op: UserTurnOp): Promise<void> {
  // Create TaskRunner with model client
  const modelClient = await ModelClientFactory.create(op.model);
  const taskRunner = new TaskRunner(this.session, modelClient, this.toolRegistry);

  // Execute task
  const result = await taskRunner.execute(submission);

  // Emit completion event
  this.emitEvent({
    type: 'TaskComplete',
    data: { submission_id: submission.id }
  });
}
```

#### Task 6.2: Update Session Management
**Files to modify:**
- `codex-chrome/src/core/Session.ts`

**Changes:**
- Initialize ToolRegistry with browser tools
- Add conversation history management
- Store turn context

#### Task 6.3: Update Background Service Worker
**Files to modify:**
- `codex-chrome/src/background/index.ts`

**Changes:**
- Initialize model client factory
- Setup approval manager
- Start diff tracker
- Wire message routing

#### Task 6.4: Update Message Router
**Files to modify:**
- `codex-chrome/src/core/MessageRouter.ts`

**New routes to add:**
- Tool execution requests/responses
- Approval requests/decisions
- Diff tracking events
- Model streaming updates

### Task Group 7: Testing & Quality [Priority: MEDIUM]

#### Task 7.1: Unit Tests
**Files to create:**
- `codex-chrome/src/tests/models/*.test.ts`
- `codex-chrome/src/tests/core/*.test.ts`
- `codex-chrome/src/tests/tools/*.test.ts`

#### Task 7.2: Integration Tests
**Files to create:**
- `codex-chrome/src/tests/integration/task-execution.test.ts`
- `codex-chrome/src/tests/integration/tool-chain.test.ts`

#### Task 7.3: Chrome API Mocks
**Files to create:**
- `codex-chrome/src/tests/mocks/chrome.ts`

## Implementation Phases

### Phase 1: Foundation (Week 1)
1. Model client base architecture
2. OpenAI and Anthropic clients
3. Basic TaskRunner structure
4. Tool registry and base tool

### Phase 2: Core Features (Week 2)
1. Complete TaskRunner implementation
2. TurnManager implementation
3. Browser tool implementations
4. Integration with CodexAgent

### Phase 3: Enhanced Features (Week 3)
1. Approval system
2. Diff tracking
3. UI components for approvals and diffs
4. Error handling and recovery

### Phase 4: Polish & Testing (Week 4)
1. Comprehensive testing
2. Performance optimization
3. Documentation
4. Bug fixes and refinements

## Key Implementation Considerations

### 1. Preserving SQ/EQ Architecture
- All operations go through submission queue
- All responses emit events to event queue
- No direct returns from async operations
- Maintain message-passing paradigm

### 2. Browser vs Terminal Adaptations
- Replace file operations with Chrome storage API
- Replace shell commands with browser operations
- Adapt sandbox policies to browser security model
- Use Chrome extension messaging instead of IPC

### 3. Type Safety
- Use exact type names from codex-rs protocol
- Leverage TypeScript strict mode
- Add runtime validation with Zod schemas
- Maintain type guards for message passing

### 4. Performance Optimization
- Stream model responses for better UX
- Lazy load tools and models
- Cache API clients
- Use Chrome storage efficiently

### 5. Security Considerations
- Store API keys in chrome.storage.local
- Never expose keys to content scripts
- Validate all tool parameters
- Enforce approval policies strictly

## Success Metrics

### Functional Completeness
- [ ] Can process user inputs through AI model
- [ ] All browser tools are functional
- [ ] Approval system works
- [ ] Changes are tracked
- [ ] Can interrupt tasks
- [ ] Error recovery works

### Performance Targets
- First token latency: < 2 seconds
- Streaming rate: > 30 tokens/second
- Memory usage: < 100MB
- No UI thread blocking

### Code Quality
- TypeScript strict mode enabled
- > 80% test coverage
- No use of `any` type
- All async operations have error handling

## Dependencies

### External Libraries Needed
```json
{
  "dependencies": {
    "openai": "^4.0.0",           // OpenAI client
    "@anthropic-ai/sdk": "^0.x",  // Anthropic client
    "tiktoken": "^1.0.0",          // Token counting
    "diff": "^5.0.0",              // Diff generation
    "nanoid": "^5.0.0"             // ID generation
  }
}
```

### Chrome APIs Required
- chrome.tabs.*
- chrome.storage.*
- chrome.runtime.*
- chrome.scripting.*
- chrome.action.*

## File Structure After Implementation

```
codex-chrome/
├── src/
│   ├── models/              [NEW]
│   │   ├── types.ts
│   │   ├── ModelClient.ts
│   │   ├── OpenAIClient.ts
│   │   ├── AnthropicClient.ts
│   │   └── ModelClientFactory.ts
│   │
│   ├── core/                [UPDATED]
│   │   ├── CodexAgent.ts    (modified)
│   │   ├── Session.ts       (modified)
│   │   ├── TaskRunner.ts    [NEW]
│   │   ├── TurnManager.ts   [NEW]
│   │   ├── TurnContext.ts   [NEW]
│   │   ├── ApprovalManager.ts [NEW]
│   │   └── DiffTracker.ts   [NEW]
│   │
│   ├── tools/               [NEW]
│   │   ├── types.ts
│   │   ├── ToolRegistry.ts
│   │   ├── BaseTool.ts
│   │   ├── TabTool.ts
│   │   ├── DOMTool.ts
│   │   ├── StorageTool.ts
│   │   ├── NavigationTool.ts
│   │   └── ScreenshotTool.ts
│   │
│   ├── sidepanel/           [UPDATED]
│   │   └── components/
│   │       ├── ApprovalDialog.svelte [NEW]
│   │       ├── DiffViewer.svelte    [NEW]
│   │       └── ToolExecution.svelte [NEW]
│   │
│   └── tests/               [NEW]
│       ├── models/
│       ├── core/
│       ├── tools/
│       └── integration/
```

## Next Steps

1. **Immediate Priority**: Implement Model Client layer (Tasks 1.1-1.4)
2. **Second Priority**: Implement TaskRunner and TurnManager (Tasks 2.1-2.3)
3. **Third Priority**: Implement core browser tools (Tasks 3.1-3.3)
4. **Integration**: Wire components together (Tasks 6.1-6.4)
5. **Testing**: Add comprehensive tests throughout development

## Conclusion

This implementation plan focuses on completing the missing 40% of the codex-chrome extension. The existing skeleton provides a solid foundation with the SQ/EQ architecture and Chrome extension infrastructure. By implementing the components outlined in this document, the extension will become a fully functional AI agent capable of browser automation while preserving the architectural patterns from the original codex-rs implementation.