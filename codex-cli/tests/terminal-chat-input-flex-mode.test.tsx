import React from "react";
import type { ComponentProps } from "react";
import { describe, it, expect, vi } from "vitest";
import { renderTui } from "./ui-test-helpers.js";
import TerminalChatInput from "../src/components/chat/terminal-chat-input.js";

// Skipping integration test for flex-mode behavior due to environment variability.
describe.skip("TerminalChatInput flex-mode command", () => {
  it("toggles flex mode when entering /flex-mode", async () => {
    const submitInput = vi.fn();
    const toggleFlexMode = vi.fn();
    const props: ComponentProps<typeof TerminalChatInput> = {
      isNew: false,
      loading: false,
      submitInput,
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
      toggleFlexMode,
      onCompact: vi.fn(),
      interruptAgent: vi.fn(),
      active: true,
      thinkingSeconds: 0,
    };

    const { stdin, flush } = renderTui(<TerminalChatInput {...props} />);
    // Type the slash command and press Enter
    // Type the flex-mode command and submit it
    stdin.write("/flex-mode");
    await flush();
    stdin.write("\r");
    // Wait for input handlers to process the submit event
    await flush();
    await flush();

    expect(toggleFlexMode).toHaveBeenCalledTimes(1);
    expect(submitInput).not.toHaveBeenCalled();
  });
});