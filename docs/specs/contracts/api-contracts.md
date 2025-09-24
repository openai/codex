# API Contracts: Codex Chrome Extension

## 1. ModelClient API

### OpenAI Provider

```typescript
interface OpenAIClient {
  // Initialize client
  constructor(config: {
    apiKey: string;
    organization?: string;
    baseURL?: string;
    timeout?: number;
    maxRetries?: number;
  });

  // Send completion request
  async complete(request: CompletionRequest): Promise<CompletionResponse>;

  // Stream completion
  async *stream(request: CompletionRequest): AsyncGenerator<StreamChunk>;

  // Count tokens
  countTokens(text: string, model: string): number;
}

interface CompletionRequest {
  model: string;
  messages: Message[];
  temperature?: number;
  maxTokens?: number;
  tools?: ToolDefinition[];
  toolChoice?: 'auto' | 'none' | ToolChoice;
  stream?: boolean;
  user?: string;
}

interface CompletionResponse {
  id: string;
  object: 'chat.completion';
  created: number;
  model: string;
  choices: Choice[];
  usage: Usage;
}

interface Choice {
  index: number;
  message: Message;
  finishReason: 'stop' | 'length' | 'tool_calls' | 'content_filter';
}

interface Message {
  role: 'system' | 'user' | 'assistant' | 'tool';
  content: string | null;
  toolCalls?: ToolCall[];
  toolCallId?: string;
}

interface ToolCall {
  id: string;
  type: 'function';
  function: {
    name: string;
    arguments: string; // JSON string
  };
}
```

### Anthropic Provider

```typescript
interface AnthropicClient {
  constructor(config: {
    apiKey: string;
    baseURL?: string;
    version?: string;
    maxRetries?: number;
  });

  async complete(request: AnthropicRequest): Promise<AnthropicResponse>;
  async *stream(request: AnthropicRequest): AsyncGenerator<AnthropicStreamChunk>;
}

interface AnthropicRequest {
  model: string;
  messages: AnthropicMessage[];
  maxTokens: number;
  temperature?: number;
  system?: string;
  tools?: AnthropicTool[];
  stream?: boolean;
}

interface AnthropicResponse {
  id: string;
  type: 'message';
  role: 'assistant';
  content: ContentBlock[];
  model: string;
  usage: {
    inputTokens: number;
    outputTokens: number;
  };
}

interface ContentBlock {
  type: 'text' | 'tool_use';
  text?: string;
  id?: string;
  name?: string;
  input?: any;
}
```

## 2. TaskRunner API

```typescript
interface TaskRunner {
  constructor(session: ChromeSession, modelClient: ModelClient);

  // Main execution method
  async execute(submission: Submission): Promise<TaskResult>;

  // Interrupt running task
  async interrupt(): Promise<void>;

  // Get task status
  getStatus(): TaskStatus;

  // Get execution history
  getHistory(): TaskExecution[];
}

interface TaskResult {
  success: boolean;
  finalMessage?: string;
  turns: Turn[];
  totalUsage: TokenUsage;
  changes: ChangeRecord[];
  error?: TaskError;
}

interface TaskStatus {
  isRunning: boolean;
  currentTurn?: number;
  phase?: 'initializing' | 'prompting' | 'tool_execution' | 'finalizing';
  lastActivity?: number;
}
```

## 3. TurnManager API

```typescript
interface TurnManager {
  constructor(modelClient: ModelClient, toolRegistry: ToolRegistry);

  // Execute a single turn
  async executeTurn(input: TurnInput): Promise<TurnOutput>;

  // Build prompt from history
  buildPrompt(history: Message[], tools: Tool[]): string;

  // Process tool calls
  async processToolCalls(toolCalls: ToolCall[]): Promise<ToolResult[]>;

  // Handle retries
  async retryWithBackoff<T>(
    operation: () => Promise<T>,
    maxRetries: number
  ): Promise<T>;
}

interface TurnInput {
  messages: Message[];
  tools: Tool[];
  context: TurnContext;
  retryCount: number;
}

interface TurnOutput {
  message: Message;
  toolResults?: ToolResult[];
  usage: TokenUsage;
  duration: number;
}
```

## 4. ToolRegistry API

```typescript
interface ToolRegistry {
  // Register a new tool
  register(tool: Tool): void;

  // Get tool by name
  getTool(name: string): Tool | undefined;

  // List all tools
  listTools(): Tool[];

  // Execute tool
  async execute(
    toolName: string,
    parameters: any,
    context: ToolContext
  ): Promise<ToolResult>;

  // Validate parameters
  validateParameters(
    toolName: string,
    parameters: any
  ): ValidationResult;
}

interface Tool {
  name: string;
  description: string;
  parameters: JsonSchema;
  category: ToolCategory;
  requiresApproval: boolean;

  // Execute function
  execute(params: any, context: ToolContext): Promise<ToolResult>;
}

interface ToolContext {
  tabId?: number;
  frameId?: number;
  approvalPolicy: ApprovalPolicy;
  sandboxPolicy: SandboxPolicy;
}

interface ValidationResult {
  valid: boolean;
  errors?: string[];
}
```

## 5. Browser Tools API

### TabTool

```typescript
interface TabTool extends Tool {
  name: 'browser_tab';

  async execute(params: TabParams): Promise<TabResult>;
}

interface TabParams {
  action: 'create' | 'close' | 'navigate' | 'reload' | 'capture';
  url?: string;
  tabId?: number;
  active?: boolean;
}

interface TabResult extends ToolResult {
  data?: {
    tabId?: number;
    url?: string;
    title?: string;
    screenshot?: string; // base64
  };
}
```

### DOMTool

```typescript
interface DOMTool extends Tool {
  name: 'browser_dom';

  async execute(params: DOMParams): Promise<DOMResult>;
}

interface DOMParams {
  action: 'click' | 'type' | 'submit' | 'extract' | 'wait';
  selector: string;
  value?: string;
  attributes?: string[];
  timeout?: number;
}

interface DOMResult extends ToolResult {
  data?: {
    text?: string;
    html?: string;
    attributes?: Record<string, string>;
    found?: boolean;
  };
}
```

### StorageTool

```typescript
interface StorageTool extends Tool {
  name: 'browser_storage';

  async execute(params: StorageParams): Promise<StorageResult>;
}

interface StorageParams {
  action: 'get' | 'set' | 'remove' | 'clear';
  storageType: 'local' | 'session' | 'sync';
  key?: string;
  value?: any;
}

interface StorageResult extends ToolResult {
  data?: {
    value?: any;
    keys?: string[];
    bytesUsed?: number;
  };
}
```

## 6. ApprovalManager API

```typescript
interface ApprovalManager {
  // Request approval
  async requestApproval(
    request: ApprovalRequest
  ): Promise<ApprovalDecision>;

  // Check if tool requires approval
  requiresApproval(
    toolName: string,
    params: any
  ): boolean;

  // Get approval history
  getHistory(): ApprovalDecision[];

  // Update policy
  updatePolicy(policy: ApprovalPolicy): void;
}

interface ApprovalRequest {
  id: string;
  toolName: string;
  parameters: any;
  risk: RiskLevel;
  description: string;
  timeout?: number;
}

interface ApprovalDecision {
  requestId: string;
  decision: 'approve' | 'reject' | 'always_allow' | 'always_deny';
  timestamp: number;
  reason?: string;
}

type RiskLevel = 'low' | 'medium' | 'high' | 'critical';
```

## 7. DiffTracker API

```typescript
interface DiffTracker {
  // Start tracking
  startTracking(sessionId: string): void;

  // Record change
  recordChange(change: ChangeRecord): void;

  // Get changes
  getChanges(): ChangeRecord[];

  // Generate summary
  generateSummary(): ChangeSummary;

  // Rollback change
  async rollback(changeId: string): Promise<boolean>;

  // Clear history
  clear(): void;
}

interface ChangeRecord {
  id: string;
  type: ChangeType;
  target: ChangeTarget;
  before?: any;
  after?: any;
  timestamp: number;
  reversible: boolean;
}

type ChangeType =
  | 'dom_modification'
  | 'navigation'
  | 'storage_write'
  | 'tab_operation'
  | 'network_request';

interface ChangeSummary {
  totalChanges: number;
  byType: Record<ChangeType, number>;
  timeline: TimelineEntry[];
  affectedResources: string[];
}
```

## 8. Chrome Extension Messages API

```typescript
// Background → Content Script
interface BackgroundToContent {
  type: 'EXECUTE_TOOL';
  tool: string;
  params: any;
  requestId: string;
}

// Content Script → Background
interface ContentToBackground {
  type: 'TOOL_RESULT';
  requestId: string;
  result: ToolResult;
}

// Background → Side Panel
interface BackgroundToPanel {
  type: 'EVENT';
  event: Event;
}

// Side Panel → Background
interface PanelToBackground {
  type: 'SUBMISSION';
  submission: Submission;
}

// Message Handler
type MessageHandler<T, R> = (
  message: T,
  sender: chrome.runtime.MessageSender
) => Promise<R> | R;

// Message Router
interface MessageRouter {
  on<T, R>(
    type: string,
    handler: MessageHandler<T, R>
  ): void;

  send<T, R>(
    target: 'background' | 'content' | 'panel',
    message: T
  ): Promise<R>;

  broadcast<T>(message: T): void;
}
```

## 9. Storage API

```typescript
interface StorageManager {
  // Conversation storage
  async saveConversation(
    id: string,
    data: ConversationData
  ): Promise<void>;

  async loadConversation(
    id: string
  ): Promise<ConversationData | null>;

  async listConversations(): Promise<ConversationSummary[]>;

  // Settings storage
  async saveSettings(settings: UserSettings): Promise<void>;
  async loadSettings(): Promise<UserSettings>;

  // Queue persistence
  async saveQueues(queues: {
    submissions: Submission[];
    events: Event[];
  }): Promise<void>;

  async loadQueues(): Promise<{
    submissions: Submission[];
    events: Event[];
  }>;

  // Clear data
  async clearAll(): Promise<void>;
}

interface ConversationSummary {
  id: string;
  title: string;
  lastMessage: string;
  timestamp: number;
  messageCount: number;
}
```

## 10. Error Handling

```typescript
// Base error class
class CodexError extends Error {
  constructor(
    public code: string,
    message: string,
    public details?: any
  ) {
    super(message);
  }
}

// Specific error types
class ModelError extends CodexError {
  constructor(message: string, details?: any) {
    super('MODEL_ERROR', message, details);
  }
}

class ToolError extends CodexError {
  constructor(toolName: string, message: string, details?: any) {
    super('TOOL_ERROR', `Tool ${toolName}: ${message}`, details);
  }
}

class ApprovalError extends CodexError {
  constructor(message: string, details?: any) {
    super('APPROVAL_ERROR', message, details);
  }
}

class StorageError extends CodexError {
  constructor(message: string, details?: any) {
    super('STORAGE_ERROR', message, details);
  }
}

// Error handler
interface ErrorHandler {
  handle(error: Error): void;
  report(error: Error): Promise<void>;
  getHistory(): ErrorRecord[];
}

interface ErrorRecord {
  timestamp: number;
  error: Error;
  context?: any;
  handled: boolean;
}
```