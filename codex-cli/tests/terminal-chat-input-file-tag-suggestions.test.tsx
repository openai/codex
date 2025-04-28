import React from "react";
import type { ComponentProps } from "react";
import { renderTui } from "./ui-test-helpers.js";
import TerminalChatInput from "../src/components/chat/terminal-chat-input.js";
import { describe, it, expect, vi, beforeEach } from "vitest";

// Helper function for typing and flushing
async function type(
  stdin: NodeJS.WritableStream,
  text: string,
  flush: () => Promise<void>,
) {
  stdin.write(text);
  await flush();
}

// Mock the file system suggestions utility
vi.mock("../src/utils/file-system-suggestions.js", () => ({
  getFileSystemSuggestions: vi.fn((pathPrefix: string) => {
    // return different results based on the prefix
    const baseName = pathPrefix.slice(2);
    const allItems = ["file1.txt", "file2.js", "directory1/", "directory2/"];
    return allItems.filter((item) => item.slice(2).startsWith(baseName));
  }),
}));

// Mock the createInputItem function to avoid filesystem operations
vi.mock("../src/utils/input-utils.js", () => ({
  createInputItem: vi.fn(async (text: string) => ({
    role: "user",
    type: "message",
    content: [{ type: "input_text", text }],
  })),
}));

describe("TerminalChatInput file tag suggestions", () => {
  // Standard props for all tests
  const baseProps: ComponentProps<typeof TerminalChatInput> = {
    isNew: false,
    loading: false,
    submitInput: vi.fn(),
    confirmationPrompt: null,
    explanation: undefined,
    submitConfirmation: vi.fn(),
    setLastResponseId: vi.fn(),
    setItems: vi.fn(),
    contextLeftPercent: 50,
    openOverlay: vi.fn(),
    openDiffOverlay: vi.fn(),
    openModelOverlay: vi.fn(),
    openApprovalOverlay: vi.fn(),
    openHelpOverlay: vi.fn(),
    onCompact: vi.fn(),
    interruptAgent: vi.fn(),
    active: true,
    thinkingSeconds: 0,
  };

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("shows file system suggestions when typing @ alone", async () => {
    const { stdin, lastFrameStripped, flush, cleanup } = renderTui(
      <TerminalChatInput {...baseProps} />,
    );

    // Type @ to trigger current directory suggestions
    await type(stdin, "@", flush);

    // Press Tab to activate suggestions
    await type(stdin, "\t", flush);

    // Check that current directory suggestions are shown
    const frame = lastFrameStripped();
    expect(frame).toContain("file1.txt");

    cleanup();
  });

  it("completes the selected file system suggestion with Tab", async () => {
    const { stdin, lastFrameStripped, flush, cleanup } = renderTui(
      <TerminalChatInput {...baseProps} />,
    );

    // Type @ to trigger suggestions
    await type(stdin, "@", flush);

    // Press Tab to activate suggestions
    await type(stdin, "\t", flush);

    // Press Tab to select the first suggestion
    await type(stdin, "\t", flush);

    // Check that the input has been completed with the selected suggestion
    const frameAfterTab = lastFrameStripped();
    expect(frameAfterTab).toContain("@file1.txt");
    // Check that the rest of the suggestions have collapsed
    expect(frameAfterTab).not.toContain("file2.txt");
    expect(frameAfterTab).not.toContain("directory2/");
    expect(frameAfterTab).not.toContain("directory1/");

    cleanup();
  });

  it("clears file system suggestions when typing a space", async () => {
    const { stdin, lastFrameStripped, flush, cleanup } = renderTui(
      <TerminalChatInput {...baseProps} />,
    );

    // Type @ to trigger suggestions
    await type(stdin, "@", flush);

    // Press Tab to activate suggestions
    await type(stdin, "\t", flush);

    // Check that suggestions are shown
    let frame = lastFrameStripped();
    expect(frame).toContain("file1.txt");

    // Type a space to clear suggestions
    await type(stdin, " ", flush);

    // Check that suggestions are cleared
    frame = lastFrameStripped();
    expect(frame).not.toContain("file1.txt");

    cleanup();
  });
});
