import { renderTui } from "./ui-test-helpers.js";
import MultilineTextEditor from "../src/components/chat/multiline-editor.js";
import * as React from "react";
import { describe, it, expect, vi } from "vitest";

// Helper that lets us type and then immediately flush ink's async timers
async function type(
  stdin: NodeJS.WritableStream,
  text: string,
  flush: () => Promise<void>,
) {
  stdin.write(text);
  await flush();
}

describe("MultilineTextEditor", () => {
  it("renders the initial text", async () => {
    const { lastFrame, cleanup, waitUntilExit } = renderTui(
      React.createElement(MultilineTextEditor, {
        initialText: "hello",
        width: 10,
        height: 3,
      }),
    );

    await waitUntilExit(); // initial render
    expect(lastFrame()?.includes("hello")).toBe(true);
    cleanup();
  });

  it("updates the buffer when typing and shows the change", async () => {
    const {
      stdin,
      lastFrame,
      cleanup,
      waitUntilExit: _,
      flush,
    } = renderTui(
      React.createElement(MultilineTextEditor, {
        initialText: "",
        width: 10,
        height: 3,
      }),
    );

    // Type "h"
    await type(stdin, "h", flush);
    expect(lastFrame()?.includes("h")).toBe(true);

    // Type "i"
    await type(stdin, "i", flush);
    expect(lastFrame()?.includes("hi")).toBe(true);

    cleanup();
  });

  it("calls onSubmit with the current text on <Esc>", async () => {
    const onSubmit = vi.fn();
    const { stdin, flush, cleanup } = renderTui(
      React.createElement(MultilineTextEditor, {
        initialText: "foo",
        width: 10,
        height: 3,
        onSubmit,
      }),
    );

    // Press Escape
    await type(stdin, "\x1b", flush);

    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledWith("foo");

    cleanup();
  });

  it("updates text when backspacing", async () => {
    const { stdin, lastFrameStripped, flush, cleanup, waitUntilExit } =
      renderTui(
        React.createElement(MultilineTextEditor, {
          initialText: "",
          width: 10,
          height: 3,
        }),
      );

    await waitUntilExit();

    // Type "hello"
    stdin.write("hello");
    await flush();
    expect(lastFrameStripped().includes("hello")).toBe(true);

    // Send 2× backspace (DEL / 0x7f)
    stdin.write("\x7f\x7f");
    await flush();

    const frame = lastFrameStripped();
    expect(frame.includes("hel")).toBe(true);
    expect(frame.includes("hell")).toBe(false);

    cleanup();
  });

  it("three consecutive backspaces after typing 'hello' leaves 'he'", async () => {
    const { stdin, lastFrameStripped, flush, cleanup, waitUntilExit } =
      renderTui(
        React.createElement(MultilineTextEditor, {
          initialText: "",
          width: 10,
          height: 3,
        }),
      );

    await waitUntilExit();

    stdin.write("hello");
    await flush();
    // 3 backspaces
    stdin.write("\x7f\x7f\x7f");
    await flush();

    const frame = lastFrameStripped();
    expect(frame.includes("he")).toBe(true);
    expect(frame.includes("hel")).toBe(false);
    expect(frame.includes("hello")).toBe(false);

    cleanup();
  });

  /* -------------------------------------------------------------- */
  /*  Caret highlighting semantics                                  */
  /* -------------------------------------------------------------- */

  it("highlights the character *under* the caret (after arrow moves)", async () => {
    const { stdin, lastFrame, flush, cleanup, waitUntilExit } = renderTui(
      React.createElement(MultilineTextEditor, {
        initialText: "",
        width: 10,
        height: 3,
      }),
    );

    await waitUntilExit();

    // Type "bar" and move caret left twice
    stdin.write("bar");
    stdin.write("\x1b[D");
    await flush();
    stdin.write("\x1b[D");
    await flush(); // ensure each arrow processed

    const frameRaw = lastFrame() || "";
    // eslint-disable-next-line no-console
    console.log("DEBUG frame:", frameRaw);
    const highlightedMatch = frameRaw.match(/\x1b\[7m(.)\x1b\[27m/);
    expect(highlightedMatch).not.toBeNull();
    const highlightedChar = highlightedMatch ? highlightedMatch[1] : null;

    expect(highlightedChar).toBe("a"); // caret should block‑highlight 'a'

    cleanup();
  });
});
