import { describe, it, expect, vi } from "vitest";
// Mock the OpenAI SDK used inside AgentLoop so we can control streaming events.
class FakeStream {
  public controller = { abort: vi.fn() };

  async *[Symbol.asyncIterator]() {
    // Immediately yield a function_call item.
    yield {
      type: "response.output_item.done",
      item: {
        type: "function_call",
        id: "call1",
        name: "shell",
        arguments: JSON.stringify({ cmd: ["node", "-e", "console.log('hi')"] }),
      },
    } as any;

    // Indicate turn completion with the same function_call.
    yield {
      type: "response.completed",
      response: {
        id: "resp1",
        status: "completed",
        output: [
          {
            type: "function_call",
            id: "call1",
            name: "shell",
            arguments: JSON.stringify({
              cmd: ["node", "-e", "console.log('hi')"],
            }),
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

// Mock the approvals and formatCommand helpers referenced by handle‑exec‑command.
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

// Stub the logger to avoid file‑system side effects during tests.
vi.mock("../src/utils/agent/log.js", () => ({
  __esModule: true,
  log: () => {},
  isLoggingEnabled: () => false,
}));

// After mocking dependencies we can import the modules under test.
import { AgentLoop } from "../src/utils/agent/agent-loop.js";
import * as handleExec from "../src/utils/agent/handle-exec-command.js";

describe("Agent cancellation", () => {
  it("does not emit function_call_output after cancel", async () => {
    // Mock handleExecCommand to simulate a slow shell command that would write
    // "hello" if allowed to finish.
    vi.spyOn(handleExec, "handleExecCommand").mockImplementation(async () => {
      await new Promise((r) => setTimeout(r, 50));
      return { outputText: "hello", metadata: {} } as any;
    });

    const received: Array<any> = [];

    const agent = new AgentLoop({
      model: "any",
      instructions: "",
      config: { model: "any", instructions: "", notify: false },
      approvalPolicy: { mode: "auto" } as any,
      additionalWritableRoots: [],
      onItem: (item) => {
        received.push(item);
      },
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });

    const userMsg = [
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "say hi" }],
      },
    ];

    // Start the agent loop but don't await it – we'll cancel while it's running.
    agent.run(userMsg as any);

    // Give the agent a moment to start processing.
    await new Promise((r) => setTimeout(r, 10));

    // Cancel the task.
    agent.cancel();

    // Wait a little longer to allow any pending promises to settle.
    await new Promise((r) => setTimeout(r, 100));

    // Ensure no function_call_output items were emitted after cancellation.
    const hasOutput = received.some((i) => i.type === "function_call_output");
    expect(hasOutput).toBe(false);
  });

  it("still suppresses output when cancellation happens after a fast exec", async () => {
    vi.restoreAllMocks();

    // Quick exec mock (returns immediately).
    vi.spyOn(handleExec, "handleExecCommand").mockResolvedValue({
      outputText: "hello-fast",
      metadata: {},
    } as any);

    const received: Array<any> = [];

    const agent = new AgentLoop({
      additionalWritableRoots: [],
      model: "any",
      instructions: "",
      config: { model: "any", instructions: "", notify: false },
      approvalPolicy: { mode: "auto" } as any,
      onItem: (item) => received.push(item),
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });

    const userMsg = [
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "say hi" }],
      },
    ];

    agent.run(userMsg as any);

    // Wait a bit so the exec has certainly finished and output is ready.
    await new Promise((r) => setTimeout(r, 20));

    agent.cancel();

    await new Promise((r) => setTimeout(r, 50));

    const hasOutput = received.some((i) => i.type === "function_call_output");
    expect(hasOutput).toBe(false);
  });

  it("cancels and prevents execution of pending commands after initial command", async () => {
    vi.restoreAllMocks();

    // Mock to track command execution calls
    const execSpy = vi.spyOn(handleExec, "handleExecCommand").mockImplementation(async () => {
      await new Promise((r) => setTimeout(r, 15));
      return { outputText: "command executed", metadata: {} } as any;
    });

    const received: Array<any> = [];

    // Create a custom FakeStream that yields multiple function_call items
    class MultiCommandFakeStream extends FakeStream {
      // 添加 override 修饰符解决 TS4114 错误
      override async *[Symbol.asyncIterator]() {
        // First command
        yield {
          type: "response.output_item.done",
          item: {
            type: "function_call",
            id: "call1",
            name: "shell",
            arguments: JSON.stringify({ cmd: ["echo", "first command"] }),
          },
        } as any;

        // Small delay to simulate processing
        await new Promise((r) => setTimeout(r, 5));

        // Second command that should not run after cancellation
        yield {
          type: "response.output_item.done",
          item: {
            type: "function_call",
            id: "call2",
            name: "shell",
            arguments: JSON.stringify({ cmd: ["echo", "second command"] }),
          },
        } as any;

        // Complete the response
        yield {
          type: "response.completed",
          response: {
            id: "resp1",
            status: "completed",
            output: [
              {
                type: "function_call",
                id: "call1",
                name: "shell",
                arguments: JSON.stringify({ cmd: ["echo", "first command"] }),
              },
              {
                type: "function_call",
                id: "call2",
                name: "shell",
                arguments: JSON.stringify({ cmd: ["echo", "second command"] }),
              },
            ],
          },
        } as any;
      }
    }

    // 修复 TS2339 错误，重新设置 mock
    // 替代原来的: vi.mocked(vi.importActual("openai")).default.prototype.responses.create
    const OpenAIMock = vi.hoisted(() => {
      return {
        default: class {
          public responses = {
            create: async () => new FakeStream()
          }
        }
      };
    });

    // 手动覆盖模拟函数
    OpenAIMock.default.prototype.responses.create = async () => new MultiCommandFakeStream();
    vi.mock("openai", () => OpenAIMock);

    const agent = new AgentLoop({
      model: "any",
      instructions: "",
      config: { model: "any", instructions: "", notify: false },
      approvalPolicy: { mode: "auto" } as any,
      additionalWritableRoots: [],
      onItem: (item) => {
        received.push(item);
      },
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });

    const userMsg = [
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "run multiple commands" }],
      },
    ];

    // Start the agent loop
    agent.run(userMsg as any);

    // Give the agent time to process the first command
    await new Promise((r) => setTimeout(r, 10));

    // Cancel the agent after the first command starts but before it completes
    agent.cancel();

    // Wait for all operations to settle
    await new Promise((r) => setTimeout(r, 100));

    // Check that handleExecCommand was called only once (for the first command)
    // and the second command was never executed
    expect(execSpy).toHaveBeenCalledTimes(1);

    // Check that no output was emitted for either command
    const hasOutput = received.some((i) => i.type === "function_call_output");
    expect(hasOutput).toBe(false);
  });
});