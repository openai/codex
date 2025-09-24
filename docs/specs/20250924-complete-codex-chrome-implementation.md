# Feature Specification: Complete Codex Chrome Implementation

## Overview
Complete the conversion of codex-rs terminal agent to Chrome extension by implementing the missing core functionality, particularly the `run_task()` method and its dependent systems including model client integration, tool execution, and turn management.

## Background
The codex-chrome extension has been partially converted from codex-rs, preserving the SQ/EQ (Submission Queue/Event Queue) architecture. However, the core agent logic that actually processes tasks and interacts with AI models is missing. The current implementation has:
- Protocol types and message passing infrastructure
- Chrome extension boilerplate (service worker, content script, side panel)
- Basic queue processing structure

What's missing is the actual task execution logic that makes the agent functional.

## Requirements

### Functional Requirements
- Implement `run_task()` equivalent functionality that processes user inputs through AI model
- Integrate model client for API communication (OpenAI/Anthropic/etc.)
- Implement tool execution system for browser operations
- Add turn management and conversation history
- Implement approval/review workflow for sensitive operations
- Add diff tracking for changes made during task execution
- Support streaming responses from AI models
- Handle task interruption and error recovery

### Non-Functional Requirements
- Maintain compatibility with Chrome Extension Manifest V3 security model
- Ensure all file/shell operations are replaced with browser-safe alternatives
- Preserve the SQ/EQ architecture from codex-rs
- Support asynchronous, non-blocking operations
- Implement proper error handling and retry logic
- Maintain type safety with TypeScript

## Technical Design

### Architecture
The complete implementation follows a layered architecture:

1. **Protocol Layer** ( Completed)
   - Types: Submission, Op, Event, EventMsg
   - Schemas and validation

2. **Core Agent Layer** (  Partially Complete)
   - CodexAgent class with queue management
   - Session management
   - Missing: Task execution logic

3. **Model Integration Layer** (L Missing)
   - Model client for API calls
   - Prompt construction
   - Response parsing
   - Token counting and usage tracking

4. **Tools Layer** (L Missing)
   - Browser operation tools (replacing shell/file tools)
   - Tool registration and discovery
   - Tool execution and result handling

5. **Chrome Extension Layer** ( Completed)
   - Background service worker
   - Content script
   - Side panel UI

### Components

#### Missing Core Components:

1. **ModelClient**
   - Purpose: Handle communication with AI model APIs
   - Responsibilities:
     - API authentication and configuration
     - Request/response formatting
     - Streaming support
     - Rate limiting and retry logic
     - Token usage tracking

2. **TaskRunner** (equivalent to run_task)
   - Purpose: Execute a complete task from user input to completion
   - Responsibilities:
     - Build prompt with context
     - Call model API
     - Process tool calls
     - Handle approvals
     - Manage conversation turns
     - Track changes/diffs

3. **TurnManager** (equivalent to run_turn)
   - Purpose: Handle individual conversation turns
   - Responsibilities:
     - Construct turn input
     - Execute model call
     - Process response
     - Handle tool invocations
     - Update history

4. **ToolsRegistry**
   - Purpose: Manage available tools for browser operations
   - Responsibilities:
     - Register browser-specific tools
     - Tool discovery and listing
     - Tool validation
     - Tool execution dispatch

5. **BrowserTools** (replacing shell/file tools)
   - Tab manipulation (create, close, navigate)
   - DOM interaction (click, type, extract)
   - Storage operations (local/session storage)
   - Network inspection
   - Screenshot capture
   - Clipboard operations

6. **ApprovalManager**
   - Purpose: Handle user approval for sensitive operations
   - Responsibilities:
     - Queue approval requests
     - Display approval UI
     - Process approval decisions
     - Enforce sandbox policies

7. **DiffTracker**
   - Purpose: Track changes made during task execution
   - Responsibilities:
     - Monitor DOM changes
     - Track storage modifications
     - Record navigation history
     - Generate change summaries

### Data Model

```typescript
// Model Integration Types
interface ModelClient {
  provider: ModelProvider;
  apiKey: string;
  model: string;
  temperature: number;
  maxTokens: number;
  streamingEnabled: boolean;
}

interface ModelProvider {
  type: 'openai' | 'anthropic' | 'google' | 'local';
  endpoint: string;
  headers: Record<string, string>;
}

interface TurnContext {
  client: ModelClient;
  conversationHistory: ResponseItem[];
  tools: Tool[];
  approvalPolicy: ApprovalPolicy;
  sandboxPolicy: SandboxPolicy;
}

interface Tool {
  name: string;
  description: string;
  parameters: JsonSchema;
  execute: (params: any) => Promise<ToolResult>;
}

interface ToolResult {
  success: boolean;
  output?: any;
  error?: string;
}

interface TaskExecution {
  taskId: string;
  status: 'running' | 'completed' | 'failed' | 'interrupted';
  turns: Turn[];
  totalTokens: number;
  changes: DiffSummary;
}

interface Turn {
  input: ResponseItem[];
  output: ResponseItem[];
  toolCalls: ToolCall[];
  tokenUsage: TokenUsage;
}
```

## Implementation Plan

### Phase 1: Model Integration
- Create ModelClient class for API communication
- Implement API providers (OpenAI, Anthropic)
- Add streaming response support
- Implement token counting and usage tracking
- Add retry logic with exponential backoff

### Phase 2: Task Execution Core
- Implement TaskRunner class (run_task equivalent)
- Implement TurnManager class (run_turn equivalent)
- Add prompt construction logic
- Implement conversation history management
- Add turn-based execution flow

### Phase 3: Browser Tools System
- Create ToolsRegistry for tool management
- Implement browser-specific tools:
  - TabTool (navigate, create, close tabs)
  - DOMTool (click, type, extract data)
  - StorageTool (read/write browser storage)
  - NetworkTool (inspect requests)
  - ScreenshotTool (capture screenshots)
- Add tool validation and error handling

### Phase 4: Approval and Safety
- Implement ApprovalManager
- Create approval UI in side panel
- Add sandbox policy enforcement
- Implement sensitive operation detection
- Add user confirmation flows

### Phase 5: Change Tracking
- Implement DiffTracker
- Add DOM mutation monitoring
- Track storage changes
- Generate change summaries
- Add undo/rollback capability

### Phase 6: Integration and Testing
- Wire TaskRunner into CodexAgent
- Update message router for new events
- Add comprehensive error handling
- Implement task interruption
- Add integration tests

## Testing Strategy

1. **Unit Tests**
   - Test each component in isolation
   - Mock API responses
   - Test error conditions
   - Validate type safety

2. **Integration Tests**
   - Test complete task execution flow
   - Test tool invocation chains
   - Test approval workflows
   - Test error recovery

3. **End-to-End Tests**
   - Test real browser operations
   - Test with actual API calls (test mode)
   - Test user interaction flows
   - Test extension installation and setup

4. **Performance Tests**
   - Measure response latency
   - Test streaming performance
   - Monitor memory usage
   - Test with large conversation histories

## Success Criteria

1. **Functional Completeness**
   - Can execute user tasks through AI model
   - All browser tools are functional
   - Approval system works correctly
   - Change tracking is accurate

2. **Performance Metrics**
   - First response within 2 seconds
   - Streaming updates at 30+ tokens/second
   - Memory usage under 100MB
   - No blocking of browser UI thread

3. **Reliability**
   - 99% task completion rate
   - Proper error recovery
   - No data loss on interruption
   - Graceful degradation on API failures

4. **User Experience**
   - Clear task progress indication
   - Responsive UI during execution
   - Meaningful error messages
   - Intuitive approval flows

5. **Security**
   - All sensitive operations require approval
   - Sandbox policies are enforced
   - No exposure of API keys
   - No unauthorized browser access