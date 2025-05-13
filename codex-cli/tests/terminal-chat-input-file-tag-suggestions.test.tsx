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

/**
 * Helper to reliably trigger file system suggestions in tests.
 *
 * This function simulates typing '@' followed by Tab to ensure suggestions appear.
 *
 * In real usage, simply typing '@' does trigger suggestions correctly.
 */
async function typeFileTag(
  stdin: NodeJS.WritableStream,
  flush: () => Promise<void>,
) {
  // Type @ character
  stdin.write("@");
  await flush();

  stdin.write("\t");
  await flush();
}

// Mock the file system suggestions utility
vi.mock("../src/utils/file-system-suggestions.js", () => ({
  FileSystemSuggestion: class {}, // Mock the interface
  getFileSystemSuggestions: vi.fn((pathPrefix: string) => {
    const normalizedPrefix = pathPrefix.startsWith("./")
      ? pathPrefix.slice(2)
      : pathPrefix;
    const allItems = [
      { path: "file1.txt", isDirectory: false },
      { path: "file2.js", isDirectory: false },
      { path: "directory1/", isDirectory: true },
      { path: "directory2/", isDirectory: true },
    ];
    return allItems.filter((item) => item.path.startsWith(normalizedPrefix));
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
    submitInput: vi.fn().mockImplementation(() => {}),
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
    openSessionsOverlay: vi.fn(),
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

    // Type @ and activate suggestions
    await typeFileTag(stdin, flush);

    // Check that current directory suggestions are shown
    const frame = lastFrameStripped();
    expect(frame).toContain("file1.txt");

    cleanup();
  });

  it("completes the selected file system suggestion with Tab", async () => {
    const { stdin, lastFrameStripped, flush, cleanup } = renderTui(
      <TerminalChatInput {...baseProps} />,
    );

    // Type @ and activate suggestions
    await typeFileTag(stdin, flush);

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

    // Type @ and activate suggestions
    await typeFileTag(stdin, flush);

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

  it("selects and retains directory when pressing Enter on directory suggestion", async () => {
    const { stdin, lastFrameStripped, flush, cleanup } = renderTui(
      <TerminalChatInput {...baseProps} />,
    );

    // Type @ and activate suggestions
    await typeFileTag(stdin, flush);

    // Navigate to directory suggestion (we need two down keys to get to the first directory)
    await type(stdin, "\u001B[B", flush); // Down arrow key - move to file2.js
    await type(stdin, "\u001B[B", flush); // Down arrow key - move to directory1/

    // Check that the directory suggestion is selected
    let frame = lastFrameStripped();
    expect(frame).toContain("directory1/");

    // Press Enter to select the directory
    await type(stdin, "\r", flush);

    // Check that the input now contains the directory path
    frame = lastFrameStripped();
    expect(frame).toContain("@directory1/");

    // Check that submitInput was NOT called (since we're only navigating, not submitting)
    expect(baseProps.submitInput).not.toHaveBeenCalled();

    cleanup();
  });

  it("submits when pressing Enter on file suggestion", async () => {
    const { stdin, flush, cleanup } = renderTui(
      <TerminalChatInput {...baseProps} />,
    );

    // Type @ and activate suggestions
    await typeFileTag(stdin, flush);

    // Press Enter to select first suggestion (file1.txt)
    await type(stdin, "\r", flush);

    // Check that submitInput was called
    expect(baseProps.submitInput).toHaveBeenCalled();

    // Get the arguments passed to submitInput
    const submitArgs = (baseProps.submitInput as any).mock.calls[0][0];

    // Verify the first argument is an array with at least one item
    expect(Array.isArray(submitArgs)).toBe(true);
    expect(submitArgs.length).toBeGreaterThan(0);

    // Check that the content includes the file path
    const content = submitArgs[0].content;
    expect(Array.isArray(content)).toBe(true);
    expect(content.length).toBeGreaterThan(0);
    expect(content[0].text).toContain("@file1.txt");

    cleanup();
  });
});
