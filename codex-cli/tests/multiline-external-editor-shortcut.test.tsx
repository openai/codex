import { renderTui } from "./ui-test-helpers.js";
import MultilineTextEditor from "../src/components/chat/multiline-editor.js";
import TextBuffer from "../src/text-buffer.js";
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

describe("MultilineTextEditor – external editor shortcut", () => {
  it("fires openInExternalEditor on Ctrl‑E (single key)", async () => {
    const spy = vi
      .spyOn(TextBuffer.prototype as any, "openInExternalEditor")
      .mockResolvedValue(undefined);

    const { stdin, flush, cleanup } = renderTui(
      React.createElement(MultilineTextEditor, {
        initialText: "hello",
        width: 20,
        height: 3,
      }),
    );

    // Ensure initial render.
    await flush();

    // Send Ctrl‑E → should fire immediately
    await type(stdin, "\x05", flush); // Ctrl‑E (ENQ / 0x05)
    expect(spy).toHaveBeenCalledTimes(1);

    spy.mockRestore();
    cleanup();
  });

  it("fires openInExternalEditor on Ctrl‑X (single key)", async () => {
    const spy = vi
      .spyOn(TextBuffer.prototype as any, "openInExternalEditor")
      .mockResolvedValue(undefined);

    const { stdin, flush, cleanup } = renderTui(
      React.createElement(MultilineTextEditor, {
        initialText: "hello",
        width: 20,
        height: 3,
      }),
    );

    // Ensure initial render.
    await flush();

    // Send Ctrl‑X → should fire immediately
    await type(stdin, "\x18", flush); // Ctrl‑X (SUB / 0x18)
    expect(spy).toHaveBeenCalledTimes(1);

    spy.mockRestore();
    cleanup();
  });
});
