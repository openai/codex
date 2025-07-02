import { describe, it, expect, beforeEach, afterEach } from "vitest";
import {
  parseCustomHeadersFromEnv,
  getCustomHeaders,
  type ProviderInfo,
} from "../src/utils/providers.js";

describe("Custom Headers", () => {
  let originalEnvVars: Record<string, string | undefined> = {};

  beforeEach(() => {
    // Store original environment variables
    const envVarNames = [
      "OPENAI_CUSTOM_HEADERS",
      "AZURE_CUSTOM_HEADERS",
      "TEST_CUSTOM_HEADERS",
    ];
    for (const envVar of envVarNames) {
      originalEnvVars[envVar] = process.env[envVar];
      delete process.env[envVar];
    }
  });

  afterEach(() => {
    // Restore original environment variables
    for (const [envVar, value] of Object.entries(originalEnvVars)) {
      if (value !== undefined) {
        process.env[envVar] = value;
      } else {
        delete process.env[envVar];
      }
    }
    originalEnvVars = {};
  });

  describe("parseCustomHeadersFromEnv", () => {
    it("returns empty object when provider-specific env var is not set", () => {
      const headers = parseCustomHeadersFromEnv("openai");
      expect(headers).toEqual({});
    });

    it("parses single header correctly", () => {
      process.env["OPENAI_CUSTOM_HEADERS"] = "X-Test-Header: test-value";
      const headers = parseCustomHeadersFromEnv("openai");
      expect(headers).toEqual({
        "X-Test-Header": "test-value",
      });
    });

    it("parses multiple headers correctly", () => {
      process.env["AZURE_CUSTOM_HEADERS"] =
        "X-Custom-Auth: Bearer token123\nX-App-Version: 2.0.0\nX-Extra: some value";
      const headers = parseCustomHeadersFromEnv("azure");
      expect(headers).toEqual({
        "X-Custom-Auth": "Bearer token123",
        "X-App-Version": "2.0.0",
        "X-Extra": "some value",
      });
    });

    it("handles headers with extra whitespace", () => {
      process.env["TEST_CUSTOM_HEADERS"] =
        "  X-Header1  :  value1  \n  X-Header2:value2\n";
      const headers = parseCustomHeadersFromEnv("test");
      expect(headers).toEqual({
        "X-Header1": "value1",
        "X-Header2": "value2",
      });
    });

    it("ignores empty lines", () => {
      process.env["TEST_CUSTOM_HEADERS"] =
        "X-Header1: value1\n\n\nX-Header2: value2\n";
      const headers = parseCustomHeadersFromEnv("test");
      expect(headers).toEqual({
        "X-Header1": "value1",
        "X-Header2": "value2",
      });
    });

    it("ignores lines without colons", () => {
      process.env["TEST_CUSTOM_HEADERS"] =
        "X-Header1: value1\ninvalid-line-without-colon\nX-Header2: value2";
      const headers = parseCustomHeadersFromEnv("test");
      expect(headers).toEqual({
        "X-Header1": "value1",
        "X-Header2": "value2",
      });
    });

    it("ignores lines with empty keys", () => {
      process.env["TEST_CUSTOM_HEADERS"] =
        "X-Header1: value1\n: empty-key\nX-Header2: value2";
      const headers = parseCustomHeadersFromEnv("test");
      expect(headers).toEqual({
        "X-Header1": "value1",
        "X-Header2": "value2",
      });
    });

    it("handles values with colons", () => {
      process.env["TEST_CUSTOM_HEADERS"] =
        "X-Auth: Bearer token:with:colons\nX-URL: https://example.com:8080";
      const headers = parseCustomHeadersFromEnv("test");
      expect(headers).toEqual({
        "X-Auth": "Bearer token:with:colons",
        "X-URL": "https://example.com:8080",
      });
    });
  });

  describe("getCustomHeaders", () => {
    it("returns empty object when provider has no custom headers and no env var", () => {
      const provider: ProviderInfo = {
        name: "Test Provider",
        baseURL: "https://api.example.com",
        envKey: "TEST_KEY",
      };
      const headers = getCustomHeaders(provider, "test");
      expect(headers).toEqual({});
    });

    it("returns provider headers when no env var is set", () => {
      const provider: ProviderInfo = {
        name: "Test Provider",
        baseURL: "https://api.example.com",
        envKey: "TEST_KEY",
        customHeaders: {
          "X-Provider-Header": "provider-value",
          "X-Provider-Version": "1.0.0",
        },
      };
      const headers = getCustomHeaders(provider, "test");
      expect(headers).toEqual({
        "X-Provider-Header": "provider-value",
        "X-Provider-Version": "1.0.0",
      });
    });

    it("returns env headers when provider has no custom headers", () => {
      process.env["OPENAI_CUSTOM_HEADERS"] =
        "X-Env-Header: env-value\nX-Env-Version: 2.0.0";
      const provider: ProviderInfo = {
        name: "OpenAI",
        baseURL: "https://api.openai.com/v1",
        envKey: "OPENAI_API_KEY",
      };
      const headers = getCustomHeaders(provider, "openai");
      expect(headers).toEqual({
        "X-Env-Header": "env-value",
        "X-Env-Version": "2.0.0",
      });
    });

    it("merges provider and env headers with env taking precedence", () => {
      process.env["AZURE_CUSTOM_HEADERS"] =
        "X-Shared-Header: env-value\nX-Env-Only: env-only";
      const provider: ProviderInfo = {
        name: "AzureOpenAI",
        baseURL: "https://test.openai.azure.com/openai",
        envKey: "AZURE_OPENAI_API_KEY",
        customHeaders: {
          "X-Shared-Header": "provider-value",
          "X-Provider-Only": "provider-only",
        },
      };
      const headers = getCustomHeaders(provider, "azure");
      expect(headers).toEqual({
        "X-Shared-Header": "env-value", // env takes precedence
        "X-Provider-Only": "provider-only",
        "X-Env-Only": "env-only",
      });
    });
  });
});
