import { mkdtempSync, rmSync, writeFileSync, mkdirSync } from "fs";
import { tmpdir } from "os";
import { join } from "path";
import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";

// ---------------------------------------------------------------------------
// Test helpers & mocks
// ---------------------------------------------------------------------------

// Fake stream returned from the mocked OpenAI SDK. The AgentLoop only cares
// that the stream is async‑iterable and eventually yields a `response.completed`
// event so the turn can finish.
class FakeStream {
  public controller = { abort: vi.fn() };

  async *[Symbol.asyncIterator]() {
    yield {
      type: "response.completed",
      response: {
        id: "r1",
        status: "completed",
        output: [],
      },
    } as any;
  }
}

// Capture the parameters that AgentLoop sends to `openai.responses.create()` so
// we can assert on the `instructions` value.
let lastCreateParams: any = null;

vi.mock("openai", () => {
  class FakeOpenAI {
    public responses = {
      create: async (params: any) => {
        lastCreateParams = params;
        return new FakeStream();
      },
    };
  }

  class APIConnectionTimeoutError extends Error {}

  return {
    __esModule: true,
    default: FakeOpenAI,
    APIConnectionTimeoutError,
  };
});

// The AgentLoop pulls these helpers in order to decide whether a command can
// be auto‑approved. None of that matters for this test, so we stub the module
// with minimal no-op implementations.
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

// Stub the file‑based logger to avoid side effects and keep the test output
// clean.
vi.mock("../src/utils/agent/log.js", () => ({
  __esModule: true,
  log: () => {},
  isLoggingEnabled: () => false,
}));

// ---------------------------------------------------------------------------
// After mocks are in place we can import the modules under test.
// ---------------------------------------------------------------------------

import { AgentLoop } from "../src/utils/agent/agent-loop.js";
import { loadConfig } from "../src/utils/config.js";

// ---------------------------------------------------------------------------

let projectDir: string;

beforeEach(() => {
  // Create a fresh temporary directory to act as an isolated git repo.
  projectDir = mkdtempSync(join(tmpdir(), "codex-proj-"));
  mkdirSync(join(projectDir, ".git")); // mark as project root

  // Write a small project doc that we expect to be included in the prompt.
  writeFileSync(join(projectDir, "codex.md"), "# Test Project\nHello docs!\n");

  lastCreateParams = null; // reset captured SDK params
});

afterEach(() => {
  rmSync(projectDir, { recursive: true, force: true });
});

describe("AgentLoop", () => {
  it("passes codex.md contents through the instructions parameter", async () => {
    const config = loadConfig(undefined, undefined, { cwd: projectDir });

    // Sanity‑check that loadConfig picked up the project doc. This is *not* the
    // main assertion – we just avoid a false‑positive if the fixture setup is
    // incorrect.
    expect(config.instructions).toContain("Hello docs!");

    const agent = new AgentLoop({
      additionalWritableRoots: [],
      model: "o3", // arbitrary
      instructions: config.instructions,
      config,
      approvalPolicy: { mode: "suggest" } as any,
      onItem: () => {},
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });

    // Kick off a single run and wait for it to finish. The fake OpenAI client
    // will resolve immediately.
    await agent.run([
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "ping" }],
      },
    ]);

    // Ensure the AgentLoop called the SDK and that the instructions we see at
    // that point still include the project doc. This validates the full path:
    // loadConfig → AgentLoop → addInstructionPrefix → OpenAI SDK.
    expect(lastCreateParams).not.toBeNull();
    expect(lastCreateParams.instructions).toContain("Hello docs!");
  });
});
