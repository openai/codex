# Codex Chrome Extension - Executable Tasks

## Feature: Codex Chrome Extension
**Goal**: Convert codex-rs terminal agent to Chrome extension preserving SQ/EQ architecture

## Tech Stack
- TypeScript 5.x (strict mode)
- Svelte 4.x + Tailwind CSS
- Vite (build system)
- Chrome Manifest V3
- Zod (validation)
- Vitest (testing)
- pnpm (package manager)

## Task Execution Guide

Tasks marked [P] can run in parallel. Use Task agent for parallel execution:
```bash
Task "T010" && Task "T011" && Task "T012"  # Run parallel tasks
```

---

## Phase 1: Setup Tasks (Sequential)

### T001: Initialize Chrome Extension Project
**File**: `codex-chrome/package.json`
**Commands**:
```bash
mkdir -p codex-chrome
cd codex-chrome
pnpm init
pnpm add -D typescript@5.x vite@5.x @types/chrome @vitejs/plugin-react
pnpm add -D @sveltejs/vite-plugin-svelte svelte tailwindcss postcss autoprefixer
pnpm add zod uuid chrome-types
```
Create package.json with scripts: dev, build, test, type-check

### T002: Configure TypeScript
**File**: `codex-chrome/tsconfig.json`
**Deps**: T001
```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ESNext",
    "lib": ["ES2020", "DOM", "chrome"],
    "strict": true,
    "esModuleInterop": true,
    "skipLibCheck": true,
    "resolveJsonModule": true,
    "moduleResolution": "node",
    "allowSyntheticDefaultImports": true,
    "types": ["chrome", "vite/client", "svelte"]
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist"]
}
```

### T003: Create Chrome Manifest V3
**File**: `codex-chrome/manifest.json`
**Deps**: T001
```json
{
  "manifest_version": 3,
  "name": "Codex Chrome Agent",
  "version": "0.1.0",
  "permissions": ["tabs", "activeTab", "storage", "scripting", "webNavigation"],
  "host_permissions": ["<all_urls>"],
  "background": {"service_worker": "dist/background/index.js", "type": "module"},
  "action": {"default_title": "Codex Agent"},
  "side_panel": {"default_path": "dist/sidepanel/index.html"},
  "content_scripts": [{"matches": ["<all_urls>"], "js": ["dist/content/index.js"]}]
}
```

### T004: Setup Vite Configuration
**File**: `codex-chrome/vite.config.ts`
**Deps**: T001, T002
```typescript
import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
export default defineConfig({
  plugins: [svelte()],
  build: {
    rollupOptions: {
      input: {
        background: 'src/background/index.ts',
        content: 'src/content/index.ts',
        sidepanel: 'src/sidepanel/index.html'
      },
      output: { dir: 'dist', format: 'es' }
    }
  }
})
```

### T005: Create Directory Structure
**Commands**:
```bash
cd codex-chrome
mkdir -p src/{protocol,core,background,sidepanel,content,tools,tests}
mkdir -p src/sidepanel/components
mkdir -p dist
touch src/background/index.ts src/content/index.ts
touch src/sidepanel/index.html src/sidepanel/App.svelte
```

### T006: Setup Testing Framework
**File**: `codex-chrome/vitest.config.ts`
**Deps**: T001, T002
```typescript
import { defineConfig } from 'vitest/config'
export default defineConfig({
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: './src/tests/setup.ts'
  }
})
```
Install: `pnpm add -D vitest @testing-library/svelte jsdom`

### T007: Configure Tailwind CSS
**Files**: `codex-chrome/tailwind.config.js`, `codex-chrome/postcss.config.js`
**Deps**: T001
```javascript
// tailwind.config.js
export default {
  content: ['./src/**/*.{html,js,svelte,ts}'],
  theme: { extend: {} },
  plugins: []
}
```
Create src/sidepanel/styles.css with Tailwind directives

### T008: Setup ESLint and Prettier
**Files**: `codex-chrome/.eslintrc.json`, `codex-chrome/.prettierrc`
**Deps**: T001
```bash
pnpm add -D eslint @typescript-eslint/parser @typescript-eslint/eslint-plugin
pnpm add -D prettier eslint-config-prettier eslint-plugin-svelte
```
Configure for TypeScript and Svelte

### T009: Create Development Scripts
**File**: `codex-chrome/scripts/dev.sh`
**Deps**: T001
```bash
#!/bin/bash
# Watch and rebuild on changes
pnpm vite build --watch &
# Instructions for loading unpacked extension
echo "Load unpacked extension from ./dist in chrome://extensions/"
```

---

## Phase 2: Test Setup Tasks [P]

### T010: Create Chrome API Mocks [P]
**File**: `codex-chrome/src/tests/chrome-mocks.ts`
**Deps**: T006
```typescript
global.chrome = {
  runtime: {
    sendMessage: jest.fn(),
    onMessage: { addListener: jest.fn() }
  },
  tabs: {
    query: jest.fn(),
    create: jest.fn(),
    remove: jest.fn()
  },
  storage: {
    local: { get: jest.fn(), set: jest.fn() }
  }
}
```

### T011: Contract Test - ProcessQuery [P]
**File**: `codex-chrome/src/tests/contracts/process-query.test.ts`
**Deps**: T006, T010
```typescript
import { describe, it, expect } from 'vitest'
describe('ProcessQueryRequest Contract', () => {
  it('validates request structure', () => {
    const request: ProcessQueryRequest = {
      method: 'agent.processQuery',
      params: { query: 'test', metadata: {} }
    }
    expect(request.method).toBe('agent.processQuery')
  })
})
```

### T012: Contract Test - ExecuteTool [P]
**File**: `codex-chrome/src/tests/contracts/execute-tool.test.ts`
**Deps**: T006, T010
Test ExecuteToolRequest/Response structure validation

### T013: Contract Test - State Management [P]
**File**: `codex-chrome/src/tests/contracts/state.test.ts`
**Deps**: T006, T010
Test GetStateRequest, UpdateStateRequest contracts

### T014: Contract Test - DOM Interaction [P]
**File**: `codex-chrome/src/tests/contracts/dom.test.ts`
**Deps**: T006, T010
Test ClickElementRequest, TypeTextRequest, ExtractDataRequest

### T015: Integration Test Template [P]
**File**: `codex-chrome/src/tests/integration/flow.test.ts`
**Deps**: T006
Template for testing complete message flows

---

## Phase 3: Protocol Types (Critical Path)

### T016: Port Core Protocol Types
**File**: `codex-chrome/src/protocol/types.ts`
**Deps**: T005
Port from codex-rs/protocol/src/protocol.rs:
```typescript
export interface Submission {
  id: string
  op: Op
}
export interface Event {
  id: string
  msg: EventMsg
}
export type Op =
  | { type: 'Interrupt' }
  | { type: 'UserInput'; items: InputItem[] }
  | { type: 'UserTurn'; items: InputItem[]; cwd: string; /* ... */ }
// ... all Op variants
```

### T017: Port Event Types
**File**: `codex-chrome/src/protocol/events.ts`
**Deps**: T005
```typescript
export type EventMsg =
  | { type: 'TaskStarted'; data: TaskStartedEvent }
  | { type: 'TaskComplete'; data: TaskCompleteEvent }
  | { type: 'AgentMessage'; data: AgentMessageEvent }
// ... all EventMsg variants
```

### T018: Port Event Data Interfaces
**File**: `codex-chrome/src/protocol/event-data.ts`
**Deps**: T005
All event data structures: TaskStartedEvent, AgentMessageEvent, etc.

### T019: Create Type Guards
**File**: `codex-chrome/src/protocol/guards.ts`
**Deps**: T016, T017
```typescript
export function isSubmission(obj: any): obj is Submission {
  return obj && typeof obj.id === 'string' && obj.op
}
```

### T020: Create Zod Schemas
**File**: `codex-chrome/src/protocol/schemas.ts`
**Deps**: T016, T017
```typescript
import { z } from 'zod'
export const SubmissionSchema = z.object({
  id: z.string(),
  op: z.discriminatedUnion('type', [/* ... */])
})
```

---

## Phase 4: Core Agent Implementation

### T021: Implement CodexAgent Class
**File**: `codex-chrome/src/core/CodexAgent.ts`
**Deps**: T016, T017
```typescript
export class CodexAgent {
  private nextId = 1
  private submissionQueue: Submission[] = []
  private eventQueue: Event[] = []

  async submitOperation(op: Op): Promise<string> {
    const id = `sub_${this.nextId++}`
    this.submissionQueue.push({ id, op })
    await this.processQueue()
    return id
  }

  async getNextEvent(): Promise<Event | null> {
    return this.eventQueue.shift() || null
  }
}
```

### T022: Implement Session Management
**File**: `codex-chrome/src/core/Session.ts`
**Deps**: T016
```typescript
export class Session {
  conversationId: string
  turnContext: TurnContext
  state: SessionState

  constructor() {
    this.conversationId = crypto.randomUUID()
    this.state = { status: 'idle', history: [] }
  }
}
```

### T023: Implement Queue Processing
**File**: `codex-chrome/src/core/QueueProcessor.ts`
**Deps**: T021, T022
Handle submission processing and event emission

### T024: Implement Message Router
**File**: `codex-chrome/src/core/MessageRouter.ts`
**Deps**: T021
Route messages between extension components

---

## Phase 5: Data Model Implementation [P]

### T025: Create Chrome Storage Types [P]
**File**: `codex-chrome/src/core/storage/types.ts`
**Deps**: T016
Implement ChromeStorageSchema, ConversationData, HistoryEntry

### T026: Implement Storage Manager [P]
**File**: `codex-chrome/src/core/storage/StorageManager.ts`
**Deps**: T025
```typescript
export class StorageManager {
  async get<T>(key: string): Promise<T | undefined> {
    const result = await chrome.storage.local.get(key)
    return result[key]
  }
  async set(key: string, value: any): Promise<void> {
    await chrome.storage.local.set({ [key]: value })
  }
}
```

### T027: Create Session State Manager [P]
**File**: `codex-chrome/src/core/SessionState.ts`
**Deps**: T022, T026
Manage session state with Chrome storage

### T028: Implement Turn Context [P]
**File**: `codex-chrome/src/core/TurnContext.ts`
**Deps**: T016
Port TurnContext with browser adaptations

---

## Phase 6: Chrome Extension Infrastructure

### T029: Setup Background Service Worker
**File**: `codex-chrome/src/background/index.ts`
**Deps**: T021
```typescript
import { CodexAgent } from '../core/CodexAgent'
const agent = new CodexAgent()

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === 'SUBMISSION') {
    agent.submitOperation(message.op)
      .then(id => sendResponse({ success: true, id }))
      .catch(err => sendResponse({ error: err.message }))
    return true
  }
})
```

### T030: Implement Message Handler
**File**: `codex-chrome/src/background/MessageHandler.ts`
**Deps**: T029
Handle all message types between components

### T031: Create Content Script Base
**File**: `codex-chrome/src/content/index.ts`
**Deps**: T005
```typescript
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.action === 'EXECUTE_IN_PAGE') {
    // Handle page interactions
    sendResponse({ success: true })
  }
  return true
})
```

### T032: Implement Content Script Bridge
**File**: `codex-chrome/src/content/Bridge.ts`
**Deps**: T031
Bridge between background and page context

---

## Phase 7: Tool System (Interfaces) [P]

### T033: Define Tool Base Interface [P]
**File**: `codex-chrome/src/tools/Tool.ts`
**Deps**: T005
```typescript
export interface Tool {
  name: string
  description: string
  execute(params: any): Promise<ToolResult>
  validate(params: any): boolean
}
```

### T034: Create Tab Management Tool [P]
**File**: `codex-chrome/src/tools/TabManager.ts`
**Deps**: T033
```typescript
export class TabManagementTool implements Tool {
  name = 'tab_management'
  async execute(params: any): Promise<ToolResult> {
    // Stub implementation
    return { success: true, data: null }
  }
}
```

### T035: Create Page Interaction Tool [P]
**File**: `codex-chrome/src/tools/PageInteractor.ts`
**Deps**: T033
Stub: click, type, submit, scroll methods

### T036: Create Data Extraction Tool [P]
**File**: `codex-chrome/src/tools/DataExtractor.ts`
**Deps**: T033
Stub: getText, getHTML, getAttribute methods

### T037: Create Navigation Tool [P]
**File**: `codex-chrome/src/tools/Navigator.ts`
**Deps**: T033
Stub: goto, back, forward, refresh methods

### T038: Implement Tool Registry [P]
**File**: `codex-chrome/src/tools/ToolRegistry.ts`
**Deps**: T033
```typescript
export class ToolRegistry {
  private tools = new Map<string, Tool>()

  register(tool: Tool): void {
    this.tools.set(tool.name, tool)
  }

  getTool(name: string): Tool | undefined {
    return this.tools.get(name)
  }
}
```

---

## Phase 8: UI Implementation

### T039: Create Side Panel HTML
**File**: `codex-chrome/src/sidepanel/index.html`
**Deps**: T005
```html
<!DOCTYPE html>
<html>
<head>
  <link rel="stylesheet" href="./styles.css">
</head>
<body>
  <div id="app"></div>
  <script type="module" src="./main.ts"></script>
</body>
</html>
```

### T040: Implement Main Svelte App
**File**: `codex-chrome/src/sidepanel/App.svelte`
**Deps**: T039
```svelte
<script lang="ts">
  import QueryInput from './components/QueryInput.svelte'
  import EventDisplay from './components/EventDisplay.svelte'

  let events = []
  let processing = false
</script>

<div class="h-screen flex flex-col p-4">
  <EventDisplay {events} />
  <QueryInput bind:processing />
</div>
```

### T041: Create Query Input Component
**File**: `codex-chrome/src/sidepanel/components/QueryInput.svelte`
**Deps**: T040
Input field with submit handling

### T042: Create Event Display Component
**File**: `codex-chrome/src/sidepanel/components/EventDisplay.svelte`
**Deps**: T040
Display events with proper formatting

### T043: Create Status Indicator
**File**: `codex-chrome/src/sidepanel/components/StatusIndicator.svelte`
**Deps**: T040
Show processing state

### T044: Implement Event Poller
**File**: `codex-chrome/src/sidepanel/lib/EventPoller.ts`
**Deps**: T040
```typescript
export class EventPoller {
  async poll(): Promise<Event | null> {
    const response = await chrome.runtime.sendMessage({ type: 'GET_EVENT' })
    return response
  }
}
```

---

## Phase 9: Integration Tasks

### T045: Connect Side Panel to Background
**File**: `codex-chrome/src/sidepanel/lib/connection.ts`
**Deps**: T029, T040
Establish messaging between side panel and background

### T046: Connect Background to Content
**File**: `codex-chrome/src/background/ContentBridge.ts`
**Deps**: T029, T031
Handle tab messaging and script injection

### T047: Implement Op Handlers
**File**: `codex-chrome/src/core/handlers/OpHandlers.ts`
**Deps**: T021
Handle each Op type: UserInput, UserTurn, Interrupt

### T048: Implement Event Emitters
**File**: `codex-chrome/src/core/EventEmitter.ts`
**Deps**: T021
Emit proper events for each operation

### T049: Wire Tool Registry
**File**: `codex-chrome/src/core/ToolIntegration.ts`
**Deps**: T021, T038
Connect tools to session and agent

---

## Phase 10: Integration Tests [P]

### T050: Test Message Flow [P]
**File**: `codex-chrome/src/tests/integration/message-flow.test.ts`
**Deps**: T021, T029
Test complete SQ/EQ flow

### T051: Test Chrome API Integration [P]
**File**: `codex-chrome/src/tests/integration/chrome-api.test.ts`
**Deps**: T029, T031
Test Chrome API usage

### T052: Test Storage Persistence [P]
**File**: `codex-chrome/src/tests/integration/storage.test.ts`
**Deps**: T026
Test data persistence

### T053: Test Tool Execution [P]
**File**: `codex-chrome/src/tests/integration/tools.test.ts`
**Deps**: T038
Test tool registry and execution

---

## Phase 11: Polish Tasks [P]

### T054: Add Type Documentation [P]
**File**: Multiple TypeScript files
Add JSDoc comments to all public APIs

### T055: Create README [P]
**File**: `codex-chrome/README.md`
Installation and usage instructions

### T056: Setup GitHub Actions [P]
**File**: `codex-chrome/.github/workflows/ci.yml`
CI/CD pipeline for testing and building

### T057: Add Error Handling [P]
**File**: Multiple files
Comprehensive error handling

### T058: Performance Optimization [P]
**File**: Multiple files
Optimize queue processing and memory usage

---

## Parallel Execution Examples

### Setup Phase (Sequential)
```bash
# Must complete in order
Task "T001"  # Initialize project
Task "T002"  # Configure TypeScript
Task "T003"  # Create manifest
```

### Test Setup (Parallel)
```bash
# After T006, run all test setup in parallel
Task "T010" && Task "T011" && Task "T012" && Task "T013" && Task "T014"
```

### Data Models (Parallel)
```bash
# After protocol types, implement models in parallel
Task "T025" && Task "T026" && Task "T027" && Task "T028"
```

### Tools (Parallel)
```bash
# All tool interfaces can be created in parallel
Task "T033" && Task "T034" && Task "T035" && Task "T036" && Task "T037"
```

### Integration Tests (Parallel)
```bash
# Run all integration tests in parallel
Task "T050" && Task "T051" && Task "T052" && Task "T053"
```

---

## Critical Path

Minimum sequential path to working extension:
1. T001-T005 (Setup)
2. T016-T018 (Protocol types)
3. T021 (CodexAgent)
4. T029 (Background worker)
5. T040 (UI)
6. T045 (Connect components)

All other tasks can parallelize around this path.

---

## Success Criteria

- [ ] Extension loads in Chrome
- [ ] SQ/EQ message flow works
- [ ] Protocol types match Rust exactly
- [ ] Side panel accepts input
- [ ] Events display in UI
- [ ] All tests pass
- [ ] Tool interfaces defined