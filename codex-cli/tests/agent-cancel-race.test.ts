import { describe, it, expect, vi } from "vitest";
// This test reproduces the real‑world issue where the user cancels the current
// task (Esc Esc) but the model's response has already started to stream — the
// partial answer still shows up in the UI.

// --- Mocks -----------------------------------------------------------------

// Shared state to track cancellation across stream instances
const state = {
  canceled: false,
  generation: 0
};

vi.mock("openai", () => {
  let callCount = 0;
  class FakeOpenAI {
    public responses = {
      create: async () => {
        callCount += 1;
        // Only the *first* stream yields "hello" so that any later answer
        // clearly comes from the canceled run.
        return callCount === 1
          ? new (class {
              public controller = { 
                abort: () => {
                  state.canceled = true;
                  state.generation++;
                }
              };
              
              async *[Symbol.asyncIterator]() {
                const thisGeneration = state.generation;
                
                // Only emit if not canceled and still in same generation
                if (!state.canceled && thisGeneration === state.generation) {
                  yield {
                    type: "response.output_item.done",
                    item: {
                      type: "message",
                      role: "assistant",
                      id: "m1",
                      content: [{ type: "text", text: "hello" }],
                    },
                  } as any;
                }
                
                // Only emit completion if not canceled and still in same generation
                if (!state.canceled && thisGeneration === state.generation) {
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
            })()
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
  class AzureOpenAI extends FakeOpenAI {}
  return { 
    __esModule: true, 
    default: FakeOpenAI, 
    AzureOpenAI,
    APIConnectionTimeoutError 
  };
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
