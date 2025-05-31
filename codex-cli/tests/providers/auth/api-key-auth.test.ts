import { describe, test, expect } from "vitest";
import { ApiKeyAuthProvider } from "../../../src/providers/auth/api-key-auth.js";

describe("ApiKeyAuthProvider", () => {
  test("returns bearer token when API key is provided", async () => {
    const provider = new ApiKeyAuthProvider("test-api-key", "TestProvider");
    const authHeader = await provider.getAuthHeader();

    expect(authHeader).toBe("Bearer test-api-key");
  });

  test("throws error when API key is undefined", async () => {
    const provider = new ApiKeyAuthProvider(undefined, "TestProvider");

    await expect(provider.getAuthHeader()).rejects.toThrow(
      "No API key found for TestProvider",
    );
  });

  test("validates successfully with valid API key", async () => {
    const provider = new ApiKeyAuthProvider("test-api-key", "TestProvider");

    // Should not throw
    await expect(provider.validate()).resolves.toBeUndefined();
  });

  test("validation throws error when API key is undefined", async () => {
    const provider = new ApiKeyAuthProvider(undefined, "TestProvider");

    await expect(provider.validate()).rejects.toThrow(
      "No API key configured for TestProvider",
    );
  });

  test("validation throws error when API key is empty string", async () => {
    const provider = new ApiKeyAuthProvider("", "TestProvider");

    await expect(provider.validate()).rejects.toThrow(
      "No API key configured for TestProvider",
    );
  });

  test("validation throws error when API key is only whitespace", async () => {
    const provider = new ApiKeyAuthProvider("   ", "TestProvider");

    await expect(provider.validate()).rejects.toThrow(
      "No API key configured for TestProvider",
    );
  });
});
