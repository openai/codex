import { renderTui } from "./ui-test-helpers.js";
import { Markdown } from "../src/components/chat/terminal-chat-response-item.js";
import React from "react";
import { it, expect } from "vitest";

/** Simple sanity check that the Markdown component renders bold/italic text.
 * We strip ANSI codes, so the output should contain the raw words. */
it("renders basic markdown", () => {
  const { lastFrameStripped } = renderTui(
    <Markdown>**bold** _italic_</Markdown>,
  );

  const frame = lastFrameStripped();
  expect(frame).toContain("bold");
  expect(frame).toContain("italic");
});
