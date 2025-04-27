import { describe, it, expect, vi } from "vitest";
// This test reproduces the real‑world issue where the user cancels the current
// task (Esc Esc) but the model’s response has already started to stream — the
// partial answer still shows up in the UI.

// --- Mocks -----------------------------------------------------------------

class FakeStream {
  public controller = { abort: vi.fn() };

  async *[Symbol.asyncIterator]() {
    // Introduce a delay to simulate network latency and allow for cancel() to be called
    await new Promise((resolve) => setTimeout(resolve, 10));

    // Mimic an assistant message containing the word "hello".
    // Our fix should prevent this from being emitted after cancel() is called
    yield {
      type: "response.output_item.done",
      item: {
        type: "message",
        role: "assistant",
        id: "m1",
        content: [{ type: "text", text: "hello" }],
      },
    } as any;

    yield {
      type: "response.completed",
      response: {
        id: "resp1",
        status: "completed",
        output: [
          {
            type: "message",
            role: "assistant",
            id: "m1",
            content: [{ type: "text", text: "hello" }],
          },
        ],
      },
    } as any;
  }
}

vi.mock("openai", () => {
  let callCount = 0;
  class FakeOpenAI {
    public responses = {
      create: async () => {
        callCount += 1;
        // Only the *first* stream yields "hello" so that any later answer
        // clearly comes from the canceled run.
        return callCount === 1
          ? new FakeStream()
          : new (class {
              public controller = { abort: vi.fn() };
              async *[Symbol.asyncIterator]() {
                // empty stream
              }
            })();
      },
    };
  }
  class APIConnectionTimeoutError extends Error {}
  return { __esModule: true, default: FakeOpenAI, APIConnectionTimeoutError };
});

// Stubs for external helpers referenced indirectly.
vi.mock("../src/approvals.js", () => ({
  __esModule: true,
  isSafeCommand: () => null,
}));
vi.mock("../src/format-command.js", () => ({
  __esModule: true,
  formatCommandForDisplay: (c: Array<string>) => c.join(" "),
}));

// Stub the logger to avoid file‑system side effects during tests.
import { AgentLoop } from "../src/utils/agent/agent-loop.js";

vi.mock("../src/utils/agent/log.js", () => ({
  __esModule: true,
  log: () => {},
  isLoggingEnabled: () => false,
}));

describe("Agent cancellation race", () => {
  // This test verifies our fix for the race condition where a cancelled message
  // could still appear after the user cancels a request.
  it("should not emit messages after cancel() is called", async () => {
    const items: Array<any> = [];

    const agent = new AgentLoop({
      additionalWritableRoots: [],
      model: "any",
      instructions: "",
      config: { model: "any", instructions: "", notify: false },
      approvalPolicy: { mode: "auto" } as any,
      onItem: (i) => items.push(i),
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });

    const input = [
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "say hello" }],
      },
    ];

    agent.run(input as any);

    // Cancel after the stream has started.
    await new Promise((r) => setTimeout(r, 5));
    agent.cancel();

    // Immediately issue a new (empty) command to mimic the UI letting the user
    // type something else – this resets the agent state.
    agent.run([
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "noop" }],
      },
    ] as any);

    // Give everything time to flush.
    await new Promise((r) => setTimeout(r, 40));

    const assistantMsg = items.find((i) => i.role === "assistant");
    // Our fix should prevent the assistant message from being delivered after cancel
    // Now that we've fixed it, the test should pass
    expect(assistantMsg).toBeUndefined();
  });
});
