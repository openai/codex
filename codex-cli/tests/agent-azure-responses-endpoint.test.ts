/**
 * tests/agent-azure-responses-endpoint.test.ts
 *
 * Verifies that AgentLoop calls the `/responses` endpoint when provider is set to Azure.
 */

import { describe, it, expect, vi, beforeEach } from "vitest";

// Fake stream that yields a completed response event
class FakeStream {
  async *[Symbol.asyncIterator]() {
    yield {
      type: "response.completed",
      response: { id: "azure_resp", status: "completed", output: [] },
    } as any;
  }
}

let lastCreateParams: any = null;

vi.mock("openai", () => {
  class FakeDefaultClient {
    public responses = {
      create: async (params: any) => {
        lastCreateParams = params;
        return new FakeStream();
      },
    };
  }
  class FakeAzureClient {
    public responses = {
      create: async (params: any) => {
        lastCreateParams = params;
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

import { AgentLoop } from "../src/utils/agent/agent-loop.js";

describe("AgentLoop Azure provider responses endpoint", () => {
  beforeEach(() => {
    lastCreateParams = null;
  });

  it("calls the /responses endpoint when provider is azure", async () => {
    const cfg: any = {
      model: "test-model",
      provider: "azure",
      instructions: "",
      disableResponseStorage: false,
      notify: false,
    };
    const loop = new AgentLoop({
      additionalWritableRoots: [],
      model: cfg.model,
      config: cfg,
      instructions: cfg.instructions,
      approvalPolicy: { mode: "suggest" } as any,
      onItem: () => {},
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });

    await loop.run([
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "hello" }],
      },
    ]);

    expect(lastCreateParams).not.toBeNull();
    expect(lastCreateParams.model).toBe(cfg.model);
    expect(Array.isArray(lastCreateParams.input)).toBe(true);
  });
});
