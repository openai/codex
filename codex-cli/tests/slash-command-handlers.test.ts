import { describe, it, expect, vi } from "vitest";
import { handleSlashCommand } from "../src/utils/slash-command-handlers";
import type { SlashCommandHandlers } from "../src/utils/slash-command-handlers";

function createMocks(): SlashCommandHandlers {
  return {
    setInput: vi.fn(),
    openOverlay: vi.fn(),
    openHelpOverlay: vi.fn(),
    openDiffOverlay: vi.fn(),
    onCompact: vi.fn(),
    openModelOverlay: vi.fn(),
    openApprovalOverlay: vi.fn(),
    toggleFlexMode: vi.fn(),
  };
}

describe("handleSlashCommand", () => {
  it("returns false for non-slash commands", () => {
    const mocks = createMocks();
    const result = handleSlashCommand("hello", mocks);
    expect(result).toBe(false);
    // No handler should have been called
    Object.values(mocks).forEach((fn) => expect(fn).not.toHaveBeenCalled());
  });

  it("handles /flex-mode by toggling flexMode and clearing input", () => {
    const mocks = createMocks();
    const result = handleSlashCommand("/flex-mode", mocks);
    expect(result).toBe(true);
    expect(mocks.setInput).toHaveBeenCalledWith("");
    expect(mocks.toggleFlexMode).toHaveBeenCalledTimes(1);
  });

  it.each([
    ["/history", "openOverlay"],
    ["/help", "openHelpOverlay"],
    ["/diff", "openDiffOverlay"],
    ["/compact", "onCompact"],
  ])("%s invokes %s and clears input", (cmd, handlerName) => {
    const mocks = createMocks();
    const result = handleSlashCommand(cmd, mocks);
    expect(result).toBe(true);
    expect(mocks.setInput).toHaveBeenCalledWith("");
    // @ts-ignore
    expect(mocks[handlerName]).toHaveBeenCalledTimes(1);
  });

  it("handles /model and /model xyz by opening model overlay and clearing input", () => {
    const mocks = createMocks();
    expect(handleSlashCommand("/model", mocks)).toBe(true);
    expect(mocks.setInput).toHaveBeenCalledWith("");
    expect(mocks.openModelOverlay).toHaveBeenCalledTimes(1);
    // Test with argument
    const mocks2 = createMocks();
    expect(handleSlashCommand("/model davinci", mocks2)).toBe(true);
    expect(mocks2.setInput).toHaveBeenCalledWith("");
    expect(mocks2.openModelOverlay).toHaveBeenCalledTimes(1);
  });

  it("handles /approval and /approval xyz by opening approval overlay and clearing input", () => {
    const mocks = createMocks();
    expect(handleSlashCommand("/approval", mocks)).toBe(true);
    expect(mocks.setInput).toHaveBeenCalledWith("");
    expect(mocks.openApprovalOverlay).toHaveBeenCalledTimes(1);
    const mocks2 = createMocks();
    expect(handleSlashCommand("/approval strict", mocks2)).toBe(true);
    expect(mocks2.setInput).toHaveBeenCalledWith("");
    expect(mocks2.openApprovalOverlay).toHaveBeenCalledTimes(1);
  });
});
