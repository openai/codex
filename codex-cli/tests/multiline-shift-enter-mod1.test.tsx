// Regression test: Terminals with modifyOtherKeys=1 emit CSI~ sequence for
// Shift+Enter: ESC [ 27 ; mod ; 13 ~.  The editor must treat Shift+Enter as
// newline (without submitting) and Ctrl+Enter as submit.

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

describe("MultilineTextEditor – Shift+Enter with modifyOtherKeys=1", () => {
  it("inserts newline, does NOT submit", async () => {
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

    await type(stdin, "abc", flush);
    // Shift+Enter => ESC [27;2;13~
    await type(stdin, "\u001B[27;2;13~", flush);
    await type(stdin, "def", flush);

    const frame = lastFrameStripped();
    expect(frame).toMatch(/abc/);
    expect(frame).toMatch(/def/);
    // newline inserted -> at least 2 lines
    expect(frame.split("\n").length).toBeGreaterThanOrEqual(2);

    expect(onSubmit).not.toHaveBeenCalled();

    cleanup();
  });
});
