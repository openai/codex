import { describe, it, expect, vi } from "vitest";

// ---------------------------------------------------------------------------
// Mock helpers
// ---------------------------------------------------------------------------

// Keep reference so test cases can programmatically change behaviour of the
// fake OpenAI client.
const openAiState: { createSpy?: ReturnType<typeof vi.fn> } = {};

/**
 * Mock the "openai" package so we can simulate rate‑limit errors without
 * making real network calls. The AgentLoop only relies on `responses.create`
 * so we expose a minimal stub.
 */
vi.mock("openai", () => {
  class FakeOpenAI {
    public responses = {
      // Will be replaced per‑test via `openAiState.createSpy`.
      create: (...args: Array<any>) => openAiState.createSpy!(...args),
    };
  }

  // The real SDK exports this constructor – include it for typings even
  // though it is not used in this spec.
  class APIConnectionTimeoutError extends Error {}

  return {
    __esModule: true,
    default: FakeOpenAI,
    APIConnectionTimeoutError,
  };
});

// Stub helpers that the agent indirectly imports so it does not attempt any
// file‑system access or real approvals logic during the test.
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

// Silence agent‑loop debug logging so test output stays clean.
vi.mock("../src/utils/agent/log.js", () => ({
  __esModule: true,
  log: () => {},
  isLoggingEnabled: () => false,
}));

import { AgentLoop } from "../src/utils/agent/agent-loop.js";

describe("AgentLoop – rate‑limit handling", () => {
  it("retries up to the maximum and then surfaces a system message", async () => {
    // Enable fake timers for this test only – we restore real timers at the end
    // so other tests are unaffected.
    vi.useFakeTimers();

    try {
      // Construct a dummy rate‑limit error that matches the implementation's
      // detection logic (`status === 429`).
      const rateLimitErr: any = new Error("Rate limit exceeded");
      rateLimitErr.status = 429;

      // Always throw the rate‑limit error to force the loop to exhaust all
      // retries (5 attempts in total).
      openAiState.createSpy = vi.fn(async () => {
        throw rateLimitErr;
      });

      const received: Array<any> = [];

      const agent = new AgentLoop({
        model: "any",
        instructions: "",
        approvalPolicy: { mode: "auto" } as any,
        additionalWritableRoots: [],
        onItem: (i) => received.push(i),
        onLoading: () => {},
        getCommandConfirmation: async () => ({ review: "yes" }) as any,
        onLastResponseId: () => {},
      });

      const userMsg = [
        {
          type: "message",
          role: "user",
          content: [{ type: "input_text", text: "hello" }],
        },
      ];

      // Start the run but don't await yet so we can advance fake timers while it
      // is in progress.
      const runPromise = agent.run(userMsg as any);

      // Should be done in at most 180 seconds.
      await vi.advanceTimersByTimeAsync(180_000);

      // Ensure the promise settles without throwing.
      await expect(runPromise).resolves.not.toThrow();

      // Flush the 10 ms staging delay used when emitting items.
      await vi.advanceTimersByTimeAsync(20);

      // The OpenAI client should have been called the maximum number of retry
      // attempts (8).
      expect(openAiState.createSpy).toHaveBeenCalledTimes(8);

      // Finally, verify that the user sees a helpful system message.
      const sysMsg = received.find(
        (i) =>
          i.role === "system" &&
          typeof i.content?.[0]?.text === "string" &&
          i.content[0].text.includes("Rate limit reached"),
      );

      expect(sysMsg).toBeTruthy();
    } finally {
      // Ensure global timer state is restored for subsequent tests.
      vi.useRealTimers();
    }
  });
});
