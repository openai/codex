// Regression test: Some terminals emit a carriage‑return ("\r") for
// Shift+Enter instead of a bare line‑feed.  Pressing Shift+Enter in that
// environment should insert a newline **without** triggering submission.

import { renderTui } from "./ui-test-helpers.js";
import MultilineTextEditor from "../src/components/chat/multiline-editor.js";
import * as React from "react";
import { describe, it, expect, vi } from "vitest";

async function type(
  stdin: NodeJS.WritableStream,
  text: string,
  flush: () => Promise<void>,
) {
  stdin.write(text);
  await flush();
}

describe("MultilineTextEditor - Shift+Enter (\r variant)", () => {
  it("inserts a newline and does NOT submit when the terminal sends \r for Shift+Enter", async () => {
    const onSubmit = vi.fn();

    const { stdin, lastFrameStripped, flush, cleanup } = renderTui(
      React.createElement(MultilineTextEditor, {
        height: 5,
        width: 20,
        initialText: "",
        onSubmit,
      }),
    );

    await flush();

    // Type some text then press Shift+Enter (simulated by kitty CSI-u seq).
    await type(stdin, "foo", flush);
    await type(stdin, "\u001B[13;2u", flush); // ESC [ 13 ; 2 u
    await type(stdin, "bar", flush);

    const frame = lastFrameStripped();
    expect(frame).toMatch(/foo/);
    expect(frame).toMatch(/bar/);

    // Must have inserted a newline (two rendered lines inside the frame)
    expect(frame.split("\n").length).toBeGreaterThanOrEqual(2);

    // No submission should have occurred
    expect(onSubmit).not.toHaveBeenCalled();

    cleanup();
  });
});
