import { describe, it, expect, vi } from "vitest";

// Utility: fake OpenAI SDK that can be instructed to fail with 5xx a set
// number of times before succeeding.

function createStream(events: Array<any>) {
  return new (class {
    public controller = { abort: vi.fn() };
    async *[Symbol.asyncIterator]() {
      for (const ev of events) {
        yield ev;
      }
    }
  })();
}

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

vi.mock("../src/utils/agent/log.js", () => ({
  __esModule: true,
  log: () => {},
  isLoggingEnabled: () => false,
}));

import { AgentLoop } from "../src/utils/agent/agent-loop.js";

describe("AgentLoop – automatic retry on 5xx errors", () => {
  it("retries up to 3 times then succeeds", async () => {
    // Fail twice with 500 then succeed.
    let call = 0;
    openAiState.createSpy = vi.fn(async () => {
      call += 1;
      if (call <= 2) {
        const err: any = new Error("Internal Server Error");
        err.status = 500;
        throw err;
      }
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

    await new Promise((r) => setTimeout(r, 20));

    expect(openAiState.createSpy).toHaveBeenCalledTimes(3);

    const assistant = received.find((i) => i.role === "assistant");
    expect(assistant?.content?.[0]?.text).toBe("ok");
  });

  it("fails after a few attempts and surfaces system message", async () => {
    openAiState.createSpy = vi.fn(async () => {
      const err: any = new Error("Internal Server Error");
      err.status = 502; // any 5xx
      throw err;
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
        content: [{ type: "input_text", text: "hello" }],
      },
    ];

    await expect(agent.run(userMsg as any)).resolves.not.toThrow();

    await new Promise((r) => setTimeout(r, 20));

    expect(openAiState.createSpy).toHaveBeenCalledTimes(8);

    const sysMsg = received.find(
      (i) =>
        i.role === "system" &&
        typeof i.content?.[0]?.text === "string" &&
        i.content[0].text.includes("Network error"),
    );

    expect(sysMsg).toBeTruthy();
  });
});
