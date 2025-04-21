import { describe, it, expect, beforeEach, afterEach } from "vitest";

// We import the module *lazily* inside each test so that we can control the
// environment variables independently per test case. Node's module cache
// would otherwise capture the value present during the first import.

const ORIGINAL_ENV_OPENAI_KEY = process.env["OPENAI_API_KEY"];

beforeEach(() => {
  delete process.env["OPENAI_API_KEY"];
});

afterEach(() => {
  if (ORIGINAL_ENV_OPENAI_KEY !== undefined) {
    process.env["OPENAI_API_KEY"] = ORIGINAL_ENV_OPENAI_KEY;
  } else {
    delete process.env["OPENAI_API_KEY"];
  }
});

describe("provider config", () => {
  it("loads provider API keys from environment", async () => {
    process.env["OPENAI_API_KEY"] = "test-openai-key";
    
    const { loadConfig } = await import("../src/utils/config.js");
    const config = loadConfig();
    
    expect(config.providers.openai.apiKey).toBe("test-openai-key");
  });
  
  it("allows setting provider API keys at runtime", async () => {
    const { setProviderApiKey, providerApiKeys } = await import(
      "../src/utils/config.js"
    );
    
    setProviderApiKey("openai", "my-openai-key");
    expect(providerApiKeys.openai).toBe("my-openai-key");
    
    setProviderApiKey("claude", "my-claude-key");
    expect(providerApiKeys.claude).toBe("my-claude-key");
  });
});
