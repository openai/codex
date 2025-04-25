// Plain Enter (CR) should submit.

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

describe("MultilineTextEditor – Enter submits (CR)", () => {
  it("calls onSubmit when \r is received", async () => {
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
    await type(stdin, "\r", flush);

    await flush();

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit.mock.calls[0]![0]).toBe("hello");

    cleanup();
  });
});
