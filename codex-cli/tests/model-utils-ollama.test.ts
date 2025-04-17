import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  getAvailableModels,
  isModelSupportedForResponses,
  RECOMMENDED_OPENAI_MODELS,
  RECOMMENDED_OLLAMA_MODELS,
  RECOMMENDED_MODELS,
} from "../src/utils/model-utils";
import * as ollamaClient from "../src/utils/ollama-client";

// Mock the ollama-client module
vi.mock("../src/utils/ollama-client", () => ({
  fetchOllamaModels: vi.fn(),
  formatOllamaModelName: vi.fn((name) => `ollama:${name}`),
  isOllamaModel: vi.fn((name) => name.startsWith("ollama:")),
}));

// Mock OpenAI
vi.mock("openai", () => {
  return {
    default: vi.fn().mockImplementation(() => ({
      models: {
        list: vi.fn().mockImplementation(() => {
          return {
            [Symbol.asyncIterator]: () => {
              let done = false;
              const models = [
                { id: "o4-mini" },
                { id: "o3" },
                { id: "gpt-4o" },
              ];
              let index = 0;
              
              return {
                next: () => {
                  if (done || index >= models.length) {
                    done = true;
                    return Promise.resolve({ done: true });
                  }
                  return Promise.resolve({
                    done: false,
                    value: models[index++],
                  });
                },
              };
            },
          };
        }),
      },
    })),
  };
});

describe("Model Utils with Ollama Integration", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    
    // Reset the modelsPromise cache
    // @ts-ignore - Accessing private variable for testing
    global.__TEST_RESET_MODELS_PROMISE?.();
  });

  afterEach(() => {
    vi.resetAllMocks();
  });

  describe("RECOMMENDED_MODELS", () => {
    it("should include both OpenAI and Ollama recommended models", () => {
      // Check that OpenAI models are included
      for (const model of RECOMMENDED_OPENAI_MODELS) {
        expect(RECOMMENDED_MODELS).toContain(model);
      }
      
      // Check that Ollama models are included with prefix
      for (const model of RECOMMENDED_OLLAMA_MODELS) {
        expect(RECOMMENDED_MODELS).toContain(`ollama:${model}`);
      }
    });
  });

  describe("getAvailableModels", () => {
    it("should fetch models from both OpenAI and Ollama when Ollama is enabled", async () => {
      // Mock Ollama models
      const mockOllamaModels = ["gemma3:1b", "llama3:8b"];
      (ollamaClient.fetchOllamaModels as any).mockResolvedValue(mockOllamaModels);
      
      // Set environment variables
      vi.stubEnv("OLLAMA_ENABLED", "true");
      vi.stubEnv("OPENAI_API_KEY", "test-key");
      
      const models = await getAvailableModels();
      
      // Check that OpenAI models are included
      expect(models).toContain("o4-mini");
      expect(models).toContain("o3");
      expect(models).toContain("gpt-4o");
      
      // Check that Ollama models are included with prefix
      expect(models).toContain("ollama:gemma3:1b");
      expect(models).toContain("ollama:llama3:8b");
      
      // Verify that fetchOllamaModels was called
      expect(ollamaClient.fetchOllamaModels).toHaveBeenCalled();
    });
    
    it("should not fetch Ollama models when Ollama is disabled", async () => {
      // Set environment variables
      vi.stubEnv("OLLAMA_ENABLED", "false");
      vi.stubEnv("OPENAI_API_KEY", "test-key");
      
      await getAvailableModels();
      
      // Verify that fetchOllamaModels was not called
      expect(ollamaClient.fetchOllamaModels).not.toHaveBeenCalled();
    });
  });

  describe("isModelSupportedForResponses", () => {
    it("should return true for Ollama models when Ollama is enabled", async () => {
      // Set environment variables
      vi.stubEnv("OLLAMA_ENABLED", "true");
      
      const result = await isModelSupportedForResponses("ollama:gemma3:1b");
      
      expect(result).toBe(true);
    });
    
    it("should return false for Ollama models when Ollama is disabled", async () => {
      // Set environment variables
      vi.stubEnv("OLLAMA_ENABLED", "false");
      
      const result = await isModelSupportedForResponses("ollama:gemma3:1b");
      
      expect(result).toBe(false);
    });
    
    it("should return true for OpenAI models regardless of Ollama status", async () => {
      // Set environment variables
      vi.stubEnv("OLLAMA_ENABLED", "false");
      
      const result = await isModelSupportedForResponses("o4-mini");
      
      expect(result).toBe(true);
    });
  });
});
