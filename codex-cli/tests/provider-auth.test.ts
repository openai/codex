import { getApiKey } from "../src/utils/config";
import { describe, it, expect, beforeEach, afterAll } from "vitest";

describe("Provider Authentication", () => {
  // Store original env variables
  const ENV_VARS = {
    OPENAI_API_KEY: process.env["OPENAI_API_KEY"],
    OPENROUTER_API_KEY: process.env["OPENROUTER_API_KEY"],
    ANTHROPIC_API_KEY: process.env["ANTHROPIC_API_KEY"],
    AZURE_OPENAI_API_KEY: process.env["AZURE_OPENAI_API_KEY"],
    GEMINI_API_KEY: process.env["GEMINI_API_KEY"],
    OLLAMA_API_KEY: process.env["OLLAMA_API_KEY"],
    MISTRAL_API_KEY: process.env["MISTRAL_API_KEY"],
    DEEPSEEK_API_KEY: process.env["DEEPSEEK_API_KEY"],
    XAI_API_KEY: process.env["XAI_API_KEY"],
    GROQ_API_KEY: process.env["GROQ_API_KEY"],
    ARCEEAI_API_KEY: process.env["ARCEEAI_API_KEY"],
  };

  beforeEach(() => {
    // Reset env variables before each test
    Object.keys(ENV_VARS).forEach((key) => {
      delete process.env[key];
    });
  });

  afterAll(() => {
    // Restore original env variables
    Object.entries(ENV_VARS).forEach(([key, value]) => {
      if (value) {
        process.env[key] = value;
      }
    });
  });

  describe("OpenAI Provider", () => {
    it("allows using OPENAI_API_KEY environment variable", () => {
      process.env["OPENAI_API_KEY"] = "sk-test-openai";
      expect(getApiKey("openai")).toBe("sk-test-openai");
    });

    it("returns undefined when no API key is available", () => {
      expect(getApiKey("openai")).toBeUndefined();
    });
  });

  describe("Other Providers", () => {
    it("throws error for unknown provider", () => {
      expect(() => getApiKey("unknown-provider")).toThrow(/Unknown provider/);
    });

    it("returns API key for OpenRouter when environment variable is set", () => {
      process.env["OPENROUTER_API_KEY"] = "sk-test-router";
      expect(getApiKey("openrouter")).toBe("sk-test-router");
    });

    it("throws descriptive error for OpenRouter when env variable is missing", () => {
      expect(() => getApiKey("openrouter")).toThrow(
        /OpenRouter API key not found/,
      );
    });

    it("returns API key for Anthropic when environment variable is set", () => {
      process.env["ANTHROPIC_API_KEY"] = "sk-test-anthropic";
      expect(getApiKey("anthropic")).toBe("sk-test-anthropic");
    });

    it("throws descriptive error for Anthropic when env variable is missing", () => {
      expect(() => getApiKey("anthropic")).toThrow(
        /Anthropic API key not found/,
      );
    });

    it("returns API key for AzureOpenAI when environment variable is set", () => {
      process.env["AZURE_OPENAI_API_KEY"] = "sk-test-azure";
      expect(getApiKey("azure")).toBe("sk-test-azure");
    });

    it("throws descriptive error for AzureOpenAI when env variable is missing", () => {
      expect(() => getApiKey("azure")).toThrow(/AzureOpenAI API key not found/);
    });

    it("returns API key for Gemini when environment variable is set", () => {
      process.env["GEMINI_API_KEY"] = "sk-test-gemini";
      expect(getApiKey("gemini")).toBe("sk-test-gemini");
    });

    it("throws descriptive error for Gemini when env variable is missing", () => {
      expect(() => getApiKey("gemini")).toThrow(/Gemini API key not found/);
    });

    it("returns API key for Ollama when environment variable is set", () => {
      process.env["OLLAMA_API_KEY"] = "sk-test-ollama";
      expect(getApiKey("ollama")).toBe("sk-test-ollama");
    });

    it("throws descriptive error for Ollama when env variable is missing", () => {
      expect(() => getApiKey("ollama")).toThrow(/Ollama API key not found/);
    });

    it("returns API key for Mistral when environment variable is set", () => {
      process.env["MISTRAL_API_KEY"] = "sk-test-mistral";
      expect(getApiKey("mistral")).toBe("sk-test-mistral");
    });

    it("throws descriptive error for Mistral when env variable is missing", () => {
      expect(() => getApiKey("mistral")).toThrow(/Mistral API key not found/);
    });

    it("returns API key for DeepSeek when environment variable is set", () => {
      process.env["DEEPSEEK_API_KEY"] = "sk-test-deepseek";
      expect(getApiKey("deepseek")).toBe("sk-test-deepseek");
    });

    it("throws descriptive error for DeepSeek when env variable is missing", () => {
      expect(() => getApiKey("deepseek")).toThrow(/DeepSeek API key not found/);
    });

    it("returns API key for xAI when environment variable is set", () => {
      process.env["XAI_API_KEY"] = "sk-test-xai";
      expect(getApiKey("xai")).toBe("sk-test-xai");
    });

    it("throws descriptive error for xAI when env variable is missing", () => {
      expect(() => getApiKey("xai")).toThrow(/xAI API key not found/);
    });

    it("returns API key for Groq when environment variable is set", () => {
      process.env["GROQ_API_KEY"] = "sk-test-groq";
      expect(getApiKey("groq")).toBe("sk-test-groq");
    });

    it("throws descriptive error for Groq when env variable is missing", () => {
      expect(() => getApiKey("groq")).toThrow(/Groq API key not found/);
    });

    it("returns API key for ArceeAI when environment variable is set", () => {
      process.env["ARCEEAI_API_KEY"] = "sk-test-arceeai";
      expect(getApiKey("arceeai")).toBe("sk-test-arceeai");
    });

    it("throws descriptive error for ArceeAI when env variable is missing", () => {
      expect(() => getApiKey("arceeai")).toThrow(/ArceeAI API key not found/);
    });
  });
});
