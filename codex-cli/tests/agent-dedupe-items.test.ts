import { describe, it, expect, vi } from "vitest";

// ---------------------------------------------------------------------------
// This regression test ensures that AgentLoop only surfaces each response item
// once even when the same item appears multiple times in the OpenAI streaming
// response (e.g. as an early `response.output_item.done` event *and* again in
// the final `response.completed` payload).
// ---------------------------------------------------------------------------

// Fake OpenAI stream that emits the *same* message twice: first as an
// incremental output event and then again in the turn completion payload.
class FakeStream {
  public controller = { abort: vi.fn() };

  async *[Symbol.asyncIterator]() {
    // 1) Early incremental item.
    yield {
      type: "response.output_item.done",
      item: {
        type: "message",
        id: "call-dedupe-1",
        role: "assistant",
        content: [{ type: "input_text", text: "Hello!" }],
      },
    } as any;

    // 2) Turn completion containing the *same* item again.
    yield {
      type: "response.completed",
      response: {
        id: "resp-dedupe-1",
        status: "completed",
        output: [
          {
            type: "message",
            id: "call-dedupe-1",
            role: "assistant",
            content: [{ type: "input_text", text: "Hello!" }],
          },
        ],
      },
    } as any;
  }
}

// Intercept the OpenAI SDK used inside AgentLoop so we can inject our fake
// streaming implementation.
vi.mock("openai", () => {
  class FakeOpenAI {
    public responses = {
      create: async () => new FakeStream(),
    };
  }

  class APIConnectionTimeoutError extends Error {}

  return { __esModule: true, default: FakeOpenAI, APIConnectionTimeoutError };
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
  formatCommandForDisplay: (cmd: Array<string>) => cmd.join(" "),
}));

vi.mock("../src/utils/agent/log.js", () => ({
  __esModule: true,
  log: () => {},
  isLoggingEnabled: () => false,
}));

// After the dependency mocks we can import the module under test.
import { AgentLoop } from "../src/utils/agent/agent-loop.js";

describe("AgentLoop deduplicates output items", () => {
  it("invokes onItem exactly once for duplicate items with the same id", async () => {
    const received: Array<any> = [];

    const agent = new AgentLoop({
      model: "any",
      instructions: "",
      config: { model: "any", instructions: "", notify: false },
      approvalPolicy: { mode: "auto" } as any,
      additionalWritableRoots: [],
      onItem: (item) => received.push(item),
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

    // Give the setTimeout(3ms) inside AgentLoop.stageItem a chance to fire.
    await new Promise((r) => setTimeout(r, 20));

    // Count how many times the duplicate item surfaced.
    const appearances = received.filter((i) => i.id === "call-dedupe-1").length;
    expect(appearances).toBe(1);
  });
});
