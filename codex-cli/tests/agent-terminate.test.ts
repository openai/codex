import { describe, it, expect, vi } from "vitest";

// --- OpenAI stream mock ----------------------------------------------------

class FakeStream {
  public controller = { abort: vi.fn() };

  async *[Symbol.asyncIterator]() {
    // Immediately ask for a shell function call so we can test that the
    // subsequent function_call_output never gets surfaced after terminate().
    yield {
      type: "response.output_item.done",
      item: {
        type: "function_call",
        id: "call‑terminate‑1",
        name: "shell",
        arguments: JSON.stringify({ cmd: ["sleep", "5"] }),
      },
    } as any;

    // Turn completion echoing the same function call.
    yield {
      type: "response.completed",
      response: {
        id: "resp‑terminate‑1",
        status: "completed",
        output: [
          {
            type: "function_call",
            id: "call‑terminate‑1",
            name: "shell",
            arguments: JSON.stringify({ cmd: ["sleep", "5"] }),
          },
        ],
      },
    } as any;
  }
}

vi.mock("openai", () => {
  class FakeOpenAI {
    public responses = {
      create: async () => new FakeStream(),
    };
  }
  class APIConnectionTimeoutError extends Error {}
  return { __esModule: true, default: FakeOpenAI, APIConnectionTimeoutError };
});

// --- Helpers referenced by handle‑exec‑command -----------------------------

vi.mock("../src/approvals.js", () => {
  return {
    __esModule: true,
    alwaysApprovedCommands: new Set<string>(),
    canAutoApprove: () =>
      ({ type: "auto-approve", runInSandbox: false }) as any,
    isSafeCommand: () => null,
  };
});

vi.mock("../src/format-command.js", () => {
  return {
    __esModule: true,
    formatCommandForDisplay: (cmd: Array<string>) => cmd.join(" "),
  };
});

// Stub logger to avoid filesystem side‑effects
vi.mock("../src/utils/agent/log.js", () => ({
  __esModule: true,
  log: () => {},
  isLoggingEnabled: () => false,
}));

// After dependency mocks we can import the modules under test.

import { AgentLoop } from "../src/utils/agent/agent-loop.js";
import * as handleExec from "../src/utils/agent/handle-exec-command.js";

describe("Agent terminate (hard cancel)", () => {
  it("suppresses function_call_output and stops processing once terminate() is invoked", async () => {
    // Simulate a long‑running exec that would normally resolve with output.
    vi.spyOn(handleExec, "handleExecCommand").mockImplementation(
      async (
        _args,
        _config,
        _policy,
        _additionalWritableRoots,
        _getConf,
        abortSignal,
      ) => {
        // Wait until the abort signal is fired or 2s (whichever comes first).
        await new Promise<void>((resolve) => {
          if (abortSignal?.aborted) {
            return resolve();
          }
          const timer = setTimeout(resolve, 2000);
          abortSignal?.addEventListener("abort", () => {
            clearTimeout(timer);
            resolve();
          });
        });

        return { outputText: "should‑not‑happen", metadata: {} } as any;
      },
    );

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
        content: [{ type: "input_text", text: "run long cmd" }],
      },
    ];

    // Start agent loop but don't wait for completion.
    agent.run(userMsg as any);

    // Give it a brief moment to start and process the function_call.
    await new Promise((r) => setTimeout(r, 10));

    agent.terminate();

    // Allow promises to settle.
    await new Promise((r) => setTimeout(r, 50));

    const hasOutput = received.some((i) => i.type === "function_call_output");
    expect(hasOutput).toBe(false);
  });

  it("rejects further run() calls after terminate()", async () => {
    const agent = new AgentLoop({
      model: "any",
      instructions: "",
      config: { model: "any", instructions: "", notify: false },
      approvalPolicy: { mode: "auto" } as any,
      additionalWritableRoots: [],
      onItem: () => {},
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });

    agent.terminate();

    const dummyMsg = [
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "noop" }],
      },
    ];

    let threw = false;
    try {
      // We expect this to fail fast – either by throwing synchronously or by
      // returning a rejected promise.
      await agent.run(dummyMsg as any);
    } catch {
      threw = true;
    }

    expect(threw).toBe(true);
  });
});
