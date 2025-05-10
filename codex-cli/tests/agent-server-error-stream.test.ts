import { describe, it, expect, vi } from "vitest";

// ---------------------------------------------------------------------------
// Utility helpers & OpenAI mock – tailored for server‑side errors that occur
// *after* the streaming iterator was created (i.e. during iteration).
// ---------------------------------------------------------------------------

function createStreamThatErrors(err: Error) {
  return new (class {
    public controller = { abort: vi.fn() };

    async *[Symbol.asyncIterator]() {
      // Immediately raise the error once iteration starts – mimics OpenAI SDK
      // behaviour which throws from the iterator when the HTTP response status
      // indicates an internal server failure.
      throw err;
    }
  })();
}

// Spy holder swapped out per test case.
const openAiState: { createSpy?: ReturnType<typeof vi.fn> } = {};

vi.mock("openai", () => {
  class FakeOpenAI {
    public responses = {
      create: (...args: Array<any>) => openAiState.createSpy!(...args),
    };
  }

  class APIConnectionTimeoutError extends Error {}

  return {
    __esModule: true,
    default: FakeOpenAI,
    APIConnectionTimeoutError,
  };
});

// Approvals / formatting stubs – not part of the behaviour under test.
vi.mock("../src/approvals.js", () => ({
  __esModule: true,
  alwaysApprovedCommands: new Set<string>(),
  canAutoApprove: () => ({ type: "auto-approve", runInSandbox: false } as any),
  isSafeCommand: () => null,
}));

vi.mock("../src/format-command.js", () => ({
  __esModule: true,
  formatCommandForDisplay: (c: Array<string>) => c.join(" "),
}));

// Silence debug logging so the test output stays uncluttered.
vi.mock("../src/utils/agent/log.js", () => ({
  __esModule: true,
  log: () => {},
  isLoggingEnabled: () => false,
}));

import { AgentLoop } from "../src/utils/agent/agent-loop.js";

describe("AgentLoop – server_error surfaced during streaming", () => {
  it("shows user‑friendly system message instead of crashing", async () => {
    const apiErr: any = new Error(
      "The server had an error while processing your request. Sorry about that!",
    );
    // Replicate the structure used by the OpenAI SDK for 5xx failures.
    apiErr.type = "server_error";
    apiErr.code = null;
    apiErr.status = undefined; // SDK leaves status undefined in this pathway

    openAiState.createSpy = vi.fn(async () => {
      return createStreamThatErrors(apiErr);
    });

    const received: Array<any> = [];

    const agent = new AgentLoop({
      model: "any",
      instructions: "",
      approvalPolicy: { mode: "auto" } as any,
      additionalWritableRoots: [],
      onItem: (i) => received.push(i),
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" } as any),
      onLastResponseId: () => {},
    });

    const userMsg = [
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "ping" }],
      },
    ];

    await expect(agent.run(userMsg as any)).resolves.not.toThrow();

    // allow async onItem deliveries to flush
    await new Promise((r) => setTimeout(r, 20));

    const sysMsg = received.find(
      (i) =>
        i.role === "system" &&
        typeof i.content?.[0]?.text === "string" &&
        i.content[0].text.includes("Network error"),
    );

    expect(sysMsg).toBeTruthy();
  });
});
