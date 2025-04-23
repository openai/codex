// Regression test: Some terminals buffer printable characters together with
// the modifyOtherKeys CSI‑u sequence in the *same* chunk.  Prior versions of
// the editor only recognised the sequence when it appeared at the *start* of
// the input chunk, causing the control codes to be inserted verbatim instead
// of being interpreted.  This test types the characters "abc" followed by the
// CSI‑u sequence for Shift+Enter (ESC [ 13 ; 2 u) in **one** write operation
// and verifies that a newline is inserted (without triggering submission).

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

describe("MultilineTextEditor – Shift+Enter combined with preceding text", () => {
  it("handles text + CSI-u in same chunk (newline, no submit)", async () => {
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

    // Write "abc" immediately followed by the CSI‑u Shift+Enter sequence in a
    // *single* chunk.  The ESC (\u001B) prefix is included to mimic a real
    // terminal.
    await type(stdin, "abc\u001B[13;2u", flush);

    // Continue typing after the newline.
    await type(stdin, "def", flush);

    const frame = lastFrameStripped();

    // Expect both segments to appear, separated by a newline (=> at least two
    // rendered lines).
    expect(frame).toMatch(/abc/);
    expect(frame).toMatch(/def/);
    expect(frame.split("\n").length).toBeGreaterThanOrEqual(2);

    // Shift+Enter must not have triggered submission.
    expect(onSubmit).not.toHaveBeenCalled();

    cleanup();
  });
});
