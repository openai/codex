/**
 * tests/azure-foundry-agent-loop.test.ts
 *
 * Unit tests for Azure Foundry integration in AgentLoop.
 */

import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";

// Fake stream that yields a completed response event
class FakeStream {
  async *[Symbol.asyncIterator]() {
    yield {
      type: "response.completed",
      response: { id: "azure_foundry_resp", status: "completed", output: [] },
    } as any;
  }
}

let _lastCreateParams: any = null;
let lastAzureClientConfig: any = null;

vi.mock("openai", () => {  class FakeDefaultClient {
    public responses = {
      create: async (params: any) => {
        _lastCreateParams = params;
        return new FakeStream();
      },
    };
  }
    class FakeAzureClient {
    constructor(config: any) {
      lastAzureClientConfig = config;
    }
      public responses = {
      create: async (params: any) => {
        _lastCreateParams = params;
        return new FakeStream();
      },
    };
  }
  
  class APIConnectionTimeoutError extends Error {}
  
  return {
    __esModule: true,
    default: FakeDefaultClient,
    AzureOpenAI: FakeAzureClient,
    APIConnectionTimeoutError,
  };
});

// Stub approvals to bypass command approval logic
vi.mock("../src/approvals.js", () => ({
  __esModule: true,
  alwaysApprovedCommands: new Set<string>(),
  canAutoApprove: () => ({ type: "auto-approve", runInSandbox: false }),
  isSafeCommand: () => null,
}));

// Stub format-command to avoid formatting side effects
vi.mock("../src/format-command.js", () => ({
  __esModule: true,
  formatCommandForDisplay: (cmd: Array<string>) => cmd.join(" "),
}));

// Stub internal logging to keep output clean
vi.mock("../src/utils/agent/log.js", () => ({
  __esModule: true,
  log: () => {},
  isLoggingEnabled: () => false,
}));

describe("AgentLoop Azure Foundry Integration", () => {
  // Store original environment variables
  const originalEnv = {
    AZURE_API_KEY: process.env["AZURE_API_KEY"],
    AZURE_ENDPOINT: process.env["AZURE_ENDPOINT"],
    AZURE_ADDITIONAL_HEADERS: process.env["AZURE_ADDITIONAL_HEADERS"],
  };  beforeEach(() => {
    _lastCreateParams = null;
    lastAzureClientConfig = null;
    
    // Clear Azure environment variables before each test
    delete process.env["AZURE_API_KEY"];
    delete process.env["AZURE_ENDPOINT"];
    delete process.env["AZURE_ADDITIONAL_HEADERS"];
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

  it("creates AzureOpenAI client with basic configuration", async () => {
    process.env["AZURE_API_KEY"] = "test-azure-key";
    process.env["AZURE_ENDPOINT"] = "https://my-resource.openai.azure.com/openai/deployments/my-model/chat/completions?api-version=2024-02-15-preview";
    
    // Reset the capture variables
    _lastCreateParams = null;
    lastAzureClientConfig = null;
    
    const { AgentLoop } = await import("../src/utils/agent/agent-loop.js");
    
    const cfg: any = {
      model: "gpt-4",
      provider: "azure",
      instructions: "",
      disableResponseStorage: false,
      notify: false,
      apiKey: "test-azure-key",
    };
    
    // Create the AgentLoop instance - this should trigger AzureOpenAI creation
    new AgentLoop({
      additionalWritableRoots: [],
      model: cfg.model,
      provider: "azure", // Explicitly pass the provider
      config: cfg,
      instructions: cfg.instructions,
      approvalPolicy: { mode: "suggest" } as any,
      onItem: () => {},
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });

    // Verify that the AzureOpenAI constructor was called
    expect(lastAzureClientConfig).not.toBeNull();
    expect(lastAzureClientConfig.apiKey).toBe("test-azure-key");
    expect(lastAzureClientConfig.baseURL).toBe("https://my-resource.openai.azure.com/openai/deployments/my-model/chat/completions?api-version=2024-02-15-preview");
  });

  it("includes additional headers in AzureOpenAI client", async () => {
    process.env["AZURE_API_KEY"] = "test-azure-key";
    process.env["AZURE_ENDPOINT"] = "https://my-resource.openai.azure.com/openai/deployments/my-model/chat/completions?api-version=2024-02-15-preview";
    process.env["AZURE_ADDITIONAL_HEADERS"] = JSON.stringify({
      "model": "gpt-4",
      "deployment-name": "my-deployment",
      "Custom-Header": "custom-value"    });
    
    // Reset the capture variables
    _lastCreateParams = null;
    lastAzureClientConfig = null;
    
    const { AgentLoop } = await import("../src/utils/agent/agent-loop.js");
    
    const cfg: any = {
      model: "gpt-4",
      provider: "azure",
      instructions: "",
      disableResponseStorage: false,
      notify: false,
      apiKey: "test-azure-key",
    };
    
    new AgentLoop({
      additionalWritableRoots: [],
      model: cfg.model,
      provider: "azure", // Explicitly pass the provider
      config: cfg,
      instructions: cfg.instructions,
      approvalPolicy: { mode: "suggest" } as any,
      onItem: () => {},
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });    expect(lastAzureClientConfig.defaultHeaders).toEqual({
      originator: "codex_cli_ts",
      version: expect.any(String),
      session_id: expect.any(String),
      "model": "gpt-4",
      "deployment-name": "my-deployment",
      "Custom-Header": "custom-value",
    });
  });

  it("handles invalid additional headers gracefully", async () => {
    process.env["AZURE_API_KEY"] = "test-azure-key";
    process.env["AZURE_ENDPOINT"] = "https://my-resource.openai.azure.com/openai/deployments/my-model/chat/completions?api-version=2024-02-15-preview";
    process.env["AZURE_ADDITIONAL_HEADERS"] = "invalid-json";
    
    // Reset the capture variables
    _lastCreateParams = null;
    lastAzureClientConfig = null;
    
    const { AgentLoop } = await import("../src/utils/agent/agent-loop.js");
    
    const cfg: any = {
      model: "gpt-4",
      provider: "azure",
      instructions: "",
      disableResponseStorage: false,
      notify: false,
      apiKey: "test-azure-key",
    };
    
    // Should not throw an error
    expect(() => new AgentLoop({
      additionalWritableRoots: [],
      model: cfg.model,
      provider: "azure", // Explicitly pass the provider
      config: cfg,
      instructions: cfg.instructions,
      approvalPolicy: { mode: "suggest" } as any,
      onItem: () => {},
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    })).not.toThrow();    // Should only include standard headers, no additional headers due to invalid JSON
    expect(lastAzureClientConfig.defaultHeaders).toEqual({
      originator: "codex_cli_ts",
      version: expect.any(String),
      session_id: expect.any(String),
    });  });
  it("creates AzureOpenAI client when provider is azure", async () => {
    process.env["AZURE_API_KEY"] = "test-azure-key";
    process.env["AZURE_ENDPOINT"] = "https://my-resource.openai.azure.com/openai/deployments/my-model/chat/completions?api-version=2024-02-15-preview";
    
    // Reset the capture variables
    _lastCreateParams = null;
    lastAzureClientConfig = null;
    
    const { AgentLoop } = await import("../src/utils/agent/agent-loop.js");
    
    const cfg: any = {
      model: "gpt-4",
      provider: "azure",
      instructions: "",
      disableResponseStorage: false,
      notify: false,
      apiKey: "test-azure-key",
    };
    
    new AgentLoop({
      additionalWritableRoots: [],
      model: cfg.model,
      provider: "azure", // Explicitly pass the provider
      config: cfg,
      instructions: cfg.instructions,
      approvalPolicy: { mode: "suggest" } as any,
      onItem: () => {},
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });

    // Verify that the AzureOpenAI constructor was called
    expect(lastAzureClientConfig).not.toBeNull();
    expect(lastAzureClientConfig.apiKey).toBe("test-azure-key");
    expect(lastAzureClientConfig.baseURL).toBe("https://my-resource.openai.azure.com/openai/deployments/my-model/chat/completions?api-version=2024-02-15-preview");
  });
});
