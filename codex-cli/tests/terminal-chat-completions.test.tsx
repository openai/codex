import React from "react";
import { describe, it, expect } from "vitest";
import type { ComponentProps } from "react";
import { renderTui } from "./ui-test-helpers.js";
import TerminalChatCompletions from "../src/components/chat/terminal-chat-completions.js";

describe("TerminalChatCompletions", () => {
  const baseProps: ComponentProps<typeof TerminalChatCompletions> = {
    completions: ["Option 1", "Option 2", "Option 3", "Option 4", "Option 5"],
    displayLimit: 3,
    selectedCompletion: 0,
  };

  it("renders visible completions within displayLimit", async () => {
    const { lastFrameStripped } = renderTui(
      <TerminalChatCompletions {...baseProps} />,
    );
    const frame = lastFrameStripped();
    expect(frame).toContain("Option 1");
    expect(frame).toContain("Option 2");
    expect(frame).toContain("Option 3");
    expect(frame).not.toContain("Option 4");
  });

  it("centers the selected completion in the visible list", async () => {
    const { lastFrameStripped } = renderTui(
      <TerminalChatCompletions {...baseProps} selectedCompletion={2} />,
    );
    const frame = lastFrameStripped();
    expect(frame).toContain("Option 2");
    expect(frame).toContain("Option 3");
    expect(frame).toContain("Option 4");
    expect(frame).not.toContain("Option 1");
  });

  it("adjusts when selectedCompletion is near the end", async () => {
    const { lastFrameStripped } = renderTui(
      <TerminalChatCompletions {...baseProps} selectedCompletion={4} />,
    );
    const frame = lastFrameStripped();
    expect(frame).toContain("Option 3");
    expect(frame).toContain("Option 4");
    expect(frame).toContain("Option 5");
    expect(frame).not.toContain("Option 2");
  });
});
