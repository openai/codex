import { renderTui } from "./ui-test-helpers.js";
import { TerminalChatCommandReview } from "../src/components/chat/terminal-chat-command-review.js";
import { ReviewDecision } from "../src/utils/agent/review.js";
import React from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { Text } from "ink";
 
// Mock useInput hook to avoid stdin issues
vi.mock("ink", async () => {
  const actual = await vi.importActual("ink");
  return {
    ...actual,
    useInput: vi.fn((callback) => {
      // Store the callback so we can trigger it in tests
      (global as any).inputCallback = callback;
    }),
  };
});
 
// Mock the Select component
vi.mock("../src/components/vendor/ink-select/select.js", () => ({
  Select: ({ onChange }: { onChange: (value: string) => void }) => {
    React.useEffect(() => {
      (global as any).selectOnChange = onChange;
    }, [onChange]);
    return <Text>Allow command?</Text>;
  },
}));
 
function triggerInput(input: string) {
  if ((global as any).inputCallback) {
    (global as any).inputCallback(input);
  }
}
 
function triggerSelect(value: string) {
  if ((global as any).selectOnChange) {
    (global as any).selectOnChange(value);
  }
}
 
describe("TerminalChatCommandReview - Explanation Mode", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (global as any).inputCallback = null;
    (global as any).selectOnChange = null;
  });
 
  it("renders explanation mode with provided explanation", async () => {
    const mockExplanation =
      "1. This command will list files\n2. It is safe to run";
    const onReviewCommand = vi.fn();
 
    const { lastFrameStripped, flush } = renderTui(
      <TerminalChatCommandReview
        confirmationPrompt={<Text>ls -la</Text>}
        onReviewCommand={onReviewCommand}
        explanation={mockExplanation}
        testMode="explanation"
      />,
    );
 
    await flush();
 
    const frame = lastFrameStripped();
    expect(frame).toContain("Command Explanation:");
    expect(frame).toContain("1. This command will list files");
    expect(frame).toContain("2. It is safe to run");
  });
 
  it("shows loading state when explanation is not provided", async () => {
    const onReviewCommand = vi.fn();
 
    const { lastFrameStripped, flush } = renderTui(
      <TerminalChatCommandReview
        confirmationPrompt={<Text>ls -la</Text>}
        onReviewCommand={onReviewCommand}
        testMode="explanation"
      />,
    );
 
    await flush();
 
    const frame = lastFrameStripped();
    expect(frame).toContain("Loading explanation...");
  });
 
  it("handles error messages in explanation", async () => {
    const errorExplanation = "Unable to generate explanation: API error";
    const onReviewCommand = vi.fn();
 
    const { lastFrameStripped, flush } = renderTui(
      <TerminalChatCommandReview
        confirmationPrompt={<Text>ls -la</Text>}
        onReviewCommand={onReviewCommand}
        explanation={errorExplanation}
        testMode="explanation"
      />,
    );
 
    await flush();
 
    const frame = lastFrameStripped();
    expect(frame).toContain("Unable to generate explanation: API error");
  });
});
 
describe("TerminalChatCommandReview - Selection Mode", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (global as any).inputCallback = null;
    (global as any).selectOnChange = null;
  });
 
  it("handles yes selection", async () => {
    const onReviewCommand = vi.fn();
 
    const { flush } = renderTui(
      <TerminalChatCommandReview
        confirmationPrompt={<Text>ls -la</Text>}
        onReviewCommand={onReviewCommand}
        testMode="select"
      />,
    );
 
    await flush();
    triggerSelect(ReviewDecision.YES);
 
    expect(onReviewCommand).toHaveBeenCalledWith(ReviewDecision.YES);
  });
});
 
describe("TerminalChatCommandReview - Confirmation Mode", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (global as any).inputCallback = null;
    (global as any).selectOnChange = null;
  });
 
  it("shows confirmation dialog", async () => {
    const onReviewCommand = vi.fn();
 
    const { lastFrameStripped, flush } = renderTui(
      <TerminalChatCommandReview
        confirmationPrompt={<Text>ls -la</Text>}
        onReviewCommand={onReviewCommand}
        testMode="confirm"
      />,
    );
 
    await flush();
 
    const frame = lastFrameStripped();
    expect(frame).toContain("Confirm your choice");
  });
 
  it("handles back navigation", async () => {
    const onReviewCommand = vi.fn();
 
    const { lastFrameStripped, flush } = renderTui(
      <TerminalChatCommandReview
        confirmationPrompt={<Text>ls -la</Text>}
        onReviewCommand={onReviewCommand}
        testMode="confirm"
      />,
    );
 
    await flush();
    triggerInput("/b");
    await flush();
 
    const frame = lastFrameStripped();
    expect(frame).toContain("Allow command?");
  });
});