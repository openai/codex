<!--
  NEW_TOOL_GUIDE.MD
  Guide for integrating a new tool into the Codex CLI agent.
-->
# New Tool Integration Guide

This document describes the generic steps required to add a new function-style tool
to the Codex CLI agent. Follow the patterns established by existing tools (e.g., `shell`,
`datetime`) to maintain consistency and quality.

---

## 1. Create a Handler

- **Location**: `codex-cli/src/utils/agent/handle-<tool>.ts`
- **Purpose**: Encapsulate the tool’s logic in a pure function that takes typed input
  and returns a typed result.

### Example Skeleton
```ts
// define your options & result types
export type <ToolName>Options = {
  /* parameters (all optional if desired) */
};
export interface <ToolName>Result {
  /* structured result fields */
  output: string;
  error?: string;
}

/**
 * Handler for the `<toolName>` tool.
 * @param options Input parameters for the tool.
 * @returns Structured result.
 */
export function handle<ToolName>(options: <ToolName>Options = {}): <ToolName>Result {
  try {
    // implement your logic here
    const result: <ToolName>Result = {
      output: '...',
      /* other fields */
    };
    return result;
  } catch (err) {
    return { output: '', error: String(err) };
  }
}
```

---

## 2. Extend Argument Parsing

- **File**: `codex-cli/src/utils/parsers.ts`
- **Task**: Teach the parser to recognize your tool’s JSON arguments so that
  `agent-loop` can route calls appropriately.

### Pattern
```ts
export function parseToolCallArguments(str: string): ExecInput | undefined {
  const json = JSON.parse(str);
  // Detect your tool by properties or by inspecting `name` in agent-loop
  if ('myParam' in json) {
    // Return a dummy ExecInput to satisfy the type system
    return { cmd: ['<toolName>'] };
  }
  // existing shell parsing...
}
```

---

## 3. Register and Dispatch in AgentLoop

- **File**: `codex-cli/src/utils/agent/agent-loop.ts`

### 3.1. Import Your Handler
```ts
import type { <ToolName>Options } from './handle-<tool>.js';
import { handle<ToolName> } from './handle-<tool>.js';
```

### 3.2. Register Tool in OpenAI Call
Locate the call to `this.oai.responses.create({... tools: [ ... ] })` and add:
```ts
{
  type: 'function',
  name: '<toolName>',
  description: 'Brief description of what the tool does.',
  strict: false,
  parameters: {
    type: 'object',
    properties: {
      param1: { type: 'string', description: '...' },
      // ... other params
    },
    additionalProperties: false,
  },
},
```

### 3.3. Handle Function Calls
In the `handleFunctionCall` method, after the existing branches for `shell`:
```ts
} else if (name === '<toolName>') {
  try {
    const parsed = JSON.parse(rawArguments ?? '{}') as Partial<<ToolName>Options>;
    const opts: <ToolName>Options = {
      param1: parsed.param1 ? String(parsed.param1) : undefined,
      // ...extract others
    };
    const res = handle<ToolName>(opts);
    outputItem.output = JSON.stringify({
      output: res.output,
      metadata: { exit_code: 0, duration_seconds: 0.1 },
    });
  } catch (err) {
    outputItem.output = JSON.stringify({
      output: `Error: ${err}`,
      metadata: { exit_code: 1, duration_seconds: 0 },
    });
  }
}
```

---

## 4. Update the UI for Tool Calls

### Confirmation Overlay
- **File**: `codex-cli/src/components/chat/terminal-chat-tool-call-item.tsx`
- **Task**: Accept a `toolName` prop and render it in the header instead of
  the static “Shell Command”.

### Chat Rendering
- **File**: `codex-cli/src/components/chat/terminal-chat-response-item.tsx`
- **Task**: In `TerminalChatResponseToolCall`, use `parseToolCall` to extract
  `details.cmd[0]` and display it instead of the literal “command”.

---

## 5. Clean Up Debug Logs

- Remove all `console.log` statements added during development in:
  - Your handler (`handle-<tool>.ts`)
  - `agent-loop.ts`

---

## 6. Testing

1. Build the CLI:
   ```bash
   cd codex-cli
   npm install
   npm run build
   ```
2. Issue a query invoking your tool:
   ```bash
   node dist/cli.js "Use the <toolName> tool with param1=foo"
   ```
3. (Optional) Enable debug logging:
   ```bash
   export DEBUG=1
   tail -F ~/.local/oai-codex/codex-cli-latest.log
   ```
4. Verify:
   - The agent emits a `function_call` with `name: '<toolName>'`.
   - Your handler runs and returns the expected JSON output.
   - The UI displays `<toolName>` in both confirmation and chat views.

---

By following these steps, future maintainers or LLMs can add new tools
to the Codex CLI with confidence and consistency.