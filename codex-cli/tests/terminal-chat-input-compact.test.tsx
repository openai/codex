import React from "react";
import type { ComponentProps } from "react";
import { renderTui } from "./ui-test-helpers.js";
import TerminalChatInput from "../src/components/chat/terminal-chat-input.js";
import { describe, it, expect } from "vitest";

describe("TerminalChatInput compact command", () => {
  it("shows /compact hint when context is low", async () => {
    const props: ComponentProps<typeof TerminalChatInput> = {
      isNew: false,
      loading: false,
      submitInput: () => {},
      confirmationPrompt: null,
      explanation: undefined,
      submitConfirmation: () => {},
      setLastResponseId: () => {},
      setItems: () => {},
      contextLeftPercent: 10,
      openOverlay: () => {},
      openModelOverlay: () => {},
      openApprovalOverlay: () => {},
      openHelpOverlay: () => {},
      onCompact: () => {},
      interruptAgent: () => {},
      active: true,
      thinkingSeconds: 0,
    };
    const { lastFrameStripped } = renderTui(<TerminalChatInput {...props} />);
    const frame = lastFrameStripped();
    expect(frame).toContain("/compact");
  });
});
