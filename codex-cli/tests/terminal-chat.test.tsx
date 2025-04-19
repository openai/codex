import React from "react";
import { renderTui } from "./ui-test-helpers.js";
import TerminalChat from "../src/components/chat/terminal-chat";
import { AppConfig } from "../src/utils/config";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import * as modelUtils from "../src/utils/model-utils";

// Mock getAvailableModels
vi.mock("../src/utils/model-utils", () => ({
  getAvailableModels: vi.fn(),
  RECOMMENDED_MODELS: [],
}));

// Mock the TerminalChatInput component, which appears to be the source of the
// stdin.ref error in the test environment. This isolates the test to focus
// on the model validation logic in TerminalChat.
vi.mock("../src/components/chat/terminal-chat-input.js", () => ({
  // Assuming the default export is the TerminalChatInput component
  default: vi.fn(() => null), // Mock the component to render null
}));

describe("TerminalChat model validation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("should display a warning if the configured model is unavailable", async () => {
    vi.mocked(modelUtils.getAvailableModels).mockResolvedValue([
      "gpt-4",
      "gpt-3.5",
    ]);
    const config = { model: "gpt-unicorn" } as AppConfig;
    const { lastFrameStripped, rerender } = renderTui(
      <TerminalChat
        config={config}
        approvalPolicy="suggest"
        additionalWritableRoots={[]}
        fullStdout={false}
      />,
    );
    // Wait for the effect in TerminalChat to complete, which fetches models
    // and updates the state that should trigger the warning message.
    // Increased timeout slightly to be safer, though the underlying Ink/stdin
    // issue was the main problem that should be resolved by mocking the input.
    await new Promise((resolve) => setTimeout(resolve, 100));
    rerender?.(); // Force a re-render to capture the updated state

    const frame = lastFrameStripped();

    // The expected output format might include surrounding characters from Ink's rendering.
    // We'll check for a significant part of the warning message.
    expect(frame).toContain(
      'Warning: model "gpt-unicorn" is not in the list of available models returned by OpenAI.',
    );
  });

  it("should NOT display a warning if the configured model is available", async () => {
    vi.mocked(modelUtils.getAvailableModels).mockResolvedValue([
      "gpt-4",
      "gpt-3.5",
    ]);
    const config = { model: "gpt-3.5" } as AppConfig;
    const { lastFrameStripped, rerender } = renderTui(
      <TerminalChat
        config={config}
        approvalPolicy="suggest"
        additionalWritableRoots={[]}
        fullStdout={false}
      />,
    );
    // Wait for the effect to complete
    await new Promise((resolve) => setTimeout(resolve, 100));
    rerender?.();
    const frame = lastFrameStripped();
    expect(frame).not.toContain(
      'Warning: model "gpt-3.5" is not in the list of available models returned by OpenAI.',
    );
  });
});
