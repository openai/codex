import { describe, it, expect, vi } from "vitest";
// This test reproduces the real‑world issue where the user cancels the current
// task (Esc Esc) but the model’s response has already started to stream — the
// partial answer still shows up in the UI.

// --- Mocks -----------------------------------------------------------------

class FakeStream {
  public controller = { abort: vi.fn() };

  async *[Symbol.asyncIterator]() {
    // Immediately start streaming an assistant message so that it is possible
    // for a user‑triggered cancellation that happens milliseconds later to
    // arrive *after* the first token has already been emitted. This mirrors
    // the real‑world race where the UI shows nothing yet (network / rendering
    // latency) even though the model has technically started responding.
    // Mimic an assistant message containing the word "hello".
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
  // We expect this test to highlight the current bug, so the suite should
  // fail (red) until the underlying race condition in `AgentLoop` is fixed.
  it("still emits the model answer even though cancel() was called", async () => {
    const items: Array<any> = [];

    const agent = new AgentLoop({
      additionalWritableRoots: [],
      model: "any",
      instructions: "",
      config: { model: "any", instructions: "", notify: false },
      approvalPolicy: { mode: "auto" } as any,
      onItem: (i) => items.push(i),
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" } as any),
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
    // The bug manifests if the assistant message is still present even though
    // it belongs to the canceled run. We assert that it *should not* be
    // delivered – this test will fail until the bug is fixed.
    expect(assistantMsg).toBeUndefined();
  });
});
