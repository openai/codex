# Codex Chrome Extension - Quick Start Guide

## Overview
Convert the Codex terminal agent to a Chrome extension, preserving the core SQ/EQ architecture and protocol from codex-rs.

## Project Setup

### 1. Initialize Project

```bash
# Create directory
mkdir codex-chrome
cd codex-chrome

# Initialize with TypeScript
npm init -y
npm install --save-dev typescript@5.x vite@5.x @types/chrome

# Svelte and UI
npm install svelte @sveltejs/vite-plugin-svelte tailwindcss

# Validation and utilities
npm install zod uuid

# Testing
npm install --save-dev vitest @playwright/test
```

### 2. TypeScript Configuration

```json
// tsconfig.json
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
    "types": ["chrome", "node", "vite/client"],
    "paths": {
      "@/*": ["./src/*"]
    }
  },
  "include": ["src/**/*"],
  "exclude": ["node_modules", "dist"]
}
```

### 3. Chrome Manifest V3

```json
// manifest.json
{
  "manifest_version": 3,
  "name": "Codex Chrome Agent",
  "version": "1.0.0",
  "description": "AI-powered browser automation preserving codex-rs architecture",

  "permissions": [
    "tabs",
    "activeTab",
    "storage",
    "scripting",
    "webNavigation"
  ],

  "host_permissions": ["<all_urls>"],

  "background": {
    "service_worker": "dist/background/index.js",
    "type": "module"
  },

  "action": {
    "default_title": "Open Codex Agent",
    "default_popup": "dist/popup/index.html"
  },

  "side_panel": {
    "default_path": "dist/sidepanel/index.html"
  },

  "content_scripts": [{
    "matches": ["<all_urls>"],
    "js": ["dist/content/index.js"],
    "run_at": "document_idle"
  }],

  "web_accessible_resources": [{
    "resources": ["dist/assets/*"],
    "matches": ["<all_urls>"]
  }]
}
```

### 4. Project Structure

```
codex-chrome/
├── src/
│   ├── protocol/           # Port from codex-rs/protocol
│   │   ├── types.ts       # Submission, Op, Event, EventMsg
│   │   ├── events.ts      # All event type definitions
│   │   └── constants.ts   # Protocol constants
│   │
│   ├── core/              # Port from codex-rs/core
│   │   ├── CodexAgent.ts  # Main agent (replacing Codex struct)
│   │   ├── Session.ts     # Session management
│   │   ├── Queue.ts       # SQ/EQ implementation
│   │   └── Tools.ts       # Tool registry
│   │
│   ├── background/        # Service worker
│   │   ├── index.ts       # Main background script
│   │   ├── MessageHandler.ts
│   │   └── StorageManager.ts
│   │
│   ├── sidepanel/         # Svelte UI
│   │   ├── App.svelte     # Main component
│   │   ├── index.html
│   │   └── components/
│   │
│   ├── content/           # Content scripts
│   │   ├── index.ts
│   │   ├── DOMInteractor.ts
│   │   └── DataExtractor.ts
│   │
│   └── tools/             # Browser tools
│       ├── TabManager.ts
│       ├── PageInteractor.ts
│       └── Navigator.ts
│
├── dist/                  # Build output
├── tests/
├── manifest.json
├── package.json
├── tsconfig.json
└── vite.config.ts
```

## Core Implementation

### 1. Port Protocol Types (Preserve Exact Names)

```typescript
// src/protocol/types.ts - Direct port from Rust

export interface Submission {
  id: string;
  op: Op;
}

export interface Event {
  id: string;
  msg: EventMsg;
}

export type Op =
  | { type: 'Interrupt' }
  | { type: 'UserInput'; items: InputItem[] }
  | { type: 'UserTurn'; items: InputItem[]; cwd: string; /* ... */ }
  // ... exact same as Rust enum

export type EventMsg =
  | { type: 'TaskStarted'; data: TaskStartedEvent }
  | { type: 'AgentMessage'; data: AgentMessageEvent }
  // ... exact same as Rust enum
```

### 2. Implement Core Agent (Preserving Architecture)

```typescript
// src/core/CodexAgent.ts - Port of codex.rs Codex struct

import { Submission, Event, Op, EventMsg } from '@/protocol/types';

export class CodexAgent {
  private nextId: number = 1;
  private submissionQueue: Submission[] = [];
  private eventQueue: Event[] = [];
  private session: Session;

  constructor() {
    this.session = new Session();
  }

  // Main entry point - same as Rust
  async submitOperation(op: Op): Promise<string> {
    const id = `sub_${this.nextId++}`;
    const submission: Submission = { id, op };

    this.submissionQueue.push(submission);
    await this.processQueue();

    return id;
  }

  async getNextEvent(): Promise<Event | null> {
    return this.eventQueue.shift() || null;
  }

  private async processQueue(): Promise<void> {
    while (this.submissionQueue.length > 0) {
      const submission = this.submissionQueue.shift()!;
      await this.handleSubmission(submission);
    }
  }

  private async handleSubmission(submission: Submission): Promise<void> {
    // Emit TaskStarted event
    this.emitEvent({
      type: 'TaskStarted',
      data: {
        submission_id: submission.id,
        turn_type: 'user'
      }
    });

    try {
      // Process based on Op type
      switch (submission.op.type) {
        case 'UserInput':
          await this.handleUserInput(submission.op.items);
          break;
        case 'UserTurn':
          await this.handleUserTurn(submission.op);
          break;
        // ... handle other ops
      }

      // Emit TaskComplete
      this.emitEvent({
        type: 'TaskComplete',
        data: { submission_id: submission.id }
      });
    } catch (error) {
      // Emit Error event
      this.emitEvent({
        type: 'Error',
        data: {
          code: 'PROCESSING_ERROR',
          message: error.message
        }
      });
    }
  }

  private emitEvent(msg: EventMsg): void {
    const event: Event = {
      id: `evt_${this.nextId++}`,
      msg
    };
    this.eventQueue.push(event);

    // Notify listeners (Chrome runtime)
    chrome.runtime.sendMessage({
      type: 'EVENT',
      payload: event
    });
  }
}
```

### 3. Session Management

```typescript
// src/core/Session.ts - Port of Session struct

export class Session {
  conversationId: string;
  turnContext: TurnContext;
  state: SessionState;
  toolRegistry: ToolRegistry;

  constructor() {
    this.conversationId = generateId();
    this.turnContext = getDefaultTurnContext();
    this.state = { status: 'idle', history: [] };
    this.toolRegistry = new ToolRegistry();

    this.registerChromeTools();
  }

  private registerChromeTools(): void {
    // Register browser-specific tools
    this.toolRegistry.register(new TabManagementTool());
    this.toolRegistry.register(new PageInteractionTool());
    this.toolRegistry.register(new DataExtractionTool());
    this.toolRegistry.register(new NavigationTool());
  }

  async executeTool(name: string, params: any): Promise<ToolResult> {
    const tool = this.toolRegistry.getTool(name);
    if (!tool) {
      throw new Error(`Tool not found: ${name}`);
    }
    return await tool.execute(params);
  }
}
```

### 4. Background Service Worker

```typescript
// src/background/index.ts

import { CodexAgent } from '@/core/CodexAgent';

const agent = new CodexAgent();

// Handle messages from sidepanel/popup
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === 'SUBMISSION') {
    agent.submitOperation(message.op)
      .then(id => sendResponse({ success: true, id }))
      .catch(err => sendResponse({ success: false, error: err.message }));
    return true; // Keep channel open
  }

  if (message.type === 'GET_EVENT') {
    agent.getNextEvent()
      .then(event => sendResponse(event))
      .catch(err => sendResponse({ error: err.message }));
    return true;
  }
});

// Initialize on install
chrome.runtime.onInstalled.addListener(() => {
  console.log('Codex Chrome Agent installed');
});
```

### 5. Side Panel UI (Svelte)

```svelte
<!-- src/sidepanel/App.svelte -->
<script lang="ts">
  import { onMount } from 'svelte';
  import type { Op, Event } from '@/protocol/types';

  let input = '';
  let events: Event[] = [];
  let processing = false;

  async function sendInput() {
    if (!input.trim() || processing) return;

    processing = true;

    // Create UserInput op
    const op: Op = {
      type: 'UserInput',
      items: [{
        type: 'text',
        content: input
      }]
    };

    // Send to background
    const response = await chrome.runtime.sendMessage({
      type: 'SUBMISSION',
      op
    });

    if (response.success) {
      input = '';
      pollForEvents();
    }

    processing = false;
  }

  async function pollForEvents() {
    const event = await chrome.runtime.sendMessage({
      type: 'GET_EVENT'
    });

    if (event) {
      events = [...events, event];

      // Continue polling if task not complete
      if (event.msg.type !== 'TaskComplete') {
        setTimeout(pollForEvents, 100);
      }
    }
  }

  onMount(() => {
    // Start event polling
    pollForEvents();
  });
</script>

<div class="p-4 h-screen flex flex-col">
  <h1 class="text-xl font-bold mb-4">Codex Agent</h1>

  <div class="flex-1 overflow-y-auto mb-4">
    {#each events as event}
      <div class="mb-2 p-2 bg-gray-100 rounded">
        <span class="text-xs text-gray-500">{event.msg.type}</span>
        {#if event.msg.type === 'AgentMessage'}
          <p>{event.msg.data.message}</p>
        {/if}
      </div>
    {/each}
  </div>

  <div class="flex gap-2">
    <input
      bind:value={input}
      on:keydown={(e) => e.key === 'Enter' && sendInput()}
      class="flex-1 p-2 border rounded"
      placeholder="Enter command..."
      disabled={processing}
    />
    <button
      on:click={sendInput}
      class="px-4 py-2 bg-blue-500 text-white rounded"
      disabled={processing}
    >
      Send
    </button>
  </div>
</div>
```

## Tool Implementation Example

```typescript
// src/tools/TabManager.ts

export class TabManagementTool implements Tool {
  name = 'tab_management';
  description = 'Manage browser tabs';

  async execute(params: any): Promise<ToolResult> {
    const { action, ...options } = params;

    try {
      switch (action) {
        case 'open':
          const tab = await chrome.tabs.create({
            url: options.url,
            active: options.active
          });
          return { success: true, data: tab };

        case 'close':
          await chrome.tabs.remove(options.tabId);
          return { success: true };

        case 'getAll':
          const tabs = await chrome.tabs.query({});
          return { success: true, data: tabs };

        default:
          throw new Error(`Unknown action: ${action}`);
      }
    } catch (error) {
      return {
        success: false,
        error: error.message
      };
    }
  }
}
```

## Testing

```typescript
// tests/protocol.test.ts
import { describe, it, expect } from 'vitest';
import { CodexAgent } from '@/core/CodexAgent';

describe('CodexAgent', () => {
  it('preserves SQ/EQ architecture', async () => {
    const agent = new CodexAgent();

    // Submit operation
    const id = await agent.submitOperation({
      type: 'UserInput',
      items: [{ type: 'text', content: 'test' }]
    });

    expect(id).toMatch(/^sub_/);

    // Get events
    const event = await agent.getNextEvent();
    expect(event?.msg.type).toBe('TaskStarted');
  });
});
```

## Key Migration Rules

1. **Preserve ALL type names** from Rust protocol
2. **Maintain SQ/EQ pattern** with same message flow
3. **Keep function names** where applicable (submitOperation, getNextEvent, etc.)
4. **Replace file ops** with browser ops (tabs, DOM, storage)
5. **Adapt sandbox** for browser security model

## Development Workflow

```bash
# Development
npm run dev

# Build extension
npm run build

# Run tests
npm test

# Load in Chrome
1. Open chrome://extensions/
2. Enable Developer mode
3. Load unpacked → select `codex-chrome` folder
```

## Debugging

- Service worker console: chrome://extensions/ → Inspect views
- Content script: Regular DevTools in web page
- Side panel: Right-click → Inspect