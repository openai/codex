import { renderTui } from "./ui-test-helpers.js";
import TerminalChatResponseItem from "../src/components/chat/terminal-chat-response-item.js";
import React from "react";
import { describe, it, expect } from "vitest";

// Component under test

// The ResponseItem type is complex and imported from the OpenAI SDK. To keep
// this test lightweight we construct the minimal runtime objects we need and
// cast them to `any` so that TypeScript is satisfied.

function userMessage(text: string) {
  return {
    type: "message",
    role: "user",
    content: [
      {
        type: "input_text",
        text,
      },
    ],
  } as any;
}

function assistantMessage(text: string) {
  return {
    type: "message",
    role: "assistant",
    content: [
      {
        type: "output_text",
        text,
      },
    ],
  } as any;
}

describe("TerminalChatResponseItem", () => {
  it("renders a user message", () => {
    const { lastFrameStripped } = renderTui(
      <TerminalChatResponseItem
        item={userMessage("Hello world")}
        fileOpener={undefined}
      />,
    );

    const frame = lastFrameStripped();
    expect(frame).toContain("user");
    expect(frame).toContain("Hello world");
  });

  it("renders an assistant message", () => {
    const { lastFrameStripped } = renderTui(
      <TerminalChatResponseItem
        item={assistantMessage("Sure thing")}
        fileOpener={undefined}
      />,
    );

    const frame = lastFrameStripped();
    // assistant messages are labelled "codex" in the UI
    expect(frame.toLowerCase()).toContain("codex");
    expect(frame).toContain("Sure thing");
  });
});
