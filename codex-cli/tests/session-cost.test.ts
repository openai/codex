import { afterEach, describe, expect, it, vi } from "vitest";

import type { ResponseItem } from "openai/resources/responses/responses.mjs";

import {
  ensureSessionTracker,
  getSessionTracker,
  printAndResetSessionSummary,
} from "../src/utils/session-cost.js";

function makeMessage(
  id: string,
  role: "user" | "assistant",
  text: string,
): ResponseItem {
  return {
    id,
    type: "message",
    role,
    content: [{ type: role === "user" ? "input_text" : "output_text", text }],
  } as ResponseItem;
}

describe("printAndResetSessionSummary", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("/clear resets tracker so successive conversations start fresh", () => {
    const spy = vi.spyOn(console, "log").mockImplementation(() => {});

    const perSessionTokens: Array<number> = [];

    for (let i = 1; i <= 3; i++) {
      const tracker = ensureSessionTracker("gpt-3.5-turbo");
      tracker.addTokens(i * 10); // 10, 20, 30
      perSessionTokens.push(tracker.getTokensUsed());

      // Simulate user typing /clear which prints & resets
      printAndResetSessionSummary();

      expect(getSessionTracker()).toBeNull();
    }

    expect(perSessionTokens).toEqual([10, 20, 30]);

    spy.mockRestore();
  });

  it("prints a summary and resets the global tracker", () => {
    const spy = vi.spyOn(console, "log").mockImplementation(() => {});

    const tracker = ensureSessionTracker("gpt-3.5-turbo");
    tracker.addItems([
      makeMessage("1", "user", "hello"),
      makeMessage("2", "assistant", "hi"),
    ]);

    printAndResetSessionSummary();

    expect(spy).toHaveBeenCalled();
    expect(getSessionTracker()).toBeNull();
  });

  it("prefers exact token counts added via addTokens() over heuristic", () => {
    const tracker = ensureSessionTracker("gpt-3.5-turbo");

    // Add a long message (heuristic would count >1 token)
    tracker.addItems([
      makeMessage("x", "user", "a".repeat(400)), // ~100 tokens
    ]);

    const heuristicTokens = tracker.getTokensUsed();
    expect(heuristicTokens).toBeGreaterThan(50);

    // Now inject an exact low token count and ensure it overrides
    tracker.addTokens(10);
    expect(tracker.getTokensUsed()).toBe(
      heuristicTokens + (10 - heuristicTokens),
    );

    const cost = tracker.getCostUSD();
    expect(cost).not.toBeNull();
  });
});
