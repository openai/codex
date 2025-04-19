import { describe, expect, test } from "vitest";

import { estimateCostUSD } from "../src/utils/estimate-cost.js";
import { SessionCostTracker } from "../src/utils/session-cost.js";
import type { ResponseItem } from "openai/resources/responses/responses.mjs";

// Helper to craft a minimal ResponseItem for tests
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

describe("estimateCostUSD", () => {
  test("returns a proportional, positive estimate for known models", () => {
    const items: Array<ResponseItem> = [
      makeMessage("1", "user", "hello world"),
      makeMessage("2", "assistant", "hi there"),
    ];

    const cost = estimateCostUSD(items, "gpt-3.5-turbo");
    expect(cost).not.toBeNull();
    expect(cost!).toBeGreaterThan(0);

    // Adding another token should increase the estimate
    const cost2 = estimateCostUSD(
      items.concat([makeMessage("3", "user", "extra")]),
      "gpt-3.5-turbo",
    );
    expect(cost2!).toBeGreaterThan(cost!);
  });
});

describe("SessionCostTracker", () => {
  test("accumulates items and reports tokens & cost", () => {
    const tracker = new SessionCostTracker("gpt-3.5-turbo");
    tracker.addItems([makeMessage("1", "user", "foo")]);
    tracker.addItems([makeMessage("2", "assistant", "bar baz")]);

    expect(tracker.getTokensUsed()).toBeGreaterThan(0);
    expect(tracker.getCostUSD()!).toBeGreaterThan(0);
  });
});
