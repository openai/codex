import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  fetchOllamaModels,
  formatOllamaModelName,
  extractOllamaModelName,
  isOllamaModel,
} from "../src/utils/ollama-client";

// Mock global fetch
const mockFetch = vi.fn();
global.fetch = mockFetch;

describe("Ollama Client", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.resetAllMocks();
  });

  describe("fetchOllamaModels", () => {
    it("should fetch models from Ollama API", async () => {
      // Mock successful response
      mockFetch.mockResolvedValueOnce({
        ok: true,
        json: async () => ({
          models: [
            { name: "gemma3:1b", modified_at: "2023-01-01", size: 1000000 },
            { name: "llama3:8b", modified_at: "2023-01-02", size: 8000000 },
          ],
        }),
      });

      const models = await fetchOllamaModels();
      
      expect(mockFetch).toHaveBeenCalledWith("http://localhost:11434/api/tags");
      expect(models).toEqual(["gemma3:1b", "llama3:8b"]);
    });

    it("should handle API errors gracefully", async () => {
      // Mock failed response
      mockFetch.mockResolvedValueOnce({
        ok: false,
        statusText: "Not Found",
      });

      const models = await fetchOllamaModels();
      
      expect(mockFetch).toHaveBeenCalledWith("http://localhost:11434/api/tags");
      expect(models).toEqual([]);
    });

    it("should handle network errors gracefully", async () => {
      // Mock network error
      mockFetch.mockRejectedValueOnce(new Error("Network error"));

      const models = await fetchOllamaModels();
      
      expect(mockFetch).toHaveBeenCalledWith("http://localhost:11434/api/tags");
      expect(models).toEqual([]);
    });
  });

  describe("formatOllamaModelName", () => {
    it("should format model name with ollama prefix", () => {
      expect(formatOllamaModelName("gemma3:1b")).toBe("ollama:gemma3:1b");
      expect(formatOllamaModelName("llama3:8b")).toBe("ollama:llama3:8b");
    });
  });

  describe("extractOllamaModelName", () => {
    it("should extract model name from prefixed name", () => {
      expect(extractOllamaModelName("ollama:gemma3:1b")).toBe("gemma3:1b");
      expect(extractOllamaModelName("ollama:llama3:8b")).toBe("llama3:8b");
    });

    it("should return null for non-Ollama models", () => {
      expect(extractOllamaModelName("gpt-4")).toBeNull();
      expect(extractOllamaModelName("o4-mini")).toBeNull();
    });
  });

  describe("isOllamaModel", () => {
    it("should identify Ollama models correctly", () => {
      expect(isOllamaModel("ollama:gemma3:1b")).toBe(true);
      expect(isOllamaModel("ollama:llama3:8b")).toBe(true);
      expect(isOllamaModel("gpt-4")).toBe(false);
      expect(isOllamaModel("o4-mini")).toBe(false);
    });
  });
});
