import { describe, test, expect, vi, beforeEach, afterEach } from "vitest";
import { BaseAdapter } from "../../../src/providers/adapters/base-adapter.js";
import {
  AuthType,
  type ProviderConfig,
  type AuthProvider,
} from "../../../src/providers/types.js";
import OpenAI from "openai";

// Save original env vars
const ORIGINAL_TEST_BASE_URL = process.env["TEST_BASE_URL"];

// Mock OpenAI
vi.mock("openai", () => {
  class FakeOpenAI {
    constructor(public config: any) {}
  }

  return {
    __esModule: true,
    default: FakeOpenAI,
  };
});

// Create a concrete implementation for testing
class TestAdapter extends BaseAdapter {
  // No additional implementation needed for basic tests
}

// Mock auth provider
class MockAuthProvider implements AuthProvider {
  constructor(
    private shouldFailValidation = false,
    private authHeader = "Bearer test-token",
    private additionalHeaders?: Record<string, string>,
  ) {}

  async getAuthHeader(): Promise<string> {
    return this.authHeader;
  }

  async validate(): Promise<void> {
    if (this.shouldFailValidation) {
      throw new Error("Auth validation failed");
    }
  }

  async getAdditionalHeaders(): Promise<Record<string, string>> {
    return this.additionalHeaders || {};
  }
}

describe("BaseAdapter", () => {
  const testConfig: ProviderConfig = {
    id: "test",
    name: "Test Provider",
    baseURL: "https://api.test.com/v1",
    envKey: "TEST_API_KEY",
    authType: AuthType.API_KEY,
  };

  beforeEach(() => {
    delete process.env["TEST_BASE_URL"];
  });

  afterEach(() => {
    if (ORIGINAL_TEST_BASE_URL !== undefined) {
      process.env["TEST_BASE_URL"] = ORIGINAL_TEST_BASE_URL;
    } else {
      delete process.env["TEST_BASE_URL"];
    }
  });

  describe("getBaseURL", () => {
    test("returns config baseURL by default", async () => {
      const authProvider = new MockAuthProvider();
      const adapter = new TestAdapter(testConfig, authProvider);

      const baseURL = await adapter.getBaseURL();
      expect(baseURL).toBe("https://api.test.com/v1");
    });

    test("prefers environment variable override", async () => {
      process.env["TEST_BASE_URL"] = "https://override.test.com/v2";

      const authProvider = new MockAuthProvider();
      const adapter = new TestAdapter(testConfig, authProvider);

      const baseURL = await adapter.getBaseURL();
      expect(baseURL).toBe("https://override.test.com/v2");
    });

    test("throws error when baseURL is empty and no env override", async () => {
      const configWithoutURL: ProviderConfig = {
        ...testConfig,
        baseURL: "",
      };

      const authProvider = new MockAuthProvider();
      const adapter = new TestAdapter(configWithoutURL, authProvider);

      await expect(adapter.getBaseURL()).rejects.toThrow(
        "No base URL configured for Test Provider",
      );
    });
  });

  describe("createClient", () => {
    test("creates OpenAI client with correct configuration", async () => {
      const authProvider = new MockAuthProvider();
      const adapter = new TestAdapter(testConfig, authProvider);

      const client = await adapter.createClient();

      expect(client).toBeInstanceOf(OpenAI);
      expect((client as any).config).toMatchObject({
        apiKey: "test-token", // "Bearer " prefix stripped
        baseURL: "https://api.test.com/v1",
      });
    });

    test("includes additional headers from auth provider", async () => {
      const additionalHeaders = {
        "X-Custom-Header": "custom-value",
        "X-Another-Header": "another-value",
      };

      const authProvider = new MockAuthProvider(
        false,
        "Bearer test-key",
        additionalHeaders,
      );
      const adapter = new TestAdapter(testConfig, authProvider);

      const client = await adapter.createClient();

      expect((client as any).config.defaultHeaders).toMatchObject(
        additionalHeaders,
      );
    });

    test("calls validateConfiguration before creating client", async () => {
      const authProvider = new MockAuthProvider();
      const adapter = new TestAdapter(testConfig, authProvider);

      // Spy on validateConfiguration
      const validateSpy = vi.spyOn(adapter as any, "validateConfiguration");

      await adapter.createClient();

      expect(validateSpy).toHaveBeenCalledTimes(1);
    });

    test("calls auth provider validate before creating client", async () => {
      const authProvider = new MockAuthProvider();
      const validateSpy = vi.spyOn(authProvider, "validate");

      const adapter = new TestAdapter(testConfig, authProvider);

      await adapter.createClient();

      expect(validateSpy).toHaveBeenCalledTimes(1);
    });

    test("throws error when auth validation fails", async () => {
      const authProvider = new MockAuthProvider(true); // Will fail validation
      const adapter = new TestAdapter(testConfig, authProvider);

      await expect(adapter.createClient()).rejects.toThrow(
        "Auth validation failed",
      );
    });

    test("strips Bearer prefix from auth header", async () => {
      const authProvider = new MockAuthProvider(false, "Bearer my-secret-key");
      const adapter = new TestAdapter(testConfig, authProvider);

      const client = await adapter.createClient();

      expect((client as any).config.apiKey).toBe("my-secret-key");
    });
  });

  describe("validateConfiguration", () => {
    test("default implementation does nothing", async () => {
      const authProvider = new MockAuthProvider();
      const adapter = new TestAdapter(testConfig, authProvider);

      // Should not throw
      await expect(
        (adapter as any).validateConfiguration(),
      ).resolves.toBeUndefined();
    });
  });
});
