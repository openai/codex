import { describe, it, expect, vi } from "vitest";

// Stream that emits a function_call so the agent records a `lastResponseId`.
class StreamWithFunctionCall {
  public controller = { abort: vi.fn() };

  async *[Symbol.asyncIterator]() {
    // First, deliver the function call.
    yield {
      type: "response.output_item.done",
      item: {
        type: "function_call",
        id: "call123",
        name: "shell",
        arguments: JSON.stringify({ cmd: ["echo", "hi"] }),
      },
    } as any;

    // Then conclude the turn.
    yield {
      type: "response.completed",
      response: {
        id: "resp_func_call", // lastResponseId that would normally be stored
        status: "completed",
        output: [
          {
            type: "function_call",
            id: "call123",
            name: "shell",
            arguments: JSON.stringify({ cmd: ["echo", "hi"] }),
          },
        ],
      },
    } as any;
  }
}

vi.mock("openai", () => {
  const invocationBodies: Array<any> = [];
  let callNum = 0;
  class FakeOpenAI {
    public responses = {
      create: async (body: any) => {
        invocationBodies.push(body);
        callNum += 1;
        // First call streams a function_call, second call returns empty stream.
        if (callNum === 1) {
          return new StreamWithFunctionCall();
        }
        // Subsequent calls: empty stream.
        return new (class {
          public controller = { abort: vi.fn() };
          async *[Symbol.asyncIterator]() {
            /* no events */
          }
        })();
      },
    };
  }

  class APIConnectionTimeoutError extends Error {}

  return {
    __esModule: true,
    default: FakeOpenAI,
    APIConnectionTimeoutError,
    _test: {
      getBodies: () => invocationBodies,
    },
  };
});

// Stub helpers not relevant for this test.
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

// Now import the agent.
import { AgentLoop } from "../src/utils/agent/agent-loop.js";

describe("cancel clears previous_response_id", () => {
  it("second run after cancel should NOT include previous_response_id", async () => {
    const { _test } = (await import("openai")) as any;

    const agent = new AgentLoop({
      model: "any",
      instructions: "",
      approvalPolicy: { mode: "auto" } as any,
      additionalWritableRoots: [],
      onItem: () => {},
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
      config: { model: "any", instructions: "", notify: false },
    });

    // First run that triggers a function_call, but we will cancel *before* the
    // turn completes so the tool result is never returned.
    agent.run([
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "do something" }],
      },
    ] as any);
    // Give it a moment to receive the function_call.
    await new Promise((r) => setTimeout(r, 40));

    // Cancel (simulate ESC ESC).
    agent.cancel();

    // Second user input.
    await agent.run([
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "new command" }],
      },
    ] as any);

    const bodies = _test.getBodies();
    // eslint-disable-next-line no-console
    console.log(JSON.stringify(bodies, null, 2));
    expect(bodies.length).toBeGreaterThanOrEqual(2);

    // The *last* invocation belongs to the second run (after cancellation).
    const found = bodies.some(
      (b: any) =>
        Array.isArray(b.input) &&
        b.input.some(
          (i: any) =>
            i.type === "function_call_output" && i.call_id === "call123",
        ),
    );

    expect(found).toBe(true);
  });
});
