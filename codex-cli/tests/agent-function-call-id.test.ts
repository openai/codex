import { describe, it, expect, vi } from "vitest";
// ---------------------------------------------------------------------------
// This regression test ensures that the AgentLoop correctly copies the ID of a
// function tool‑call (be it `call_id` from the /responses endpoint *or* `id`
// from the /chat endpoint) into the subsequent `function_call_output` item. A
// missing or mismatched ID leads to the dreaded
//   400 | No tool output found for function call …
// error from the OpenAI API.
// ---------------------------------------------------------------------------

// Fake OpenAI stream that immediately yields a *chat‑style* function_call item.
class FakeStream {
  public controller = { abort: vi.fn() };

  async *[Symbol.asyncIterator]() {
    yield {
      type: "response.output_item.done",
      item: {
        // Chat endpoint style (id + nested function descriptor)
        type: "function_call",
        id: "call_test_123",
        function: {
          name: "shell",
          arguments: JSON.stringify({ cmd: ["echo", "hi"] }),
        },
      },
    } as any;

    yield {
      type: "response.completed",
      response: {
        id: "resp1",
        status: "completed",
        output: [
          {
            type: "function_call",
            id: "call_test_123",
            function: {
              name: "shell",
              arguments: JSON.stringify({ cmd: ["echo", "hi"] }),
            },
          },
        ],
      },
    } as any;
  }
}

// We intercept the OpenAI SDK so we can inspect the body of the second call –
// the one that is expected to contain our `function_call_output` item.
vi.mock("openai", () => {
  let invocation = 0;
  let capturedSecondBody: any;

  class FakeOpenAI {
    public responses = {
      create: async (body: any) => {
        invocation += 1;
        if (invocation === 1) {
          return new FakeStream();
        }
        if (invocation === 2) {
          capturedSecondBody = body;
          // empty stream
          return new (class {
            public controller = { abort: vi.fn() };
            async *[Symbol.asyncIterator]() {
              /* no items */
            }
          })();
        }
        throw new Error("Unexpected additional invocation in test");
      },
    };
  }

  class APIConnectionTimeoutError extends Error {}

  return {
    __esModule: true,
    default: FakeOpenAI,
    APIConnectionTimeoutError,
    // Re‑export so the test can access the captured body.
    _test: {
      getCapturedSecondBody: () => capturedSecondBody,
    },
  };
});

// Stub approvals & command formatting – not relevant for this test.
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

// Stub logger to keep the test output clean.
vi.mock("../src/utils/agent/log.js", () => ({
  __esModule: true,
  log: () => {},
  isLoggingEnabled: () => false,
}));

// Finally, import the module under test.
import { AgentLoop } from "../src/utils/agent/agent-loop.js";

describe("function_call_output includes original call ID", () => {
  it("copies id → call_id so the API accepts the tool result", async () => {
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
    });

    const userMsg = [
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "run" }],
      },
    ];

    await agent.run(userMsg as any);

    // Give the agent a tick to finish the second round‑trip.
    await new Promise((r) => setTimeout(r, 20));

    const body = _test.getCapturedSecondBody();
    expect(body).toBeTruthy();

    const outputItem = body.input?.find(
      (i: any) => i.type === "function_call_output",
    );
    expect(outputItem).toBeTruthy();
    expect(outputItem.call_id).toBe("call_test_123");
  });
});
