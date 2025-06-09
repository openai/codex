import { describe, it, expect } from "vitest";
import { approximateTokensUsed } from "../src/utils/approximate-tokens-used";
import type { ResponseItem } from "openai/resources/responses/responses.mjs";

describe("approximateTokensUsed", () => {
  it("handles function_call_output without output", () => {
    const items: Array<ResponseItem> = [
      {
        id: "1",
        type: "function_call_output",
        status: "completed",
        call_id: "c1",
        output: undefined as unknown as string,
      } as ResponseItem,
    ];
    expect(() => approximateTokensUsed(items)).not.toThrow();
    expect(approximateTokensUsed(items)).toBe(0);
  });

  it("counts characters when output is present", () => {
    const items: Array<ResponseItem> = [
      {
        id: "1",
        type: "function_call_output",
        status: "completed",
        call_id: "c1",
        output: "abcde",
      } as ResponseItem,
    ];
    expect(approximateTokensUsed(items)).toBe(2); // ceil(5/4)
  });

  it("handles missing text or refusal fields", () => {
    const items: Array<ResponseItem> = [
      {
        id: "1",
        type: "message",
        role: "assistant",
        status: "completed",
        content: [
          { type: "output_text", text: undefined as unknown as string },
          { type: "refusal", refusal: undefined as unknown as string },
        ],
      } as ResponseItem,
    ];

    expect(() => approximateTokensUsed(items)).not.toThrow();
    expect(approximateTokensUsed(items)).toBe(0);
  });

  it("handles null text or refusal fields", () => {
    const items: Array<ResponseItem> = [
      {
        id: "1",
        type: "message",
        role: "assistant",
        status: "completed",
        content: [
          { type: "output_text", text: null as unknown as string },
          { type: "refusal", refusal: null as unknown as string },
        ],
      } as ResponseItem,
    ];

    expect(() => approximateTokensUsed(items)).not.toThrow();
    expect(approximateTokensUsed(items)).toBe(0);
  });
});
