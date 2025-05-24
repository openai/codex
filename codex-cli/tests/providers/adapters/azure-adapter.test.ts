import { describe, test, expect, vi, beforeEach, afterEach } from "vitest";
import { AzureAdapter } from "../../../src/providers/adapters/azure-adapter.js";
import { AuthType, type ProviderConfig, type AuthProvider } from "../../../src/providers/types.js";
import { AzureOpenAI } from "openai";

// Save original env vars
const ORIGINAL_AZURE_BASE_URL = process.env["AZURE_BASE_URL"];
const ORIGINAL_AZURE_API_VERSION = process.env["AZURE_OPENAI_API_VERSION"];

// Mock OpenAI
vi.mock("openai", () => {
  class FakeAzureOpenAI {
    constructor(public config: any) {}
  }
  
  return {
    __esModule: true,
    AzureOpenAI: FakeAzureOpenAI,
  };
});

// Mock auth provider
class MockAuthProvider implements AuthProvider {
  async getAuthHeader(): Promise<string> {
    return "Bearer azure-test-key";
  }

  async validate(): Promise<void> {
    // No-op
  }
}

describe("AzureAdapter", () => {
  const azureConfig: ProviderConfig = {
    id: "azure",
    name: "AzureOpenAI",
    baseURL: "", // Azure requires this to be set via env var
    envKey: "AZURE_OPENAI_API_KEY",
    authType: AuthType.API_KEY,
  };

  beforeEach(() => {
    delete process.env["AZURE_BASE_URL"];
    delete process.env["AZURE_OPENAI_API_VERSION"];
  });

  afterEach(() => {
    if (ORIGINAL_AZURE_BASE_URL !== undefined) {
      process.env["AZURE_BASE_URL"] = ORIGINAL_AZURE_BASE_URL;
    }
    if (ORIGINAL_AZURE_API_VERSION !== undefined) {
      process.env["AZURE_OPENAI_API_VERSION"] = ORIGINAL_AZURE_API_VERSION;
    }
  });

  describe("validateConfiguration", () => {
    test("throws error when no base URL is configured", async () => {
      const authProvider = new MockAuthProvider();
      const adapter = new AzureAdapter(azureConfig, authProvider);
      
      await expect(adapter.createClient()).rejects.toThrow(
        "Azure OpenAI requires a base URL. Please set the AZURE_BASE_URL environment variable"
      );
    });

    test("succeeds when base URL is set via environment variable", async () => {
      process.env["AZURE_BASE_URL"] = "https://my-resource.openai.azure.com/openai";
      
      const authProvider = new MockAuthProvider();
      const adapter = new AzureAdapter(azureConfig, authProvider);
      
      // Should not throw
      const client = await adapter.createClient();
      expect(client).toBeInstanceOf(AzureOpenAI);
    });

    test("succeeds when base URL is in config", async () => {
      const configWithURL: ProviderConfig = {
        ...azureConfig,
        baseURL: "https://my-resource.openai.azure.com/openai",
      };
      
      const authProvider = new MockAuthProvider();
      const adapter = new AzureAdapter(configWithURL, authProvider);
      
      // Should not throw
      const client = await adapter.createClient();
      expect(client).toBeInstanceOf(AzureOpenAI);
    });
  });

  describe("createClient", () => {
    test("creates AzureOpenAI client with correct configuration", async () => {
      process.env["AZURE_BASE_URL"] = "https://test.openai.azure.com/openai";
      process.env["AZURE_OPENAI_API_VERSION"] = "2025-03-01-preview";
      
      const authProvider = new MockAuthProvider();
      const adapter = new AzureAdapter(azureConfig, authProvider);
      
      const client = await adapter.createClient();
      
      expect(client).toBeInstanceOf(AzureOpenAI);
      expect((client as any).config).toMatchObject({
        apiKey: "azure-test-key",
        baseURL: "https://test.openai.azure.com/openai",
        apiVersion: "2025-03-01-preview",
      });
    });

    test("uses default API version when not specified", async () => {
      process.env["AZURE_BASE_URL"] = "https://test.openai.azure.com/openai";
      // Don't set AZURE_OPENAI_API_VERSION
      
      const authProvider = new MockAuthProvider();
      const adapter = new AzureAdapter(azureConfig, authProvider);
      
      const client = await adapter.createClient();
      
      // Should use the default from config
      expect((client as any).config.apiVersion).toBe("2025-03-01-preview");
    });

    test("returns AzureOpenAI instance", async () => {
      process.env["AZURE_BASE_URL"] = "https://test.openai.azure.com/openai";
      
      const authProvider = new MockAuthProvider();
      const adapter = new AzureAdapter(azureConfig, authProvider);
      
      const client = await adapter.createClient();
      
      expect(client).toBeInstanceOf(AzureOpenAI);
    });
  });
});