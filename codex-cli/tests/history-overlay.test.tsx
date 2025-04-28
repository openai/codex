/* -------------------------------------------------------------------------- *
 * Tests for the HistoryOverlay component and its formatHistoryForDisplay utility function
 *
 * The component displays a list of commands and files from the chat history.
 * It supports two modes:
 * - Command mode: shows all commands and user messages
 * - File mode: shows all files that were touched
 *
 * The formatHistoryForDisplay function processes ResponseItems to extract:
 * - Commands: User messages and function calls
 * - Files: Paths referenced in commands or function calls
 * -------------------------------------------------------------------------- */

import { describe, it, expect, vi } from "vitest";
import { render } from "ink-testing-library";
import React from "react";
import type {
  ResponseInputMessageItem,
  ResponseFunctionToolCallItem,
} from "openai/resources/responses/responses.mjs";
import HistoryOverlay from "../src/components/history-overlay";

// ---------------------------------------------------------------------------
// Module mocks *must* be registered *before* the module under test is imported
// so that Vitest can replace the dependency during evaluation.
// ---------------------------------------------------------------------------

// Mock ink's useInput to capture keyboard handlers
let keyboardHandler: ((input: string, key: any) => void) | undefined;
vi.mock("ink", async () => {
  const actual = await vi.importActual("ink");
  return {
    ...actual,
    useInput: (handler: (input: string, key: any) => void) => {
      keyboardHandler = handler;
    },
  };
});

// ---------------------------------------------------------------------------
// Test Helpers
// ---------------------------------------------------------------------------

function createUserMessage(content: string): ResponseInputMessageItem {
  return {
    type: "message",
    role: "user",
    id: `msg_${Math.random().toString(36).slice(2)}`,
    content: [{ type: "input_text", text: content }],
  };
}

function createFunctionCall(
  name: string,
  args: unknown,
): ResponseFunctionToolCallItem {
  return {
    type: "function_call",
    name,
    id: `fn_${Math.random().toString(36).slice(2)}`,
    call_id: `call_${Math.random().toString(36).slice(2)}`,
    arguments: JSON.stringify(args),
  } as ResponseFunctionToolCallItem;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("HistoryOverlay", () => {
  describe("command mode", () => {
    it("displays user messages", () => {
      const items = [createUserMessage("hello"), createUserMessage("world")];
      const { lastFrame } = render(
        <HistoryOverlay items={items} onExit={vi.fn()} />,
      );
      const frame = lastFrame();
      expect(frame).toContain("hello");
      expect(frame).toContain("world");
    });

    it("displays shell commands", () => {
      const items = [
        createFunctionCall("shell", { cmd: ["ls", "-la"] }),
        createFunctionCall("shell", { cmd: ["pwd"] }),
      ];
      const { lastFrame } = render(
        <HistoryOverlay items={items} onExit={vi.fn()} />,
      );
      const frame = lastFrame();
      expect(frame).toContain("ls -la");
      expect(frame).toContain("pwd");
    });

    it("displays file operations", () => {
      const items = [createFunctionCall("read_file", { path: "test.txt" })];
      const { lastFrame } = render(
        <HistoryOverlay items={items} onExit={vi.fn()} />,
      );
      const frame = lastFrame();
      expect(frame).toContain("read_file test.txt");
    });

    it("displays patch operations", () => {
      const items = [
        createFunctionCall("shell", {
          cmd: [
            "apply_patch",
            "*** Begin Patch\n--- a/src/file1.txt\n+++ b/src/file1.txt\n@@ -1,5 +1,5 @@\n-const x = 1;\n+const x = 2;\n",
          ],
        }),
      ];
      const { lastFrame } = render(
        <HistoryOverlay items={items} onExit={vi.fn()} />,
      );

      // Verify patch is displayed in command mode
      let frame = lastFrame();
      expect(frame).toContain("apply_patch");
      expect(frame).toContain("src/file1.txt");

      // Verify file is extracted in file mode
      keyboardHandler?.("f", {});
      frame = lastFrame();
      expect(frame).toContain("src/file1.txt");
    });

    it("displays mixed content in chronological order", () => {
      const items = [
        createUserMessage("first message"),
        createFunctionCall("shell", { cmd: ["echo", "hello"] }),
        createUserMessage("second message"),
      ];
      const { lastFrame } = render(
        <HistoryOverlay items={items} onExit={vi.fn()} />,
      );
      const frame = lastFrame();
      expect(frame).toContain("first message");
      expect(frame).toContain("echo hello");
      expect(frame).toContain("second message");
    });

    it("truncates long user messages", () => {
      const shortMessage = "Hello";
      const longMessage =
        "This is a very long message that should be truncated because it exceeds the maximum length of 120 characters. We need to make sure it gets properly truncated with the right prefix and ellipsis.";
      const items = [
        createUserMessage(shortMessage),
        createUserMessage(longMessage),
      ];

      const { lastFrame } = render(
        <HistoryOverlay items={items} onExit={vi.fn()} />,
      );
      const frame = lastFrame()!;

      // Short message should have the > prefix
      expect(frame).toContain(`> ${shortMessage}`);

      // Long message should be truncated and contain:
      // 1. The > prefix
      expect(frame).toContain("> This is a very long message");
      // 2. An ellipsis indicating truncation
      expect(frame).toContain("…");
      // 3. Not contain the full message
      expect(frame).not.toContain(longMessage);

      // Find the truncated message line
      const lines = frame.split("\n");
      const truncatedLine = lines.find((line) =>
        line.includes("This is a very long message"),
      )!;
      // Verify it's not too long (allowing for some UI elements)
      expect(truncatedLine.trim().length).toBeLessThan(150);
    });
  });

  describe("file mode", () => {
    it("displays files from shell commands", () => {
      const items = [
        createFunctionCall("shell", { cmd: ["cat", "/path/to/file"] }),
      ];
      const { lastFrame } = render(
        <HistoryOverlay items={items} onExit={vi.fn()} />,
      );

      // Switch to file mode
      keyboardHandler?.("f", {});
      const frame = lastFrame();
      expect(frame).toContain("Files touched");
      expect(frame).toContain("/path/to/file");
    });

    it("displays files from read operations", () => {
      const items = [
        createFunctionCall("read_file", { path: "/path/to/file" }),
      ];
      const { lastFrame } = render(
        <HistoryOverlay items={items} onExit={vi.fn()} />,
      );

      // Switch to file mode
      keyboardHandler?.("f", {});
      const frame = lastFrame();
      expect(frame).toContain("Files touched");
      expect(frame).toContain("/path/to/file");
    });

    it("displays files from patches", () => {
      const items = [
        createFunctionCall("shell", {
          cmd: [
            "apply_patch",
            "*** Begin Patch\n--- a/src/file1.txt\n+++ b/src/file1.txt\n@@ -1,5 +1,5 @@\n-const x = 1;\n+const x = 2;\n",
          ],
        }),
      ];
      const { lastFrame } = render(
        <HistoryOverlay items={items} onExit={vi.fn()} />,
      );

      // Switch to file mode
      keyboardHandler?.("f", {});
      const frame = lastFrame();
      expect(frame).toContain("Files touched");
      expect(frame).toContain("src/file1.txt");
    });
  });

  describe("keyboard interaction", () => {
    it("handles mode switching with 'c' and 'f' keys", () => {
      const items = [
        createUserMessage("hello"),
        createFunctionCall("shell", { cmd: ["cat", "src/test.txt"] }),
      ];
      const { lastFrame } = render(
        <HistoryOverlay items={items} onExit={vi.fn()} />,
      );

      // Initial state (command mode)
      let frame = lastFrame();
      expect(frame).toContain("Commands run");
      expect(frame).toContain("hello");
      expect(frame).toContain("cat src/test.txt");

      // Switch to files mode
      keyboardHandler?.("f", {});
      frame = lastFrame();
      expect(frame).toContain("Files touched");
      expect(frame).toContain("src/test.txt");

      // Switch back to commands mode
      keyboardHandler?.("c", {});
      frame = lastFrame();
      expect(frame).toContain("Commands run");
      expect(frame).toContain("hello");
      expect(frame).toContain("cat src/test.txt");
    });

    it("handles escape key", () => {
      const onExit = vi.fn();
      render(<HistoryOverlay items={[]} onExit={onExit} />);

      keyboardHandler?.("", { escape: true });
      expect(onExit).toHaveBeenCalled();
    });

    it("handles arrow keys for navigation", () => {
      const items = [createUserMessage("first"), createUserMessage("second")];
      const { lastFrame } = render(
        <HistoryOverlay items={items} onExit={vi.fn()} />,
      );

      // Initial state shows first item selected
      let frame = lastFrame();
      expect(frame).toContain("› > first");
      expect(frame).not.toContain("› > second");

      // Move down - second item should be selected
      keyboardHandler?.("", { downArrow: true });
      frame = lastFrame();
      expect(frame).toContain("› > second");
      expect(frame).not.toContain("› > first");

      // Move up - first item should be selected again
      keyboardHandler?.("", { upArrow: true });
      frame = lastFrame();
      expect(frame).toContain("› > first");
      expect(frame).not.toContain("› > second");
    });

    it("handles page up/down navigation", () => {
      const items = Array.from({ length: 12 }, (_, i) =>
        createUserMessage(`message ${i + 1}`),
      );

      const { lastFrame } = render(
        <HistoryOverlay items={items} onExit={vi.fn()} />,
      );

      // Initial position - first message selected
      let frame = lastFrame();
      expect(frame).toMatch(/│ › > message 1\s+│/); // message 1 should be selected
      expect(frame).toMatch(/│ {3}> message 11\s+│/); // message 11 should be visible but not selected

      // Page down moves by 10 - message 11 should be selected
      keyboardHandler?.("", { pageDown: true });
      frame = lastFrame();
      expect(frame).toMatch(/│ {3}> message 1\s+│/); // message 1 should be visible but not selected
      expect(frame).toMatch(/│ › > message 11\s+│/); // message 11 should be selected
    });

    it("handles vim-style navigation", () => {
      const items = [
        createUserMessage("first"),
        createUserMessage("second"),
        createUserMessage("third"),
      ];
      const { lastFrame } = render(
        <HistoryOverlay items={items} onExit={vi.fn()} />,
      );

      // Initial state should show first item selected
      let frame = lastFrame();
      expect(frame).toContain("› > first");
      expect(frame).not.toContain("› > third"); // Make sure third is not selected initially

      // Test G to jump to end - third should be selected
      keyboardHandler?.("G", {});
      frame = lastFrame();
      expect(frame).toContain("› > third");

      // Test g to jump to beginning - first should be selected again
      keyboardHandler?.("g", {});
      frame = lastFrame();
      expect(frame).toContain("› > first");
    });
  });

  describe("error handling", () => {
    it("handles empty or invalid items", () => {
      const items = [{ type: "invalid" } as any, null as any, undefined as any];
      const { lastFrame } = render(
        <HistoryOverlay items={items} onExit={vi.fn()} />,
      );
      // Should render without errors
      expect(lastFrame()).toBeTruthy();
    });
  });
});
