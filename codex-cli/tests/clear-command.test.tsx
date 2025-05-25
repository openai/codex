import React from "react";
import type { ComponentProps } from "react";
import { describe, it, expect, vi } from "vitest";
import { renderTui } from "./ui-test-helpers.js";
import TerminalChatInput from "../src/components/chat/terminal-chat-input.js";
import * as TermUtils from "../src/utils/terminal.js";

// -------------------------------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------------------------------

async function type(
  stdin: NodeJS.WritableStream,
  text: string,
  flush: () => Promise<void>,
): Promise<void> {
  stdin.write(text);
  await flush();
}

// -------------------------------------------------------------------------------------------------
// Tests
// -------------------------------------------------------------------------------------------------

describe("/clear command", () => {
  it("invokes clearTerminal and resets context in TerminalChatInput", async () => {
    const clearSpy = vi
      .spyOn(TermUtils, "clearTerminal")
      .mockImplementation(() => {});

    const setItems = vi.fn();

    // Minimal stub of a ResponseItem – cast to bypass exhaustive type checks in this test context
    const existingItems = [
      {
        id: "dummy-1",
        type: "message",
        role: "system",
        content: [{ type: "input_text", text: "Old item" }],
      },
    ] as Array<any>;

    const props: ComponentProps<typeof TerminalChatInput> = {
      isNew: false,
      loading: false,
      submitInput: () => {},
      confirmationPrompt: null,
      explanation: undefined,
      submitConfirmation: () => {},
      setLastResponseId: () => {},
      setItems,
      contextLeftPercent: 100,
      openOverlay: () => {},
      openModelOverlay: () => {},
      openApprovalOverlay: () => {},
      openHelpOverlay: () => {},
      openDiffOverlay: () => {},
      openSessionsOverlay: () => {},
      onCompact: () => {},
      interruptAgent: () => {},
      active: true,
      thinkingSeconds: 0,
      items: existingItems,
    };

    const { stdin, flush, cleanup } = renderTui(
      <TerminalChatInput {...props} />,
    );

    await flush();

    await type(stdin, "/clear", flush);
    await type(stdin, "\r", flush); // press Enter

    // Allow any asynchronous state updates to propagate
    await flush();

    expect(clearSpy).toHaveBeenCalledTimes(2);
    expect(setItems).toHaveBeenCalledTimes(2);

    const stateUpdater = setItems.mock.calls[0]![0];
    expect(typeof stateUpdater).toBe("function");
    const newItems = stateUpdater(existingItems);
    expect(Array.isArray(newItems)).toBe(true);
    expect(newItems).toHaveLength(2);
    expect(newItems.at(-1)).toMatchObject({
      role: "system",
      type: "message",
      content: [{ type: "input_text", text: "Terminal cleared" }],
    });

    cleanup();
    clearSpy.mockRestore();
  });
});

describe("clearTerminal", () => {
  it("writes escape sequence to stdout", () => {
    const originalQuiet = process.env["CODEX_QUIET_MODE"];
    delete process.env["CODEX_QUIET_MODE"];

    process.env["CODEX_QUIET_MODE"] = "0";

    const writeSpy = vi
      .spyOn(process.stdout, "write")
      .mockImplementation(() => true);

    TermUtils.clearTerminal();

    expect(writeSpy).toHaveBeenCalledWith("\x1b[3J\x1b[H\x1b[2J");

    writeSpy.mockRestore();

    if (originalQuiet !== undefined) {
      process.env["CODEX_QUIET_MODE"] = originalQuiet;
    } else {
      delete process.env["CODEX_QUIET_MODE"];
    }
  });
});
