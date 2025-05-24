import { describe, test, expect, vi, beforeEach, afterEach } from "vitest";
import { createProviderAdapter } from "../../src/providers/registry.js";
import { providerConfigs } from "../../src/providers/configs.js";
import { AuthType } from "../../src/providers/types.js";

// Save original env vars
const ORIGINAL_OPENAI_KEY = process.env["OPENAI_API_KEY"];
const ORIGINAL_VERTEX_PROJECT = process.env["VERTEX_PROJECT_ID"];
const ORIGINAL_GOOGLE_PROJECT = process.env["GOOGLE_CLOUD_PROJECT"];

// Mock state holders
const googleAuthState: {
  getClientSpy?: ReturnType<typeof vi.fn>;
  getProjectIdSpy?: ReturnType<typeof vi.fn>;
  getAccessTokenSpy?: ReturnType<typeof vi.fn>;
} = {};

// Mock google-auth-library
vi.mock("google-auth-library", () => {
  class FakeGoogleAuth {
    async getClient() {
      return (
        googleAuthState.getClientSpy?.() ?? {
          getAccessToken:
            googleAuthState.getAccessTokenSpy ||
            (() => ({ token: "fake-token" })),
        }
      );
    }

    async getProjectId() {
      return googleAuthState.getProjectIdSpy?.() ?? "test-project";
    }
  }

  return {
    GoogleAuth: FakeGoogleAuth,
  };
});

describe("Provider Registry", () => {
  beforeEach(() => {
    // Reset env vars
    delete process.env["OPENAI_API_KEY"];
    delete process.env["VERTEX_PROJECT_ID"];
    delete process.env["GOOGLE_CLOUD_PROJECT"];

    // Reset mock state
    googleAuthState.getClientSpy = undefined;
    googleAuthState.getProjectIdSpy = undefined;
    googleAuthState.getAccessTokenSpy = undefined;
  });

  afterEach(() => {
    // Restore env vars
    if (ORIGINAL_OPENAI_KEY !== undefined) {
      process.env["OPENAI_API_KEY"] = ORIGINAL_OPENAI_KEY;
    }
    if (ORIGINAL_VERTEX_PROJECT !== undefined) {
      process.env["VERTEX_PROJECT_ID"] = ORIGINAL_VERTEX_PROJECT;
    }
    if (ORIGINAL_GOOGLE_PROJECT !== undefined) {
      process.env["GOOGLE_CLOUD_PROJECT"] = ORIGINAL_GOOGLE_PROJECT;
    }
  });

  describe("providerConfigs", () => {
    test("all providers have required fields", () => {
      Object.entries(providerConfigs).forEach(([id, config]) => {
        expect(config.id).toBe(id);
        expect(config.name).toBeTruthy();
        expect(config.baseURL).toBeDefined(); // Can be empty string
        expect(config.envKey).toBeTruthy();
        expect(config.authType).toBeDefined();
        expect(Object.values(AuthType)).toContain(config.authType);
      });
    });

    test("vertex provider uses OAuth auth type", () => {
      expect(providerConfigs["vertex"]?.authType).toBe(AuthType.OAUTH);
    });

    test("ollama provider uses no auth", () => {
      expect(providerConfigs["ollama"]?.authType).toBe(AuthType.NONE);
    });
  });

  describe("createProviderAdapter", () => {
    test("throws error for unknown provider", async () => {
      await expect(createProviderAdapter("unknown-provider")).rejects.toThrow(
        "Unknown provider: unknown-provider",
      );
    });

    test("creates adapter for OpenAI provider", async () => {
      process.env["OPENAI_API_KEY"] = "test-key";
      const adapter = await createProviderAdapter("openai");

      expect(adapter).toBeDefined();
      expect(adapter.config.id).toBe("openai");
      expect(adapter.config.name).toBe("OpenAI");
    });

    test("creates adapter for Azure provider", async () => {
      process.env["AZURE_OPENAI_API_KEY"] = "test-key";
      process.env["AZURE_BASE_URL"] = "https://test.openai.azure.com/openai";

      const adapter = await createProviderAdapter("azure");

      expect(adapter).toBeDefined();
      expect(adapter.config.id).toBe("azure");
      expect(adapter.config.name).toBe("AzureOpenAI");
    });

    test("creates adapter for Vertex provider", async () => {
      process.env["VERTEX_PROJECT_ID"] = "test-project";

      googleAuthState.getClientSpy = vi.fn().mockResolvedValue({
        getAccessToken: () => ({ token: "test-token" }),
      });

      const adapter = await createProviderAdapter("vertex");

      expect(adapter).toBeDefined();
      expect(adapter.config.id).toBe("vertex");
      expect(adapter.config.name).toBe("Vertex AI");
      expect(adapter.config.authType).toBe(AuthType.OAUTH);
    });

    test("creates adapter for Ollama provider", async () => {
      const adapter = await createProviderAdapter("ollama");

      expect(adapter).toBeDefined();
      expect(adapter.config.id).toBe("ollama");
      expect(adapter.config.authType).toBe(AuthType.NONE);
    });

    test("provider IDs are case insensitive", async () => {
      process.env["OPENAI_API_KEY"] = "test-key";

      const adapter1 = await createProviderAdapter("OpenAI");
      const adapter2 = await createProviderAdapter("openai");
      const adapter3 = await createProviderAdapter("OPENAI");

      expect(adapter1.config.id).toBe("openai");
      expect(adapter2.config.id).toBe("openai");
      expect(adapter3.config.id).toBe("openai");
    });
  });
});
