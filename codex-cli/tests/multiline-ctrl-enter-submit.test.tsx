// Ctrl+Enter (CSI‑u 13;5u) should submit the buffer.

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

describe("MultilineTextEditor – Ctrl+Enter submits", () => {
  it("calls onSubmit when CSI 13;5u is received", async () => {
    const onSubmit = vi.fn();

    const { stdin, flush, cleanup } = renderTui(
      React.createElement(MultilineTextEditor, {
        height: 5,
        width: 20,
        onSubmit,
      }),
    );

    await flush();

    await type(stdin, "hello", flush);
    await type(stdin, "\u001B[13;5u", flush); // Ctrl+Enter (modifier 5 = Ctrl)

    await flush();

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0]![0]).toBe("hello");

    cleanup();
  });
});
