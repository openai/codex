/**
 * tests/azure-foundry-client.test.ts
 *
 * Unit tests for Azure Foundry OpenAI client creation functionality.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

// Mock the openai module
const mockOpenAI = vi.fn();
const mockAzureOpenAI = vi.fn();

vi.mock("openai", () => ({
  __esModule: true,
  default: mockOpenAI,
  AzureOpenAI: mockAzureOpenAI,
}));

describe("Azure Foundry OpenAI Client", () => {
  // Store original environment variables
  const originalEnv = {
    AZURE_API_KEY: process.env["AZURE_API_KEY"],
    AZURE_ENDPOINT: process.env["AZURE_ENDPOINT"],
    AZURE_ADDITIONAL_HEADERS: process.env["AZURE_ADDITIONAL_HEADERS"],
    OPENAI_ORGANIZATION: process.env["OPENAI_ORGANIZATION"],
    OPENAI_PROJECT: process.env["OPENAI_PROJECT"],
  };

  beforeEach(() => {
    // Clear environment variables before each test
    delete process.env["AZURE_API_KEY"];
    delete process.env["AZURE_ENDPOINT"];
    delete process.env["AZURE_ADDITIONAL_HEADERS"];
    delete process.env["OPENAI_ORGANIZATION"];
    delete process.env["OPENAI_PROJECT"];
    
    // Reset mocks
    vi.clearAllMocks();
    vi.resetModules();
  });

  afterEach(() => {
    // Restore original environment variables
    Object.entries(originalEnv).forEach(([key, value]) => {
      if (value !== undefined) {
        process.env[key] = value;
      } else {
        delete process.env[key];
      }
    });
  });

  describe("createOpenAIClient", () => {
    it("creates AzureOpenAI client for azure provider", async () => {
      process.env["AZURE_API_KEY"] = "test-key";
      process.env["AZURE_ENDPOINT"] = "https://test.openai.azure.com/openai";
      
      const { createOpenAIClient } = await import("../src/utils/openai-client.js");
      
      createOpenAIClient({ provider: "azure" });
      
      expect(mockAzureOpenAI).toHaveBeenCalledWith({
        apiKey: "test-key",
        baseURL: "https://test.openai.azure.com/openai",
        apiVersion: "2025-04-01-preview",
        timeout: undefined,
        defaultHeaders: {},
      });
      expect(mockOpenAI).not.toHaveBeenCalled();
    });

    it("includes additional headers for azure provider", async () => {
      process.env["AZURE_API_KEY"] = "test-key";
      process.env["AZURE_ENDPOINT"] = "https://test.openai.azure.com/openai";
      process.env["AZURE_ADDITIONAL_HEADERS"] = JSON.stringify({
        "model": "gpt-4",
        "deployment-name": "my-deployment"
      });
      
      const { createOpenAIClient } = await import("../src/utils/openai-client.js");
      
      createOpenAIClient({ provider: "azure" });
      
      expect(mockAzureOpenAI).toHaveBeenCalledWith({
        apiKey: "test-key",
        baseURL: "https://test.openai.azure.com/openai",
        apiVersion: "2025-04-01-preview",
        timeout: undefined,
        defaultHeaders: {
          "model": "gpt-4",
          "deployment-name": "my-deployment"
        },
      });
    });

    it("includes OpenAI organization and project headers when set", async () => {
      process.env["AZURE_API_KEY"] = "test-key";
      process.env["AZURE_ENDPOINT"] = "https://test.openai.azure.com/openai";
      process.env["OPENAI_ORGANIZATION"] = "org-123";
      process.env["OPENAI_PROJECT"] = "proj-456";
      
      const { createOpenAIClient } = await import("../src/utils/openai-client.js");
      
      createOpenAIClient({ provider: "azure" });
      
      expect(mockAzureOpenAI).toHaveBeenCalledWith({
        apiKey: "test-key",
        baseURL: "https://test.openai.azure.com/openai",
        apiVersion: "2025-04-01-preview",
        timeout: undefined,
        defaultHeaders: {
          "OpenAI-Organization": "org-123",
          "OpenAI-Project": "proj-456"
        },
      });
    });

    it("combines all headers for azure provider", async () => {
      process.env["AZURE_API_KEY"] = "test-key";
      process.env["AZURE_ENDPOINT"] = "https://test.openai.azure.com/openai";
      process.env["AZURE_ADDITIONAL_HEADERS"] = JSON.stringify({
        "model": "gpt-4",
        "Custom-Header": "custom-value"
      });
      process.env["OPENAI_ORGANIZATION"] = "org-123";
      process.env["OPENAI_PROJECT"] = "proj-456";
      
      const { createOpenAIClient } = await import("../src/utils/openai-client.js");
      
      createOpenAIClient({ provider: "azure" });
      
      expect(mockAzureOpenAI).toHaveBeenCalledWith({
        apiKey: "test-key",
        baseURL: "https://test.openai.azure.com/openai",
        apiVersion: "2025-04-01-preview",
        timeout: undefined,
        defaultHeaders: {
          "OpenAI-Organization": "org-123",
          "OpenAI-Project": "proj-456",
          "model": "gpt-4",
          "Custom-Header": "custom-value"
        },
      });
    });

    it("creates regular OpenAI client for non-azure provider", async () => {
      process.env["OPENAI_API_KEY"] = "test-openai-key";
      
      const { createOpenAIClient } = await import("../src/utils/openai-client.js");
      
      createOpenAIClient({ provider: "openai" });
      
      expect(mockOpenAI).toHaveBeenCalledWith({
        apiKey: "test-openai-key",
        baseURL: "https://api.openai.com/v1",
        timeout: undefined,
        defaultHeaders: {},
      });
      expect(mockAzureOpenAI).not.toHaveBeenCalled();
    });

    it("handles invalid additional headers gracefully", async () => {
      process.env["AZURE_API_KEY"] = "test-key";
      process.env["AZURE_ENDPOINT"] = "https://test.openai.azure.com/openai";
      process.env["AZURE_ADDITIONAL_HEADERS"] = "invalid-json";
      
      const { createOpenAIClient } = await import("../src/utils/openai-client.js");
      
      // Should not throw an error
      expect(() => createOpenAIClient({ provider: "azure" })).not.toThrow();
      
      expect(mockAzureOpenAI).toHaveBeenCalledWith({
        apiKey: "test-key",
        baseURL: "https://test.openai.azure.com/openai",
        apiVersion: "2025-04-01-preview",
        timeout: undefined,
        defaultHeaders: {}, // Invalid JSON should result in empty additional headers
      });
    });
  });
});
