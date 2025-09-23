# Codex Chrome Extension - Data Model (Based on Actual Protocol)

## Core Protocol Types (Direct from codex-rs)

### Queue Architecture

```typescript
// SQ (Submission Queue) - User requests
interface Submission {
  id: string;
  op: Op;
}

// EQ (Event Queue) - System responses
interface Event {
  id: string;
  msg: EventMsg;
}
```

### Operation Types (User → System)

```typescript
type Op =
  | { type: 'Interrupt' }
  | {
      type: 'UserInput';
      items: InputItem[];
    }
  | {
      type: 'UserTurn';
      items: InputItem[];
      cwd: string;
      approval_policy: AskForApproval;
      sandbox_policy: SandboxPolicy;
      model: string;
      effort?: ReasoningEffortConfig;
      summary: ReasoningSummaryConfig;
    }
  | {
      type: 'OverrideTurnContext';
      cwd?: string;
      approval_policy?: AskForApproval;
      sandbox_policy?: SandboxPolicy;
      model?: string;
      effort?: ReasoningEffortConfig | null;
      summary?: ReasoningSummaryConfig;
    }
  | {
      type: 'ExecApproval';
      id: string;
      decision: ReviewDecision;
    }
  | {
      type: 'PatchApproval';
      id: string;
      decision: ReviewDecision;
    }
  | {
      type: 'AddToHistory';
      text: string;
    }
  | {
      type: 'GetConversationPath';
    }
  | {
      type: 'GetInference';
    }
  | {
      type: 'GetMemoryUsage';
    }
  | {
      type: 'ListCustomPrompts';
    }
  | {
      type: 'GetExecLog';
      sessionId: string;
    };
```

### Event Types (System → User)

```typescript
type EventMsg =
  // Task Events
  | { type: 'TaskStarted'; data: TaskStartedEvent }
  | { type: 'TaskComplete'; data: TaskCompleteEvent }
  | { type: 'TurnAborted'; data: TurnAbortedEvent }

  // Agent Messages
  | { type: 'AgentMessage'; data: AgentMessageEvent }
  | { type: 'AgentMessageDelta'; data: AgentMessageDeltaEvent }

  // Agent Reasoning
  | { type: 'AgentReasoning'; data: AgentReasoningEvent }
  | { type: 'AgentReasoningDelta'; data: AgentReasoningDeltaEvent }
  | { type: 'AgentReasoningSectionBreak'; data: AgentReasoningSectionBreakEvent }
  | { type: 'AgentReasoningRawContentDelta'; data: AgentReasoningRawContentDeltaEvent }

  // Execution Events
  | { type: 'ExecCommandBegin'; data: ExecCommandBeginEvent }
  | { type: 'ExecCommandEnd'; data: ExecCommandEndEvent }
  | { type: 'ExecApprovalRequest'; data: ExecApprovalRequestEvent }
  | { type: 'ExecOutput'; data: ExecOutputEvent }
  | { type: 'ExecStdinRequest'; data: ExecStdinRequestEvent }

  // Patch Events
  | { type: 'PatchApplyBegin'; data: PatchApplyBeginEvent }
  | { type: 'PatchApplyEnd'; data: PatchApplyEndEvent }
  | { type: 'ApplyPatchApprovalRequest'; data: ApplyPatchApprovalRequestEvent }

  // File Events
  | { type: 'FileCreate'; data: FileCreateEvent }
  | { type: 'FileUpdate'; data: FileUpdateEvent }
  | { type: 'FileDelete'; data: FileDeleteEvent }

  // Plan Events
  | { type: 'PlanUpdate'; data: PlanUpdateEvent }

  // Info Events
  | { type: 'TokenCount'; data: TokenCountEvent }
  | { type: 'RateLimitSnapshot'; data: RateLimitSnapshotEvent }
  | { type: 'Error'; data: ErrorEvent }
  | { type: 'BackgroundEvent'; data: BackgroundEventEvent }

  // Response Events
  | { type: 'ConversationPathResponse'; data: ConversationPathResponseEvent }
  | { type: 'ListCustomPromptsResponse'; data: ListCustomPromptsResponseEvent }
  | { type: 'GetExecLogResponse'; data: GetExecLogResponseEvent };
```

### Core Data Structures

```typescript
// Input types
interface InputItem {
  type: 'text' | 'image' | 'clipboard' | 'context';
  content?: string;
  imageUrl?: string;
  path?: string;
  alt?: string;
}

// Turn context
interface TurnContext {
  cwd: string;
  approval_policy: AskForApproval;
  sandbox_policy: SandboxPolicy;
  model: string;
  effort?: ReasoningEffortConfig;
  summary: ReasoningSummaryConfig;
}

// Approval policies
type AskForApproval =
  | 'Never'
  | 'OnChange'
  | 'Always';

// Sandbox policies (adapted for browser)
type SandboxPolicy =
  | 'None'              // No restrictions
  | 'ReadOnly'          // Read-only access
  | 'TabWrite'          // Write to current tab only
  | 'AllTabsWrite';     // Write to all tabs

// Review decisions
type ReviewDecision =
  | 'approve'
  | 'reject'
  | 'request_change';

// Reasoning configuration
interface ReasoningEffortConfig {
  effort: 'low' | 'medium' | 'high';
}

interface ReasoningSummaryConfig {
  enabled: boolean;
}
```

### Event Data Structures

```typescript
// Task events
interface TaskStartedEvent {
  submission_id: string;
  turn_type: 'user' | 'review';
}

interface TaskCompleteEvent {
  submission_id: string;
}

interface TurnAbortedEvent {
  submission_id: string;
  reason: TurnAbortReason;
}

type TurnAbortReason =
  | 'user_interrupt'
  | 'automatic_abort'
  | 'error';

// Agent message events
interface AgentMessageEvent {
  message: string;
}

interface AgentMessageDeltaEvent {
  delta: string;
}

// Reasoning events
interface AgentReasoningEvent {
  content: string;
}

interface AgentReasoningDeltaEvent {
  delta: string;
}

// Execution events (adapted for browser)
interface ExecCommandBeginEvent {
  session_id: string;
  command: string;
  tab_id?: number;
  url?: string;
}

interface ExecCommandEndEvent {
  session_id: string;
  exit_code: number;
  duration_ms: number;
}

interface ExecOutputEvent {
  session_id: string;
  output: string;
  stream: 'stdout' | 'stderr';
}

// File events (adapted for browser storage)
interface FileCreateEvent {
  path: string;
  content: string;
  storage_type: 'local' | 'session' | 'sync';
}

interface FileUpdateEvent {
  path: string;
  old_content: string;
  new_content: string;
  storage_type: 'local' | 'session' | 'sync';
}

// Token usage
interface TokenCountEvent {
  total_tokens: number;
  prompt_tokens: number;
  completion_tokens: number;
  cached_tokens?: number;
}

// Error event
interface ErrorEvent {
  code: string;
  message: string;
  details?: any;
}
```

## Chrome Extension Specific Types

### Extension Architecture

```typescript
// Main agent class (replacing Codex struct)
interface CodexChromeAgent {
  submissionQueue: Submission[];
  eventQueue: Event[];
  session: ChromeSession;

  submitOperation(op: Op): Promise<void>;
  getNextEvent(): Promise<Event | null>;
  processSubmission(submission: Submission): Promise<void>;
}

// Chrome-specific session
interface ChromeSession {
  conversationId: string;
  turnContext: TurnContext;
  state: SessionState;
  tabManager: TabManager;
  storageManager: StorageManager;
  toolRegistry: ToolRegistry;
}

// Session state
interface SessionState {
  status: 'idle' | 'processing' | 'waiting_approval' | 'error';
  currentSubmission?: Submission;
  history: HistoryEntry[];
  activeTabs: chrome.tabs.Tab[];
}
```

### Chrome Message Types

```typescript
// Extension internal messages
interface ExtensionMessage {
  source: 'background' | 'content' | 'sidepanel' | 'popup';
  target: 'background' | 'content' | 'sidepanel' | 'popup';
  payload: Submission | Event | ChromeCommand;
}

// Chrome-specific commands
type ChromeCommand =
  | { type: 'OpenTab'; url: string; active?: boolean }
  | { type: 'CloseTab'; tabId: number }
  | { type: 'ExecuteScript'; tabId: number; code: string }
  | { type: 'CaptureTab'; tabId: number }
  | { type: 'ModifyDOM'; tabId: number; selector: string; action: DOMAction }
  | { type: 'ExtractData'; tabId: number; selector: string };

type DOMAction =
  | { type: 'click' }
  | { type: 'type'; text: string }
  | { type: 'submit' }
  | { type: 'scroll'; x: number; y: number };
```

### Tool System (Chrome Adapted)

```typescript
// Base tool interface (preserving structure)
interface Tool {
  name: string;
  description: string;
  parameters: ToolParameter[];

  execute(params: any): Promise<ToolResult>;
}

// Chrome-specific tools
interface ChromeTools {
  // Tab management
  openTab: Tool;
  closeTab: Tool;
  switchTab: Tool;
  getAllTabs: Tool;

  // Page interaction
  click: Tool;
  type: Tool;
  submit: Tool;
  scroll: Tool;
  screenshot: Tool;

  // Data extraction
  getText: Tool;
  getHTML: Tool;
  getAttribute: Tool;
  waitForElement: Tool;

  // Navigation
  navigate: Tool;
  goBack: Tool;
  goForward: Tool;
  refresh: Tool;
}

// Tool result
interface ToolResult {
  success: boolean;
  data?: any;
  error?: string;
}
```

### Storage Schema

```typescript
// Chrome storage structure
interface ChromeStorageSchema {
  // Local storage (persistent)
  local: {
    'codex.conversation': ConversationData;
    'codex.history': HistoryEntry[];
    'codex.settings': UserSettings;
  };

  // Session storage (temporary)
  session: {
    'codex.state': SessionState;
    'codex.queue.submissions': Submission[];
    'codex.queue.events': Event[];
  };

  // Sync storage (cross-device)
  sync: {
    'codex.preferences': UserPreferences;
    'codex.models': ModelConfig[];
  };
}

// Conversation data
interface ConversationData {
  id: string;
  messages: Message[];
  turnContext: TurnContext;
  createdAt: number;
  updatedAt: number;
}

// History entry (from protocol)
interface HistoryEntry {
  timestamp: number;
  text: string;
  type: 'user' | 'agent';
}

// User settings
interface UserSettings {
  defaultModel: string;
  approvalPolicy: AskForApproval;
  sandboxPolicy: SandboxPolicy;
  theme: 'light' | 'dark' | 'system';
  debugMode: boolean;
}
```

## Constants and Enums

```typescript
// Preserve protocol constants
export const USER_INSTRUCTIONS_OPEN_TAG = '<user_instructions>';
export const USER_INSTRUCTIONS_CLOSE_TAG = '</user_instructions>';
export const ENVIRONMENT_CONTEXT_OPEN_TAG = '<environment_context>';
export const ENVIRONMENT_CONTEXT_CLOSE_TAG = '</environment_context>';
export const USER_MESSAGE_BEGIN = '## My request for Codex:';

// Chrome extension specific
export const MAX_QUEUE_SIZE = 100;
export const EVENT_TIMEOUT_MS = 30000;
export const MAX_CONCURRENT_TABS = 20;
export const STORAGE_KEY_PREFIX = 'codex.';
```

## Type Guards and Validators

```typescript
// Type guards for runtime checking
function isSubmission(obj: any): obj is Submission {
  return obj && typeof obj.id === 'string' && obj.op;
}

function isEvent(obj: any): obj is Event {
  return obj && typeof obj.id === 'string' && obj.msg;
}

// Zod schemas for validation
import { z } from 'zod';

const SubmissionSchema = z.object({
  id: z.string(),
  op: z.discriminatedUnion('type', [
    z.object({ type: z.literal('Interrupt') }),
    z.object({
      type: z.literal('UserInput'),
      items: z.array(InputItemSchema)
    }),
    // ... other op types
  ])
});

const EventSchema = z.object({
  id: z.string(),
  msg: z.discriminatedUnion('type', [
    z.object({
      type: z.literal('TaskStarted'),
      data: TaskStartedEventSchema
    }),
    // ... other event types
  ])
});
```

## Migration Notes

### Key Naming Preservation
All type names from the Rust protocol are preserved exactly:
- `Submission`, `Op`, `Event`, `EventMsg`
- `InputItem`, `TurnContext`, `AskForApproval`
- All event types with exact same names

### Structural Changes for Chrome
1. File operations → Chrome storage operations
2. Shell execution → Tab script execution
3. Sandbox policies adapted for browser context
4. MCP tools optional (can add later)

### Queue Architecture Maintained
The SQ/EQ pattern is preserved with async message passing between:
- Side panel ↔ Background worker (via Chrome runtime messages)
- Background worker ↔ Content scripts (via Chrome tabs messages)
- Content scripts ↔ Web pages (via DOM injection)