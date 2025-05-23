import { getApiKey } from "../src/utils/config";
import { describe, it, expect, beforeEach, afterAll, vi } from "vitest";

describe("API Key Handling", () => {
  const OLD_OPENROUTER = process.env["OPENROUTER_API_KEY"];
  const OLD_OPENAI = process.env["OPENAI_API_KEY"];

  beforeEach(() => {
    delete process.env["OPENROUTER_API_KEY"];
    delete process.env["OPENAI_API_KEY"];
  });

  afterAll(() => {
    process.env["OPENROUTER_API_KEY"] = OLD_OPENROUTER;
    process.env["OPENAI_API_KEY"] = OLD_OPENAI;
  });

  describe("Non-OpenAI providers", () => {
    it("shows error message without OPENROUTER_API_KEY", () => {
      const mockExit = vi.fn();
      process.exit = mockExit as any;

      expect(() => getApiKey("openrouter")).toThrow(
        "OpenRouter API key not found",
      );
    });

    it("returns the key when set for OpenRouter", () => {
      process.env["OPENROUTER_API_KEY"] = "sk-test-router";
      expect(getApiKey("openrouter")).toBe("sk-test-router");
    });

    it("shows error message for other non-OpenAI providers", () => {
      expect(() => getApiKey("groq")).toThrow("Groq API key not found");
    });
  });

  describe("OpenAI provider", () => {
    it("returns undefined without OPENAI_API_KEY", () => {
      expect(getApiKey("openai")).toBeUndefined();
    });

    it("returns the key when set for OpenAI", () => {
      process.env["OPENAI_API_KEY"] = "sk-test-openai";
      expect(getApiKey("openai")).toBe("sk-test-openai");
    });
  });

  it("throws error for unsupported provider", () => {
    expect(() => getApiKey("unsupported")).toThrow(
      "Unknown provider: unsupported",
    );
  });
});
