import { describe, test, expect } from "vitest";
import {
  loadPrompts,
  adaptPromptForProvider,
  getProviderForModel,
} from "../src/prompts/loader";
import { allModelInfo } from "../src/utils/model-info";

describe("Prompt Loader", () => {
  describe("getProviderForModel", () => {
    test("correctly identifies provider for Claude models", () => {
      expect(getProviderForModel("claude-3-opus")).toBe("anthropic");
      expect(getProviderForModel("claude-3-sonnet")).toBe("anthropic");
      expect(getProviderForModel("claude-3-haiku")).toBe("anthropic");
    });

    test("correctly identifies provider for Gemini models", () => {
      expect(getProviderForModel("gemini-1.5-pro")).toBe("gemini");
      expect(getProviderForModel("gemini-1.5-flash")).toBe("gemini");
    });

    test("defaults to OpenAI for other models", () => {
      expect(getProviderForModel("gpt-4")).toBe("openai");
      expect(getProviderForModel("gpt-4o")).toBe("openai");
      expect(getProviderForModel("unknown-model")).toBe("openai");
    });
  });

  describe("adaptPromptForProvider", () => {
    const testPrompt = "You are Codex.\n\nTOOL USE\n\nThis is a test prompt.";

    test("adapts prompts for Claude/Anthropic models", () => {
      const adapted = adaptPromptForProvider(testPrompt, "anthropic");
      expect(adapted).toContain("CRITICALLY IMPORTANT INSTRUCTIONS");
      expect(adapted).toContain("WAIT for confirmation");
    });

    test("adapts prompts for Gemini models", () => {
      const adapted = adaptPromptForProvider(testPrompt, "gemini");
      expect(adapted).toContain("FOLLOW THESE STEPS PRECISELY");
      expect(adapted).toContain("IMPORTANT GUIDELINES FOR GEMINI MODELS");
    });

    test("adapts prompts for OpenAI models", () => {
      const adapted = adaptPromptForProvider(testPrompt, "openai");
      expect(adapted).toContain("FINAL REMINDER");
      expect(adapted).toContain(testPrompt); // Should contain the original prompt
    });
  });

  describe("loadPrompts", () => {
    test("loads prompts with all expected properties", () => {
      const prompts = loadPrompts({
        cwd: "/test/directory",
        provider: "openai",
        model: "gpt-4o",
      });

      // Check all expected prompt collections are present
      expect(prompts).toHaveProperty("systemPrompt");
      expect(prompts).toHaveProperty("newTaskResponse");
      expect(prompts).toHaveProperty("condenseResponse");
      expect(prompts).toHaveProperty("planModeResponse");
      expect(prompts).toHaveProperty("mcpDocumentationResponse");
      expect(prompts).toHaveProperty("askFollowupQuestionResponse");
      expect(prompts).toHaveProperty("attemptCompletionResponse");
      expect(prompts).toHaveProperty("listCodeDefinitionNamesResponse");
      expect(prompts).toHaveProperty("browserActionResponse");
      expect(prompts).toHaveProperty("useMcpToolResponse");
      expect(prompts).toHaveProperty("accessMcpResourceResponse");
    });

    test("includes cwd in system prompt", () => {
      const testCwd = "/test/directory";
      const prompts = loadPrompts({
        cwd: testCwd,
      });

      expect(prompts.systemPrompt).toContain(testCwd);
    });

    test("adds user instructions when provided", () => {
      const userInstructions = "These are custom instructions.";
      const prompts = loadPrompts({
        cwd: "/test/directory",
        userInstructions,
      });

      expect(prompts.systemPrompt).toContain(userInstructions);
    });
  });

  describe("Model Integration", () => {
    test("combined modelInfo contains models from all providers", () => {
      // Check for OpenAI models
      expect(allModelInfo).toHaveProperty("gpt-4o");
      expect(allModelInfo).toHaveProperty("gpt-4.1");

      // Check for Anthropic/Claude models
      expect(allModelInfo).toHaveProperty("claude-3-opus");
      expect(allModelInfo).toHaveProperty("claude-3-sonnet");
      expect(allModelInfo).toHaveProperty("claude-3-haiku");

      // Check for Gemini models
      expect(allModelInfo).toHaveProperty("gemini-1.5-pro");
      expect(allModelInfo).toHaveProperty("gemini-1.5-flash");
    });

    test("model info entries have required properties across all providers", () => {
      // Sample models from each provider
      const sampleModels = [
        "gpt-4o", // OpenAI
        "claude-3-opus", // Anthropic
        "gemini-1.5-pro", // Gemini
      ];

      sampleModels.forEach((modelName) => {
        // Add type assertion to fix TypeScript error
        const info = allModelInfo[modelName as keyof typeof allModelInfo];
        expect(info).toBeDefined();
        expect(info).toHaveProperty("label");
        expect(info).toHaveProperty("maxContextLength");
        expect(typeof info.label).toBe("string");
        expect(typeof info.maxContextLength).toBe("number");
      });
    });
  });
});
