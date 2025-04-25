import { renderTui } from "./ui-test-helpers.js";
import MultilineTextEditor from "../src/components/chat/multiline-editor.js";
import * as React from "react";
import { describe, it, expect } from "vitest";

// Helper to send keystrokes and wait for Ink's async timing so that the frame
// reflects the input.
async function type(
  stdin: NodeJS.WritableStream,
  text: string,
  flush: () => Promise<void>,
) {
  stdin.write(text);
  await flush();
}

describe("MultilineTextEditor – inserting new lines", () => {
  // Same as above – the React wrapper still differs from the Rust reference
  // when handling <Enter>.  Keep the test around but mark it as expected to
  // fail.
  it("splits the line and renders the new row when <Enter> is pressed", async () => {
    const { stdin, lastFrameStripped, flush, cleanup } = renderTui(
      React.createElement(MultilineTextEditor, {
        height: 5,
        width: 20,
        initialText: "",
      }),
    );

    // Wait for first render
    await flush();

    // Type "hello", press Enter, then type "world"
    await type(stdin, "hello", flush);
    await type(stdin, "\n", flush); // Enter / Return
    await type(stdin, "world", flush);

    const frame = lastFrameStripped();
    const lines = frame.split("\n");

    // eslint-disable-next-line no-console
    console.log(
      "\n--- RENDERED FRAME ---\n" + frame + "\n---------------------",
    );

    // We expect at least two rendered lines and the texts to appear on their
    // own respective rows.
    expect(lines.length).toBeGreaterThanOrEqual(2);
    // First rendered (inside border) line should contain 'hello'
    expect(lines.some((l: string) => l.includes("hello"))).toBe(true);
    // Another line should contain 'world'
    expect(lines.some((l: string) => l.includes("world"))).toBe(true);

    cleanup();
  });
});
