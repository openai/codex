import React from "react";
import type { ComponentProps } from "react";
import { renderTui } from "./ui-test-helpers.js";
import TerminalChatInput from "../src/components/chat/terminal-chat-input.js";
import { describe, it, expect, vi } from "vitest";

// Helper that lets us type and then immediately flush ink's async timers
async function type(
  stdin: NodeJS.WritableStream,
  text: string,
  flush: () => Promise<void>,
) {
  stdin.write(text);
  await flush();
}

// Mock the createInputItem function to avoid filesystem operations
vi.mock("../src/utils/input-utils.js", () => ({
  createInputItem: vi.fn(async (text: string) => ({
    role: "user",
    type: "message",
    content: [{ type: "input_text", text }],
  })),
}));

describe("TerminalChatInput multiline functionality", () => {
  it("allows multiline input with shift+enter", async () => {
    const submitInput = vi.fn();

    const props: ComponentProps<typeof TerminalChatInput> = {
      isNew: false,
      loading: false,
      submitInput,
      confirmationPrompt: null,
      explanation: undefined,
      submitConfirmation: () => {},
      setLastResponseId: () => {},
      setItems: () => {},
      contextLeftPercent: 50,
      openOverlay: () => {},
      openDiffOverlay: () => {},
      openModelOverlay: () => {},
      openApprovalOverlay: () => {},
      openHelpOverlay: () => {},
      openSessionsOverlay: () => {},
      onCompact: () => {},
      interruptAgent: () => {},
      active: true,
      thinkingSeconds: 0,
    };

    const { stdin, lastFrameStripped, flush, cleanup } = renderTui(
      <TerminalChatInput {...props} />,
    );

    // Type some text
    await type(stdin, "first line", flush);

    // Send Shift+Enter (CSI-u format)
    await type(stdin, "\u001B[13;2u", flush);

    // Type more text
    await type(stdin, "second line", flush);

    // Check that both lines are visible in the editor
    const frame = lastFrameStripped();
    expect(frame).toContain("first line");
    expect(frame).toContain("second line");

    // Submit the multiline input with Enter
    await type(stdin, "\r", flush);

    // Check that submitInput was called with the multiline text
    expect(submitInput).toHaveBeenCalledTimes(1);

    cleanup();
  });

  it("allows multiline input with shift+enter (modifyOtherKeys=1 format)", async () => {
    const submitInput = vi.fn();

    const props: ComponentProps<typeof TerminalChatInput> = {
      isNew: false,
      loading: false,
      submitInput,
      confirmationPrompt: null,
      explanation: undefined,
      submitConfirmation: () => {},
      setLastResponseId: () => {},
      setItems: () => {},
      contextLeftPercent: 50,
      openOverlay: () => {},
      openDiffOverlay: () => {},
      openModelOverlay: () => {},
      openApprovalOverlay: () => {},
      openHelpOverlay: () => {},
      openSessionsOverlay: () => {},
      onCompact: () => {},
      interruptAgent: () => {},
      active: true,
      thinkingSeconds: 0,
    };

    const { stdin, lastFrameStripped, flush, cleanup } = renderTui(
      <TerminalChatInput {...props} />,
    );

    // Type some text
    await type(stdin, "first line", flush);

    // Send Shift+Enter (modifyOtherKeys=1 format)
    await type(stdin, "\u001B[27;2;13~", flush);

    // Type more text
    await type(stdin, "second line", flush);

    // Check that both lines are visible in the editor
    const frame = lastFrameStripped();
    expect(frame).toContain("first line");
    expect(frame).toContain("second line");

    // Submit the multiline input with Enter
    await type(stdin, "\r", flush);

    // Check that submitInput was called with the multiline text
    expect(submitInput).toHaveBeenCalledTimes(1);

    cleanup();
  });
});
