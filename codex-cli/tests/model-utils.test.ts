import { describe, test, expect } from "vitest";
import {
  calculateContextPercentRemaining,
  maxTokensForModel,
} from "../src/utils/model-utils";
import { openAiModelInfo } from "../src/utils/model-info";
import type { ResponseItem } from "openai/resources/responses/responses.mjs";

describe("Model Utils", () => {
  describe("openAiModelInfo", () => {
    test("model info entries have required properties", () => {
      Object.entries(openAiModelInfo).forEach(([_, info]) => {
        expect(info).toHaveProperty("label");
        expect(info).toHaveProperty("maxContextLength");
        expect(typeof info.label).toBe("string");
        expect(typeof info.maxContextLength).toBe("number");
      });
    });
  });

  describe("maxTokensForModel", () => {
    test("returns correct token limit for known models", () => {
      const knownModel = "gpt-4o";
      const expectedTokens = openAiModelInfo[knownModel].maxContextLength;
      expect(maxTokensForModel(knownModel)).toBe(expectedTokens);
    });

    test("handles models with size indicators in their names", () => {
      expect(maxTokensForModel("some-model-32k")).toBe(32000);
      expect(maxTokensForModel("some-model-16k")).toBe(16000);
      expect(maxTokensForModel("some-model-8k")).toBe(8000);
      expect(maxTokensForModel("some-model-4k")).toBe(4000);
    });

    test("defaults to 128k for unknown models not in the registry", () => {
      expect(maxTokensForModel("completely-unknown-model")).toBe(128000);
    });
  });

  describe("calculateContextPercentRemaining", () => {
    test("returns 100% for empty items", () => {
      const result = calculateContextPercentRemaining([], "gpt-4o");
      expect(result).toBe(100);
    });

    test("calculates percentage correctly for non-empty items", () => {
      const mockItems: Array<ResponseItem> = [
        {
          id: "test-id",
          type: "message",
          role: "user",
          status: "completed",
          content: [
            {
              type: "input_text",
              text: "A".repeat(
                openAiModelInfo["gpt-4o"].maxContextLength * 0.25 * 4,
              ),
            },
          ],
        } as ResponseItem,
      ];

      const result = calculateContextPercentRemaining(mockItems, "gpt-4o");
      expect(result).toBeCloseTo(75, 0);
    });

    test("handles models that are not in the registry", () => {
      const mockItems: Array<ResponseItem> = [];

      const result = calculateContextPercentRemaining(
        mockItems,
        "unknown-model",
      );
      expect(result).toBe(100);
    });
  });
});
