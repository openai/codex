import { describe, expect, test } from "vitest";
import { openAiModelInfo } from "../src/utils/model-info";

describe("Model Info", () => {
  test("supportedModelInfo contains expected models", () => {
    expect(openAiModelInfo).toHaveProperty("gpt-4o");
    expect(openAiModelInfo).toHaveProperty("gpt-4.1");
    expect(openAiModelInfo).toHaveProperty("o3");
  });

  test("model info entries have required properties", () => {
    Object.entries(openAiModelInfo).forEach(([_, info]) => {
      expect(info).toHaveProperty("label");
      expect(info).toHaveProperty("maxContextLength");
      expect(typeof info.label).toBe("string");
      expect(typeof info.maxContextLength).toBe("number");
    });
  });
});
