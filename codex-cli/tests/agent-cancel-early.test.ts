import { describe, it, expect, vi } from "vitest";

// Fake stream that waits a bit before yielding the function_call so the test
// can cancel first.
class SlowFunctionCallStream {
  public controller = { abort: vi.fn() };

  async *[Symbol.asyncIterator]() {
    await new Promise((r) => setTimeout(r, 30));
    yield {
      type: "response.output_item.done",
      item: {
        type: "function_call",
        id: "slow_call",
        name: "shell",
        arguments: JSON.stringify({ cmd: ["echo", "hi"] }),
      },
    } as any;

    yield {
      type: "response.completed",
      response: {
        id: "resp_slow",
        status: "completed",
        output: [
          {
            type: "function_call",
            id: "slow_call",
            name: "shell",
            arguments: JSON.stringify({ cmd: ["echo", "hi"] }),
          },
        ],
      },
    } as any;
  }
}

vi.mock("openai", () => {
  const bodies: Array<any> = [];
  let callCount = 0;
  class FakeOpenAI {
    public responses = {
      create: async (body: any) => {
        bodies.push(body);
        callCount += 1;
        if (callCount === 1) {
          return new SlowFunctionCallStream();
        }
        return new (class {
          public controller = { abort: vi.fn() };
          async *[Symbol.asyncIterator]() {}
        })();
      },
    };
  }

  class APIConnectionTimeoutError extends Error {}

  return {
    __esModule: true,
    default: FakeOpenAI,
    APIConnectionTimeoutError,
    _test: { getBodies: () => bodies },
  };
});

vi.mock("../src/approvals.js", () => ({
  __esModule: true,
  alwaysApprovedCommands: new Set<string>(),
  canAutoApprove: () => ({ type: "auto-approve", runInSandbox: false }) as any,
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

describe("cancel before first function_call", () => {
  it("clears previous_response_id if no call ids captured", async () => {
    const { _test } = (await import("openai")) as any;

    const agent = new AgentLoop({
      additionalWritableRoots: [],
      model: "any",
      instructions: "",
      approvalPolicy: { mode: "auto" } as any,
      onItem: () => {},
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
      config: { model: "any", instructions: "", notify: false },
    });

    // Start first run.
    agent.run([
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "do" }],
      },
    ] as any);

    // Cancel quickly before any stream item.
    await new Promise((r) => setTimeout(r, 5));
    agent.cancel();

    // Second run.
    await agent.run([
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "new" }],
      },
    ] as any);

    const bodies = _test.getBodies();
    const last = bodies[bodies.length - 1];
    expect(last.previous_response_id).toBeUndefined();
  });
});
