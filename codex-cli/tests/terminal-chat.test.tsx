import React from "react";
import { renderTui } from "./ui-test-helpers.js";
import TerminalChat from "../src/components/chat/terminal-chat";
import { AppConfig } from "../src/utils/config";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import * as modelUtils from "../src/utils/model-utils";

// --- Mock the get-diff utility to prevent loading errors in this test ---
vi.mock("../../utils/get-diff.js", () => ({
  getGitDiff: vi.fn(() => ({
    // Mock the getGitDiff function to return dummy values
    isGitRepo: false,
    diff: "Mock git diff output for tests",
  })),
}));
// --- End mock ---

// --- Mock the DiffOverlay component to prevent loading errors in this test ---
vi.mock("../diff-overlay.js", () => ({
  default: vi.fn(() => null), // Mock the component to return null
}));
// --- End mock ---

// Mock getAvailableModels
vi.mock("../src/utils/model-utils", () => ({
  getAvailableModels: vi.fn(),
  RECOMMENDED_MODELS: [], // Mock recommended models if needed
}));

// Mock the TerminalChatInput component
vi.mock("../src/components/chat/terminal-chat-input.js", () => ({
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
    // Arrange: Mock getAvailableModels to return a list that does NOT include "gpt-unicorn"
    vi.mocked(modelUtils.getAvailableModels).mockResolvedValue([
      "gpt-4",
      "gpt-3.5",
    ]);
    const config = { model: "gpt-unicorn" } as AppConfig;

    // Act: Render the TerminalChat component
    const { lastFrameStripped, rerender } = renderTui(
      <TerminalChat
        config={config}
        approvalPolicy="suggest"
        additionalWritableRoots={[]}
        fullStdout={false}
      />,
    );

    // Assert: Wait for effects to run and check the output
    await new Promise((resolve) => setTimeout(resolve, 100));
    rerender?.(); // Force a re-render to ensure Ink captures the latest state

    const frame = lastFrameStripped();

    // Check that the warning message is present in the rendered output.
    // Updated to match the new message format with provider information
    expect(frame).toContain(
      'Warning: model "gpt-unicorn" is not in the list of available models for provider "openai".',
    );
  });

  it("should NOT display a warning if the configured model is available", async () => {
    // Arrange: Mock getAvailableModels to return a list that *does* include "gpt-3.5"
    vi.mocked(modelUtils.getAvailableModels).mockResolvedValue([
      "gpt-4",
      "gpt-3.5",
    ]);
    const config = { model: "gpt-3.5" } as AppConfig;

    // Act: Render the TerminalChat component
    const { lastFrameStripped, rerender } = renderTui(
      <TerminalChat
        config={config}
        approvalPolicy="suggest"
        additionalWritableRoots={[]}
        fullStdout={false}
      />,
    );

    // Assert: Wait for effects and check output
    await new Promise((resolve) => setTimeout(resolve, 100));
    rerender?.();
    const frame = lastFrameStripped();

    // Check that the warning message is NOT present in the rendered output.
    // Updated to match the new message format with provider information
    expect(frame).not.toContain(
      'Warning: model "gpt-3.5" is not in the list of available models for provider "openai".',
    );
  });
});
