# Testing Guidelines for Codex CLI

This document outlines the testing approach and patterns used in the Codex CLI project to ensure code quality and reliability.

## Testing Framework

Codex CLI uses [Vitest](https://vitest.dev/) as its testing framework, which is a Vite-native test runner with an API that's compatible with Jest.

```bash
# Run all tests
npm test

# Run a specific test file
npx vitest run tests/path/to/file.test.ts

# Run tests in watch mode during development
npm run test:watch
```

## Key Test Patterns

### 1. Unit Testing Provider Implementations

Provider implementations (like `ClaudeProvider` and `OpenAIProvider`) have dedicated test files that verify their functionality:

- `tests/claude-provider.test.ts`
- `tests/openai-provider.test.ts`
- `tests/base-provider.test.ts`

These tests use mocking to isolate the provider from external APIs:

```typescript
// Example of mocking a dependency
import Anthropic from "@anthropic-ai/sdk";
vi.mock("@anthropic-ai/sdk");

// Later in the test...
(Anthropic as unknown as vi.Mock).mockImplementation(() => ({
  messages: { 
    create: mockCreateFn,
    stream: mockStreamFn
  }
}));
```

### 2. Feature-Specific Tests

Features have dedicated test files that verify their functionality:

- `tests/claude-shell-disabled.test.ts` - Tests shell command handling in Claude provider
- `tests/apply-patch.test.ts` - Tests patch application functionality
- `tests/parse-apply-patch.test.ts` - Tests parsing of patches
- `tests/github-cli.test.ts` - Tests GitHub CLI integration

### 3. UI Component Testing

UI components are tested using the Ink testing library, which allows rendering and interaction with terminal UI components:

- `tests/terminal-chat-response-item.test.tsx` - Tests chat response rendering
- `tests/multiline-shift-enter.test.tsx` - Tests multiline editor behavior
- `tests/markdown.test.tsx` - Tests markdown rendering

Example:
```typescript
import { render } from "ink-testing-library";
import { TerminalChatResponseItem } from "../src/components/chat/terminal-chat-response-item";

test("renders plain text correctly", () => {
  const { lastFrame } = render(<TerminalChatResponseItem content="Hello world" />);
  expect(lastFrame()).toContain("Hello world");
});
```

### 4. Agent Loop Testing

The agent loop is thoroughly tested with various scenarios:

- `tests/agent-cancel.test.ts` - Tests cancellation behavior
- `tests/agent-cancel-early.test.ts` - Tests early cancellation
- `tests/agent-cancel-prev-response.test.ts` - Tests previous response cancellation
- `tests/agent-cancel-race.test.ts` - Tests race conditions in cancellation
- `tests/agent-function-call-id.test.ts` - Tests function call ID handling
- `tests/agent-generic-network-error.test.ts` - Tests network error handling
- `tests/agent-interrupt-continue.test.ts` - Tests interruption with continuation
- `tests/agent-invalid-request-error.test.ts` - Tests invalid request handling
- `tests/agent-max-tokens-error.test.ts` - Tests max tokens error handling
- `tests/agent-network-errors.test.ts` - Tests various network errors
- `tests/agent-project-doc.test.ts` - Tests project documentation handling
- `tests/agent-rate-limit-error.test.ts` - Tests rate limit handling
- `tests/agent-server-retry.test.ts` - Tests server retry behavior
- `tests/agent-terminate.test.ts` - Tests agent termination
- `tests/agent-thinking-time.test.ts` - Tests thinking time behavior

### 5. Text Buffer Testing

The text buffer component has dedicated tests:

- `tests/text-buffer.test.ts` - Tests core text buffer functionality
- `tests/text-buffer-crlf.test.ts` - Tests CRLF line ending handling
- `tests/text-buffer-copy-paste.test.ts` - Tests copy/paste operations
- `tests/text-buffer-gaps.test.ts` - Tests gap buffer operations
- `tests/text-buffer-word.test.ts` - Tests word-based operations

### 6. Configuration Testing

Configuration handling is tested to ensure proper settings are loaded:

- `tests/config.test.tsx` - Tests configuration loading and parsing
- `tests/config-provider-integration.test.ts` - Tests provider integration with config
- `tests/provider-config.test.ts` - Tests provider-specific config
- `tests/api-key.test.ts` - Tests API key handling

### 7. Shell and Command Execution Testing

- `tests/raw-exec-process-group.test.ts` - Tests raw command execution
- `tests/cancel-exec.test.ts` - Tests cancellation of command execution
- `tests/direct-command.test.ts` - Tests direct command execution
- `tests/format-command.test.ts` - Tests command formatting
- `tests/invalid-command-handling.test.ts` - Tests invalid command handling

### 8. Multiline Editor Testing

The multiline editor component has thorough testing:

- `tests/multiline-input-test.ts` - Tests basic input handling
- `tests/multiline-ctrl-enter-submit.test.tsx` - Tests Ctrl+Enter submission
- `tests/multiline-dynamic-width.test.tsx` - Tests dynamic width handling
- `tests/multiline-enter-submit-cr.test.tsx` - Tests Enter submission with CR
- `tests/multiline-external-editor-shortcut.test.tsx` - Tests external editor integration
- `tests/multiline-history-behavior.test.tsx` - Tests history navigation
- `tests/multiline-newline.test.tsx` - Tests newline insertion
- `tests/multiline-shift-enter-crlf.test.tsx` - Tests Shift+Enter with CRLF
- `tests/multiline-shift-enter-mod1.test.tsx` - Tests Shift+Enter with Alt
- `tests/multiline-shift-enter.test.tsx` - Tests basic Shift+Enter behavior

### 9. Git and GitHub Testing

- `tests/git-github-approval.test.ts` - Tests Git and GitHub approval flows

## Best Practices for Testing

### 1. Create Dedicated Test Files

Create dedicated test files that focus on specific functionality. Name them with the `.test.ts` or `.test.tsx` extension.

### 2. Use Descriptive Test Blocks

Use descriptive `describe` and `test` blocks to organize tests:

```typescript
describe("ClaudeProvider", () => {
  describe("formatTools", () => {
    test("should handle shell tool with not implemented message", () => {
      // Test implementation
    });
  });
});
```

### 3. Mock External Dependencies

Use Vitest's mocking capabilities to isolate the unit under test from external dependencies:

```typescript
import { externalDependency } from "../src/external";
vi.mock("../src/external");

// Mock implementation
(externalDependency as jest.Mock).mockReturnValue("mocked result");
```

### 4. Test Edge Cases

Include tests for edge cases and error conditions:

```typescript
test("should handle undefined input", () => {
  const result = normalizeShellCommand(undefined);
  expect(result).toEqual(["echo", "Shell commands are not implemented in claude provider"]);
});
```

### 5. Testing UI Components

Use `ink-testing-library` to test UI components:

```typescript
import { render } from "ink-testing-library";
import { MyComponent } from "../src/components/my-component";

test("renders correctly", () => {
  const { lastFrame } = render(<MyComponent />);
  expect(lastFrame()).toContain("Expected output");
});
```

### 6. Adding New Tests

When adding new functionality:

1. Create a dedicated test file if it's a significant feature
2. Add tests to an existing file if it's an enhancement to existing functionality
3. Run tests in watch mode during development: `npm run test:watch`
4. Ensure all tests pass before submitting changes: `npm test`

## Example: Testing Shell Command Handling in Claude Provider

Here's a complete example of testing the shell command handling in the Claude provider:

```typescript
import { describe, test, expect, vi, beforeEach } from "vitest";
import {
  normalizeShellCommand,
  processShellToolInput,
  parseClaudeToolCall,
  claudeToolToOpenAIFunction,
  createDefaultClaudeTools,
  createShellCommandInstructions
} from "../src/utils/providers/claude-tools.js";

// Mock console.log to avoid cluttering test output
vi.spyOn(console, 'log').mockImplementation(() => {});

describe("Claude shell command handling", () => {
  test("normalizeShellCommand should return 'not implemented' message", () => {
    // Test with string input
    const result1 = normalizeShellCommand("ls -la");
    expect(result1).toEqual(["echo", "Shell commands are not implemented in claude provider"]);
    
    // Test with array input
    const result2 = normalizeShellCommand(["ls", "-la"]);
    expect(result2).toEqual(["echo", "Shell commands are not implemented in claude provider"]);
    
    // Test with undefined input
    const result3 = normalizeShellCommand(undefined);
    expect(result3).toEqual(["echo", "Shell commands are not implemented in claude provider"]);
  });
  
  // Additional tests...
});
```

This approach:
1. Isolates the functions being tested
2. Tests multiple input types
3. Verifies the expected output
4. Mocks console.log to keep test output clean