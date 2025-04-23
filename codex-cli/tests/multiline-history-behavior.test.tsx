/* --------------------------------------------------------------------------
 *  Regression test – chat history navigation (↑/↓) should *only* activate
 *  once the caret reaches the very first / last line of the multiline input.
 *
 *  Current buggy behaviour: TerminalChatInput intercepts the up‑arrow at the
 *  outer <useInput> handler regardless of the caret row, causing an immediate
 *  history recall even when the user is still somewhere within a multi‑line
 *  draft.  The test captures the *expected* behaviour (matching e.g. Bash,
 *  zsh, Readline, etc.) – the ↑ key must first move the caret vertically to
 *  the topmost row; only a *subsequent* press should start cycling through
 *  previous messages.
 *
 *  The spec is written *before* the fix so we mark it as an expected failure
 *  (it.todo) until the implementation is aligned.
 * ----------------------------------------------------------------------- */

import { renderTui } from "./ui-test-helpers.js";
import * as React from "react";
import { describe, it, expect, vi } from "vitest";

// ---------------------------------------------------------------------------
//  Module mocks *must* be registered *before* the module under test is
//  imported so that Vitest can replace the dependency during evaluation.
// ---------------------------------------------------------------------------

// The chat‑input component relies on an async helper that performs filesystem
// work when images are referenced.  Mock it so our unit test remains fast and
// free of side‑effects.
vi.mock("../src/utils/input-utils.js", () => ({
  createInputItem: vi.fn(async (text: string /*, images: Array<string> */) => ({
    role: "user",
    type: "message",
    content: [{ type: "input_text", text }],
  })),
}));

// Mock the optional ../src/* dependencies so the dynamic import in parsers.ts
// does not fail during the test environment where the alias isn't configured.
vi.mock("../src/format-command.js", () => ({
  formatCommandForDisplay: (cmd: Array<string>) => cmd.join(" "),
}));
vi.mock("../src/approvals.js", () => ({
  isSafeCommand: (_cmd: Array<string>) => null,
}));

// After mocks are in place we can safely import the component under test.
import TerminalChatInput from "../src/components/chat/terminal-chat-new-input.js";

// Tiny helper mirroring the one used in other UI tests so we can await Ink's
// internal promises between keystrokes.
async function type(
  stdin: NodeJS.WritableStream,
  text: string,
  flush: () => Promise<void>,
) {
  stdin.write(text);
  await flush();
}

/** Build a set of no-op callbacks so <TerminalChatInput> renders with minimal
 *  scaffolding.
 */
function stubProps(): any {
  return {
    isNew: true,
    loading: false,
    submitInput: vi.fn(),
    confirmationPrompt: null,
    submitConfirmation: vi.fn(),
    setLastResponseId: vi.fn(),
    // Cast to any to satisfy the generic React.Dispatch signature without
    // pulling the ResponseItem type into the test bundle.
    setItems: (() => {}) as any,
    contextLeftPercent: 100,
    openOverlay: vi.fn(),
    openModelOverlay: vi.fn(),
    openHelpOverlay: vi.fn(),
    interruptAgent: vi.fn(),
    active: true,
  };
}

describe("TerminalChatInput – history navigation with multiline drafts", () => {
  it("should not recall history until caret is on the first line", async () => {
    const { stdin, lastFrameStripped, flush, cleanup } = renderTui(
      React.createElement(TerminalChatInput, stubProps()),
    );

    // -------------------------------------------------------------------
    // 1.  Submit one previous message so that history isn't empty.
    // -------------------------------------------------------------------
    for (const ch of ["p", "r", "e", "v"]) {
      await type(stdin, ch, flush);
    }
    await type(stdin, "\r", flush); // <Enter/Return> submits the text

    // Let the async onSubmit finish (mocked so it's immediate, but flush once
    // more to allow state updates to propagate).
    await flush();

    // -------------------------------------------------------------------
    // 2.  Start a *multi‑line* draft so that the caret ends up on row 1.
    // -------------------------------------------------------------------
    await type(stdin, "line1", flush);
    await type(stdin, "\n", flush); // newline inside the editor (Shift+Enter)
    await type(stdin, "line2", flush);

    // Sanity‑check – both lines should be visible in the current frame.
    const frameBefore = lastFrameStripped();
    expect(frameBefore.includes("line1")).toBe(true);
    expect(frameBefore.includes("line2")).toBe(true);

    // -------------------------------------------------------------------
    // 3.  Press ↑ once.  Expected: caret moves from (row:1) -> (row:0) but
    //     NO history recall yet, so the text stays unchanged.
    // -------------------------------------------------------------------
    await type(stdin, "\x1b[A", flush); // up‑arrow

    const frameAfter = lastFrameStripped();

    // The buffer should be unchanged – we *haven't* entered history‑navigation
    // mode yet because the caret only moved vertically inside the draft.
    expect(frameAfter.includes("prev")).toBe(false);
    expect(frameAfter.includes("line1")).toBe(true);

    cleanup();
  });

  it("should restore the draft when navigating forward (↓) past the newest history entry", async () => {
    const { stdin, lastFrameStripped, flush, cleanup } = renderTui(
      React.createElement(TerminalChatInput, stubProps()),
    );

    // Submit one message so we have history to recall later.
    for (const ch of ["p", "r", "e", "v"]) {
      await type(stdin, ch, flush);
    }
    await type(stdin, "\r", flush); // <Enter> – submit
    await flush();

    // Begin a multi‑line draft that we'll want to recover later.
    await type(stdin, "draft1", flush);
    await type(stdin, "\n", flush); // newline inside editor
    await type(stdin, "draft2", flush);

    // Record the frame so we can later assert that it comes back.
    const draftFrame = lastFrameStripped();
    expect(draftFrame.includes("draft1")).toBe(true);
    expect(draftFrame.includes("draft2")).toBe(true);

    // ────────────────────────────────────────────────────────────────────
    // 1) Hit ↑ twice: first press just moves the caret to row‑0, second
    //    enters history mode and shows the previous message ("prev").
    // ────────────────────────────────────────────────────────────────────
    await type(stdin, "\x1b[A", flush); // first up – vertical move only
    await type(stdin, "\x1b[A", flush); // second up – recall history

    const historyFrame = lastFrameStripped();
    expect(historyFrame.includes("prev")).toBe(true);

    // 2) Hit ↓ once – should exit history mode and restore the original draft
    //    (multi‑line input).
    await type(stdin, "\x1b[B", flush); // down‑arrow

    const restoredFrame = lastFrameStripped();
    expect(restoredFrame.includes("draft1")).toBe(true);
    expect(restoredFrame.includes("draft2")).toBe(true);

    cleanup();
  });
});
