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

const mocks = {
  transcriber: null as any,
};

// Mock the RealtimeTranscriber to avoid real WebSocket connections
vi.mock("../src/utils/transcriber.js", () => ({
  RealtimeTranscriber: vi.fn().mockImplementation(() => {
    const mock = {
      on: vi.fn(),
      start: vi.fn().mockResolvedValue(undefined),
      cleanup: vi.fn(),
    };
    mocks.transcriber = mock;
    return mock;
  }),
}));

describe("TerminalChatInput transcription functionality", () => {
  it("/speak command starts recording and shows the recording indicator", async () => {
    const props: ComponentProps<typeof TerminalChatInput> = {
      isNew: false,
      loading: false,
      submitInput: () => {},
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
      onCompact: () => {},
      interruptAgent: () => {},
      active: true,
      thinkingSeconds: 0,
    };

    const { stdin, flush, lastFrameStripped } = renderTui(
      <TerminalChatInput {...props} />,
    );
    // Wait for initial render to settle
    await flush();

    // Simulate the /speak command and press Enter to start recording
    await type(stdin, "/speak", flush);
    await type(stdin, "\r", flush);
    // Allow UI to update after pressing Enter
    await flush();

    // Check that the recording indicator is shown (isRecording = true)
    expect(lastFrameStripped()).toContain("●");
    // Allow transcription effect to run
    await flush();
    // Verify RealtimeTranscriber.start was called on the instance
    const transcriber = mocks.transcriber;
    expect(transcriber.start).toHaveBeenCalled();
  });

  it("pressing any key while recording stops the recording", async () => {
    const props: ComponentProps<typeof TerminalChatInput> = {
      isNew: false,
      loading: false,
      submitInput: () => {},
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
      onCompact: () => {},
      interruptAgent: () => {},
      active: true,
      thinkingSeconds: 0,
    };

    const { stdin, flush, lastFrameStripped } = renderTui(
      <TerminalChatInput {...props} />,
    );
    await flush();

    // Simulate the /speak command and press Enter to start recording
    await type(stdin, "/speak", flush);
    await type(stdin, "\r", flush);
    await flush();

    // Check that the recording indicator is shown (isRecording = true)
    expect(lastFrameStripped()).toContain("●");

    // Simulate pressing any key while recording
    await type(stdin, "a", flush);
    await flush();

    // Check that the recording indicator is no longer shown (isRecording = false)
    expect(lastFrameStripped()).not.toContain("●");

    // Verify RealtimeTranscriber.cleanup was called on the instance
    const transcriber = mocks.transcriber;
    expect(transcriber.cleanup).toHaveBeenCalled();
  });

  it("pressing enter while recording submits but doesn't stop recording", async () => {
    const props: ComponentProps<typeof TerminalChatInput> = {
      isNew: false,
      loading: false,
      submitInput: () => {},
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
      onCompact: () => {},
      interruptAgent: () => {},
      active: true,
      thinkingSeconds: 0,
    };

    const { stdin, flush, lastFrameStripped } = renderTui(
      <TerminalChatInput {...props} />,
    );
    await flush();

    // Simulate the /speak command and press Enter to start recording
    await type(stdin, "/speak", flush);
    await type(stdin, "\r", flush);
    await flush();

    // Check that the recording indicator is shown (isRecording = true)
    expect(lastFrameStripped()).toContain("●");

    // Simulate pressing Enter while recording
    await type(stdin, "\r", flush);
    await flush();

    // Check that the recording indicator is still shown (isRecording = true)
    expect(lastFrameStripped()).toContain("●");

    // Verify RealtimeTranscriber.cleanup has not been called
    const transcriber = mocks.transcriber;
    expect(transcriber.cleanup).not.toHaveBeenCalled();
  });
});
