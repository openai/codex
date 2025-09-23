# Chrome Extension API Contracts

## Background Service Worker API

### Query Processing Endpoint

```typescript
// Process user query from side panel
interface ProcessQueryRequest {
  method: 'agent.processQuery';
  params: {
    query: string;
    metadata: QueryMetadata;
    options?: {
      timeout?: number;
      streaming?: boolean;
    };
  };
}

interface ProcessQueryResponse {
  success: boolean;
  data?: {
    queryId: string;
    response: AgentResponse;
  };
  error?: {
    code: ErrorCode;
    message: string;
    context?: ErrorContext;
  };
}
```

### Tool Execution API

```typescript
// Execute a specific tool
interface ExecuteToolRequest {
  method: 'tool.execute';
  params: {
    toolName: string;
    toolParams: ToolParams;
    context?: ExecutionContext;
  };
}

interface ExecuteToolResponse {
  success: boolean;
  data?: ToolResponse;
  error?: {
    code: string;
    message: string;
  };
}

// List available tools
interface ListToolsRequest {
  method: 'tool.list';
  params?: {
    filter?: string;
    category?: ToolCategory;
  };
}

interface ListToolsResponse {
  tools: ToolInfo[];
}
```

### State Management API

```typescript
// Get current agent state
interface GetStateRequest {
  method: 'state.get';
  params?: {
    includeHistory?: boolean;
    includeMemory?: boolean;
  };
}

interface GetStateResponse {
  state: AgentState;
  history?: Query[];
  memory?: SerializedMemory;
}

// Update agent state
interface UpdateStateRequest {
  method: 'state.update';
  params: {
    updates: Partial<AgentState>;
  };
}
```

## Content Script API

### DOM Interaction

```typescript
// Click element on page
interface ClickElementRequest {
  action: 'click';
  selector: string;
  options?: {
    waitForSelector?: boolean;
    timeout?: number;
    scrollIntoView?: boolean;
  };
}

// Type text into element
interface TypeTextRequest {
  action: 'type';
  selector: string;
  text: string;
  options?: {
    clear?: boolean;
    delay?: number;
    pressEnter?: boolean;
  };
}

// Extract data from page
interface ExtractDataRequest {
  action: 'extract';
  selectors: {
    [key: string]: {
      selector: string;
      attribute?: string;
      all?: boolean;
    };
  };
}

interface ExtractDataResponse {
  data: Record<string, string | string[] | null>;
}
```

### Page State

```typescript
// Wait for element
interface WaitForElementRequest {
  action: 'waitForElement';
  selector: string;
  options?: {
    timeout?: number;
    visible?: boolean;
    hidden?: boolean;
  };
}

// Get page info
interface GetPageInfoRequest {
  action: 'getPageInfo';
}

interface GetPageInfoResponse {
  url: string;
  title: string;
  readyState: string;
  documentHeight: number;
  documentWidth: number;
}
```

## Side Panel API

### UI Commands

```typescript
// Send query from UI
interface SendQueryCommand {
  command: 'sendQuery';
  query: string;
  options?: {
    autoExecute?: boolean;
  };
}

// Cancel current operation
interface CancelOperationCommand {
  command: 'cancelOperation';
  operationId?: string;
}

// Clear history
interface ClearHistoryCommand {
  command: 'clearHistory';
  options?: {
    keepCurrent?: boolean;
  };
}
```

### UI Events

```typescript
// Query status update
interface QueryStatusEvent {
  event: 'queryStatus';
  queryId: string;
  status: 'pending' | 'processing' | 'completed' | 'failed';
  progress?: number;
  message?: string;
}

// Tool execution event
interface ToolExecutionEvent {
  event: 'toolExecution';
  tool: string;
  status: 'started' | 'completed' | 'failed';
  details?: any;
}

// Response stream event
interface ResponseStreamEvent {
  event: 'responseStream';
  queryId: string;
  chunk: string;
  isComplete: boolean;
}
```

## Chrome Extension Manifest APIs

### Permissions Required

```json
{
  "permissions": [
    "tabs",
    "activeTab",
    "storage",
    "scripting",
    "webNavigation",
    "contextMenus",
    "sidePanel"
  ],
  "host_permissions": [
    "<all_urls>"
  ],
  "optional_permissions": [
    "downloads",
    "bookmarks",
    "history"
  ]
}
```

### Service Worker Registration

```typescript
// Service worker lifecycle
interface ServiceWorkerLifecycle {
  onInstalled(details: chrome.runtime.InstalledDetails): void;
  onStartup(): void;
  onSuspend(): void;
  onMessage(message: ExtensionMessage, sender: chrome.runtime.MessageSender): Promise<any>;
}
```

## Message Passing Protocol

### Request/Response Pattern

```typescript
// Generic message structure
interface ExtensionMessage<T = any> {
  id: string;
  timestamp: number;
  source: MessageSource;
  target: MessageTarget;
  type: 'request' | 'response' | 'event';
  method?: string;
  params?: T;
  result?: any;
  error?: MessageError;
}

type MessageSource = 'background' | 'content' | 'sidepanel' | 'popup';
type MessageTarget = MessageSource | 'all';

interface MessageError {
  code: number;
  message: string;
  data?: any;
}

// Message handler
class MessageHandler {
  async handleMessage(message: ExtensionMessage): Promise<ExtensionMessage> {
    switch (message.method) {
      case 'agent.processQuery':
        return this.handleProcessQuery(message);
      case 'tool.execute':
        return this.handleToolExecute(message);
      // ... other handlers
    }
  }
}
```

### Event Subscription

```typescript
// Subscribe to events
interface EventSubscription {
  subscribe(event: string, callback: (data: any) => void): () => void;
  unsubscribe(event: string, callback?: (data: any) => void): void;
  emit(event: string, data: any): void;
}

// Event types
type EventType =
  | 'query.started'
  | 'query.completed'
  | 'tool.executed'
  | 'state.changed'
  | 'error.occurred';
```

## Tab Management API

```typescript
// Tab operations
interface TabOperations {
  create(options: {
    url?: string;
    active?: boolean;
    pinned?: boolean;
    index?: number;
  }): Promise<chrome.tabs.Tab>;

  update(tabId: number, options: {
    url?: string;
    active?: boolean;
    pinned?: boolean;
    muted?: boolean;
  }): Promise<chrome.tabs.Tab>;

  remove(tabIds: number | number[]): Promise<void>;

  query(options: {
    active?: boolean;
    currentWindow?: boolean;
    url?: string | string[];
    title?: string;
    status?: 'loading' | 'complete';
  }): Promise<chrome.tabs.Tab[]>;

  executeScript(tabId: number, injection: {
    code?: string;
    file?: string;
    allFrames?: boolean;
  }): Promise<any[]>;
}
```

## Storage API

```typescript
// Storage operations
interface StorageOperations {
  // Local storage
  local: {
    get<T>(keys: string | string[]): Promise<T>;
    set(items: Record<string, any>): Promise<void>;
    remove(keys: string | string[]): Promise<void>;
    clear(): Promise<void>;
  };

  // Session storage
  session: {
    get<T>(keys: string | string[]): Promise<T>;
    set(items: Record<string, any>): Promise<void>;
    remove(keys: string | string[]): Promise<void>;
    clear(): Promise<void>;
  };

  // Sync storage
  sync: {
    get<T>(keys: string | string[]): Promise<T>;
    set(items: Record<string, any>): Promise<void>;
    remove(keys: string | string[]): Promise<void>;
    clear(): Promise<void>;
  };
}
```

## Error Handling

```typescript
// Standard error responses
enum ErrorCode {
  INVALID_REQUEST = 1001,
  METHOD_NOT_FOUND = 1002,
  INVALID_PARAMS = 1003,
  INTERNAL_ERROR = 1004,
  TIMEOUT = 1005,
  PERMISSION_DENIED = 1006,
  RESOURCE_NOT_FOUND = 1007,
  RATE_LIMITED = 1008,
  NETWORK_ERROR = 1009
}

// Error handler
class ErrorHandler {
  static handle(error: Error): MessageError {
    if (error instanceof AgentError) {
      return {
        code: this.mapErrorCode(error.code),
        message: error.message,
        data: error.context
      };
    }

    return {
      code: ErrorCode.INTERNAL_ERROR,
      message: error.message || 'Unknown error occurred',
      data: null
    };
  }

  private static mapErrorCode(code: string): number {
    // Map internal error codes to API error codes
    const mapping: Record<string, number> = {
      'PARSE_ERROR': ErrorCode.INVALID_PARAMS,
      'TOOL_NOT_FOUND': ErrorCode.RESOURCE_NOT_FOUND,
      'TIMEOUT': ErrorCode.TIMEOUT,
      'PERMISSION_DENIED': ErrorCode.PERMISSION_DENIED
    };

    return mapping[code] || ErrorCode.INTERNAL_ERROR;
  }
}
```

## Rate Limiting

```typescript
// Rate limiter for API calls
interface RateLimiter {
  check(key: string): Promise<boolean>;
  consume(key: string, tokens?: number): Promise<void>;
  reset(key: string): Promise<void>;
}

// Rate limit configuration
interface RateLimitConfig {
  maxTokens: number;
  refillRate: number;
  windowMs: number;
}

const DEFAULT_RATE_LIMITS: Record<string, RateLimitConfig> = {
  'agent.processQuery': {
    maxTokens: 10,
    refillRate: 1,
    windowMs: 60000
  },
  'tool.execute': {
    maxTokens: 50,
    refillRate: 5,
    windowMs: 60000
  }
};
```