import { describe, it, expect, vi } from "vitest";
// ---------------------------------------------------------------------------
//  Utility: fake OpenAI SDK with programmable behaviour per test case.
// ---------------------------------------------------------------------------

// A minimal helper to build predetermined streams.
function createStream(events: Array<any>, opts: { throwAfter?: Error } = {}) {
  return new (class {
    public controller = { abort: vi.fn() };

    async *[Symbol.asyncIterator]() {
      for (const ev of events) {
        yield ev;
      }
      if (opts.throwAfter) {
        throw opts.throwAfter;
      }
    }
  })();
}

// Holders so tests can access spies/state injected by the mock.
const openAiState: {
  createSpy?: ReturnType<typeof vi.fn>;
} = {};

vi.mock("openai", () => {
  class APIConnectionTimeoutError extends Error {}

  class FakeOpenAI {
    public responses = {
      // `createSpy` will be swapped out per test.
      create: (...args: Array<any>) => openAiState.createSpy!(...args),
    };
  }

  return {
    __esModule: true,
    default: FakeOpenAI,
    APIConnectionTimeoutError,
  };
});

// Stub approvals / formatting helpers – not relevant here.
vi.mock("../src/approvals.js", () => ({
  __esModule: true,
  alwaysApprovedCommands: new Set<string>(),
  canAutoApprove: () => ({ type: "auto-approve", runInSandbox: false }) as any,
  isSafeCommand: () => null,
}));

vi.mock("../src/format-command.js", () => ({
  __esModule: true,
  formatCommandForDisplay: (c: Array<string>) => c.join(" "),
}));

// Silence debug logging from agent‑loop.
vi.mock("../src/utils/agent/log.js", () => ({
  __esModule: true,
  log: () => {},
  isLoggingEnabled: () => false,
}));

import { AgentLoop } from "../src/utils/agent/agent-loop.js";

describe("AgentLoop – network resilience", () => {
  it("retries once on APIConnectionTimeoutError and succeeds", async () => {
    // Arrange fake OpenAI: first call throws APIConnectionTimeoutError, second returns a short stream.
    const { APIConnectionTimeoutError } = await import("openai");

    let call = 0;
    openAiState.createSpy = vi.fn(async () => {
      call += 1;
      if (call === 1) {
        throw new APIConnectionTimeoutError({ message: "timeout" });
      }
      // Second attempt – minimal assistant reply.
      return createStream([
        {
          type: "response.output_item.done",
          item: {
            type: "message",
            role: "assistant",
            id: "m1",
            content: [{ type: "text", text: "ok" }],
          },
        },
        {
          type: "response.completed",
          response: {
            id: "r1",
            status: "completed",
            output: [
              {
                type: "message",
                role: "assistant",
                id: "m1",
                content: [{ type: "text", text: "ok" }],
              },
            ],
          },
        },
      ]);
    });

    const received: Array<any> = [];

    const agent = new AgentLoop({
      model: "any",
      instructions: "",
      approvalPolicy: { mode: "auto" } as any,
      additionalWritableRoots: [],
      onItem: (i) => received.push(i),
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });

    const userMsg = [
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "hi" }],
      },
    ];

    await agent.run(userMsg as any);

    // Wait a tick for flush.
    await new Promise((r) => setTimeout(r, 20));

    expect(openAiState.createSpy).toHaveBeenCalledTimes(2);

    const assistant = received.find((i) => i.role === "assistant");
    expect(assistant).toBeTruthy();
    expect(assistant.content?.[0]?.text).toBe("ok");
  });

  it("shows system message when connection closes prematurely", async () => {
    const prematureError = new Error("Premature close");
    // @ts-ignore add code prop
    prematureError.code = "ERR_STREAM_PREMATURE_CLOSE";

    openAiState.createSpy = vi.fn(async () => {
      return createStream([], { throwAfter: prematureError });
    });

    const received: Array<any> = [];

    const agent = new AgentLoop({
      model: "any",
      instructions: "",
      approvalPolicy: { mode: "auto" } as any,
      additionalWritableRoots: [],
      onItem: (i) => received.push(i),
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });

    const userMsg = [
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "hi" }],
      },
    ];

    await agent.run(userMsg as any);

    // Wait a tick.
    await new Promise((r) => setTimeout(r, 20));

    const sysMsg = received.find(
      (i) =>
        i.role === "system" &&
        i.content?.[0]?.text?.includes("Connection closed prematurely"),
    );
    expect(sysMsg).toBeTruthy();
  });
});
