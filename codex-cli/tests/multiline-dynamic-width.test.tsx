// These tests exercise MultilineTextEditor behaviour when the editor width is
// *not* provided via props so that it has to derive its width from the current
// terminal size.  We emulate a terminal‑resize by mutating
// `process.stdout.columns` and emitting a synthetic `resize` event – the
// `useTerminalSize` hook listens for that and causes the component to
// re‑render.  The test then asserts that
//   1.  The rendered line re‑wraps to the new width, *and*
//   2.  The caret (highlighted inverse character) is still kept in view after
//       the horizontal shrink so that editing remains possible.

import { renderTui } from "./ui-test-helpers.js";
import MultilineTextEditor from "../src/components/chat/multiline-editor.js";
import * as React from "react";
import { describe, it, expect } from "vitest";

// Helper to synchronously type text then flush Ink's timers so that the next
// `lastFrame()` call sees the updated UI.
async function type(
  stdin: NodeJS.WritableStream,
  text: string,
  flush: () => Promise<void>,
) {
  stdin.write(text);
  await flush();
}

describe("MultilineTextEditor – dynamic width", () => {
  // The dynamic horizontal scroll logic is still flaky – mark as an expected
  // *failing* test so it doesn't break CI until the feature is aligned with
  // the Rust implementation.
  it("keeps the caret visible when the terminal width shrinks", async () => {
    // Fake an initial terminal width large enough that no horizontal
    // scrolling is required while we type the long alphabet sequence.
    process.stdout.columns = 40; // width seen by useTerminalSize (after padding)

    const { stdin, lastFrame, flush, cleanup } = renderTui(
      React.createElement(MultilineTextEditor, {
        initialText: "",
        // width *omitted* – component should fall back to terminal columns
        height: 3,
      }),
    );

    // Ensure initial render completes.
    await flush();

    // Type the alphabet – longer than the width we'll shrink to.
    const alphabet = "abcdefghijklmnopqrstuvwxyz";
    await type(stdin, alphabet, flush);

    // The cursor (block) now sits on the far right after the 'z'. Verify that
    // the character 'z' is visible in the current frame.
    expect(lastFrame()?.includes("z")).toBe(true);

    /* -----------------------  Simulate resize  ----------------------- */

    // Shrink the reported terminal width so that the previously visible slice
    // would no longer include the cursor *unless* the editor re‑computes
    // scroll offsets on re‑render.
    process.stdout.columns = 20; // shrink significantly (remember: padding‑8)
    process.stdout.emit("resize"); // notify listeners

    // Allow Ink to schedule the state update and then perform the re‑render.
    await flush();
    await flush();

    // After the resize the editor should have scrolled horizontally so that
    // the caret (and thus the 'z' character that is block‑highlighted) remains
    // visible in the rendered slice.
    const frameAfter = lastFrame() || "";
    // eslint-disable-next-line no-console
    console.log("FRAME AFTER RESIZE:\n" + frameAfter);
    expect(frameAfter.includes("z")).toBe(true);

    cleanup();
  });
});
