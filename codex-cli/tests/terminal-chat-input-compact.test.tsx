import React from "react";
import type { ComponentProps } from "react";
import { describe, expect, it } from "vitest";
import TerminalChatInput from "../src/components/chat/terminal-chat-input.js";
import { renderTui } from "./ui-test-helpers.js";

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
			openDiffOverlay: () => {},
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
