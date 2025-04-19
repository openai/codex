import { describe, expect, it } from "vitest";

import type { ResponseItem } from "openai/resources/responses/responses.mjs";

import { calculateContextPercentRemaining } from "../src/components/chat/terminal-chat-utils.js";

function makeUserMessage(id: string, text: string): ResponseItem {
  return {
    id,
    type: "message",
    role: "user",
    content: [{ type: "input_text", text }],
  } as ResponseItem;
}

describe("calculateContextPercentRemaining", () => {
  it("includes extra context characters in calculation", () => {
    const msgText = "a".repeat(40); // 40 chars → 10 tokens
    const items = [makeUserMessage("1", msgText)];

    const model = "gpt-4-16k";

    const base = calculateContextPercentRemaining(items, model);
    const withExtra = calculateContextPercentRemaining(items, model, 8); // +8 chars → +2 tokens

    expect(withExtra).toBeLessThan(base);
  });
});
