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
  canAutoApprove: () => ({ type: "auto-approve", runInSandbox: false } as any),
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
import {
  DEFAULT_RATE_LIMIT_MAX_RETRIES,
  DEFAULT_RATE_LIMIT_INITIAL_RETRY_DELAY_MS,
  DEFAULT_RATE_LIMIT_MAX_RETRY_DELAY_MS,
  DEFAULT_RATE_LIMIT_JITTER_FACTOR,
} from "../src/utils/config.js";

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
        config: {
          model: "any",
          instructions: "",
          rateLimits: {
            maxRetries: DEFAULT_RATE_LIMIT_MAX_RETRIES,
            initialRetryDelayMs: DEFAULT_RATE_LIMIT_INITIAL_RETRY_DELAY_MS,
            maxRetryDelayMs: DEFAULT_RATE_LIMIT_MAX_RETRY_DELAY_MS,
            jitterFactor: DEFAULT_RATE_LIMIT_JITTER_FACTOR,
          },
        },
        additionalWritableRoots: [],
        onItem: (i) => received.push(i),
        onLoading: () => {},
        getCommandConfirmation: async () => ({ review: "yes" } as any),
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

      // The agent uses exponential backoff with jitter for retries.
      // With default settings, the maximum total wait time would be approximately:
      // 2500 + 5000 + 10000 + 20000 = 37500ms (without considering jitter)
      // We add a safety margin to account for jitter and other delays.
      await vi.advanceTimersByTimeAsync(60_000); // Generous time to cover all retries

      // Ensure the promise settles without throwing.
      await expect(runPromise).resolves.not.toThrow();

      // Flush the 10 ms staging delay used when emitting items.
      await vi.advanceTimersByTimeAsync(20);

      // The OpenAI client should have been called the maximum number of retry
      // attempts (5).
      expect(openAiState.createSpy).toHaveBeenCalledTimes(5);

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

  it("respects custom rate limit configuration", async () => {
    // Enable fake timers for this test
    vi.useFakeTimers();

    try {
      // Construct a dummy rate‑limit error
      const rateLimitErr: any = new Error("Rate limit exceeded");
      rateLimitErr.status = 429;

      // Always throw the rate‑limit error
      openAiState.createSpy = vi.fn(async () => {
        throw rateLimitErr;
      });

      const received: Array<any> = [];

      // Create an agent with custom rate limit settings
      const customMaxRetries = 3; // Fewer retries than default
      const customInitialDelay = 1000; // Shorter initial delay
      const customMaxDelay = 10000; // Shorter max delay
      const customJitter = 0.1; // Less jitter

      const agent = new AgentLoop({
        model: "any",
        instructions: "",
        approvalPolicy: { mode: "auto" } as any,
        config: {
          model: "any",
          instructions: "",
          rateLimits: {
            maxRetries: customMaxRetries,
            initialRetryDelayMs: customInitialDelay,
            maxRetryDelayMs: customMaxDelay,
            jitterFactor: customJitter,
          },
        },
        onItem: (i) => received.push(i),
        onLoading: () => {},
        getCommandConfirmation: async () => ({ review: "yes" } as any),
        onLastResponseId: () => {},
      });

      const userMsg = [
        {
          type: "message",
          role: "user",
          content: [{ type: "input_text", text: "hello" }],
        },
      ];

      // Start the run
      const runPromise = agent.run(userMsg as any);

      // With custom settings, the maximum total wait time would be approximately:
      // 1000 + 2000 + 4000 = 7000ms (without considering jitter)
      await vi.advanceTimersByTimeAsync(15_000); // Generous time to cover all retries

      // Ensure the promise settles without throwing
      await expect(runPromise).resolves.not.toThrow();

      // Flush the staging delay
      await vi.advanceTimersByTimeAsync(20);

      // The OpenAI client should have been called the custom maximum number of retry attempts
      expect(openAiState.createSpy).toHaveBeenCalledTimes(customMaxRetries);

      // Verify that the user sees a helpful system message
      const sysMsg = received.find(
        (i) =>
          i.role === "system" &&
          typeof i.content?.[0]?.text === "string" &&
          i.content[0].text.includes("Rate limit reached"),
      );

      expect(sysMsg).toBeTruthy();
    } finally {
      // Ensure global timer state is restored for subsequent tests
      vi.useRealTimers();
    }
  });
});
