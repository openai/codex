import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// ---------------------------------------------------------------------------
// Mock helpers
// ---------------------------------------------------------------------------

// Keep reference so test cases can programmatically change behaviour of the
// fake OpenAI client.
const openAiState: {
  createSpy?: ReturnType<typeof vi.fn>;
  streamEvents?: Array<any>;
  throwRateLimitOnEventIndex?: number;
  streamAttempts?: number;
  shouldSucceedAfterRetries?: number;
} = {
  streamEvents: [],
  throwRateLimitOnEventIndex: -1,
  streamAttempts: 0,
  shouldSucceedAfterRetries: 1, // Succeed after this many retries
};

/**
 * Mock the "openai" package so we can simulate mid-stream rate‑limit errors without
 * making real network calls.
 */
vi.mock("openai", () => {
  // Create a class that will be used for APIError
  class APIError extends Error {
    status?: number;
    error?: any;
    code?: string;
    type?: string;

    constructor(status?: number, error?: any, message?: string, _headers?: Record<string, string>) {
      super(message || error?.message || "API Error");
      this.status = status;
      if (error) {
        this.error = error;
        this.code = error.code;
        this.type = error.type;
      }
    }
  }

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
    APIError,
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
vi.mock("../src/utils/logger/log.js", () => ({
  __esModule: true,
  log: () => {},
  isLoggingEnabled: () => false,
}));

import { AgentLoop } from "../src/utils/agent/agent-loop.js";
import { DEFAULT_RATE_LIMIT_MAX_RETRIES } from "../src/utils/config.js";

beforeEach(() => {
  // Reset state before each test
  openAiState.streamAttempts = 0;
  openAiState.shouldSucceedAfterRetries = 1;
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("AgentLoop – mid-stream rate‑limit handling", () => {
  it("retries when rate limit error occurs during streaming", async () => {
    // Enable fake timers for this test
    vi.useFakeTimers();

    try {
      // Import the APIError from the mocked openai module
      const { APIError } = await import("openai");

      // Set up stream events that will be returned
      openAiState.streamEvents = [
        {
          type: "response.output_item.done",
          item: {
            type: "message",
            role: "assistant",
            content: [{ type: "input_text", text: "Hello! How can I help you today?" }],
          },
        },
        {
          type: "response.completed",
          response: {
            id: "resp_123",
            status: "completed",
            output: [],
          },
        },
      ];

      // Create a mock stream generator function
      const createMockStream = () => {
        openAiState.streamAttempts!++;

        // After the specified number of retries, return a successful stream
        const shouldSucceed = openAiState.streamAttempts! > openAiState.shouldSucceedAfterRetries!;

        return {
          [Symbol.asyncIterator]: () => ({
            next: async () => {
              // If we haven't reached the success threshold, throw a rate limit error
              if (!shouldSucceed) {
                const rateLimitErr = new APIError(
                  429,
                  { code: "rate_limit_exceeded", type: "rate_limit_exceeded" },
                  "Rate limit exceeded. Please try again in 1.5s",
                  {}
                );
                throw rateLimitErr;
              }

              // Otherwise return the events successfully
              if (openAiState.streamEvents!.length > 0) {
                const event = openAiState.streamEvents!.shift();
                return { done: false, value: event };
              }

              return { done: true, value: undefined };
            },
          }),
          controller: { abort: vi.fn() },
        };
      };

      // Mock the responses.create method to return our mock stream
      openAiState.createSpy = vi.fn(async () => {
        return createMockStream();
      });

      const received: Array<any> = [];
      let loadingState = false;

      const agent = new AgentLoop({
        model: "any",
        instructions: "",
        approvalPolicy: { mode: "auto" } as any,
        config: {
          model: "any",
          instructions: "",
          notify: false,
          rateLimits: {
            maxRetries: DEFAULT_RATE_LIMIT_MAX_RETRIES,
            initialRetryDelayMs: 100, // Use small values for faster testing
            maxRetryDelayMs: 500,
            jitterFactor: 0.1,
          },
        },
        additionalWritableRoots: [],
        onItem: (i) => received.push(i),
        onLoading: (loading) => { loadingState = loading; },
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

      // Advance time to allow for the initial stream creation and first event
      await vi.advanceTimersByTimeAsync(100);

      // Advance time to cover the retry delay
      await vi.advanceTimersByTimeAsync(1000);

      // Advance more time to allow for the completion of the run
      await vi.advanceTimersByTimeAsync(1000);

      // Ensure the promise settles without throwing
      await runPromise;

      // Flush the 10 ms staging delay used when emitting items
      await vi.advanceTimersByTimeAsync(50);

      // The stream should have been created twice: once initially and once after the rate limit error
      expect(openAiState.streamAttempts).toBe(2);

      // Verify that loading state is false at the end
      expect(loadingState).toBe(false);
    } finally {
      // Ensure global timer state is restored for subsequent tests
      vi.useRealTimers();
    }
  });

  it("respects suggested retry time from rate limit error message", async () => {
    // Enable fake timers for this test
    vi.useFakeTimers();

    try {
      // Import the APIError from the mocked openai module
      const { APIError } = await import("openai");

      // Create a spy for setTimeout to verify the delay
      const setTimeoutSpy = vi.spyOn(global, 'setTimeout');

      // Suggested retry time in seconds
      const suggestedRetrySeconds = 2.5;

      // Set up stream events
      openAiState.streamEvents = [
        {
          type: "response.output_item.done",
          item: {
            type: "message",
            role: "assistant",
            content: [{ type: "input_text", text: "Hello!" }],
          },
        },
        {
          type: "response.completed",
          response: {
            id: "resp_123",
            status: "completed",
            output: [],
          },
        },
      ];

      // Create a mock stream generator function
      const createMockStream = () => {
        openAiState.streamAttempts!++;

        // After the specified number of retries, return a successful stream
        const shouldSucceed = openAiState.streamAttempts! > openAiState.shouldSucceedAfterRetries!;

        return {
          [Symbol.asyncIterator]: () => ({
            next: async () => {
              // If we haven't reached the success threshold, throw a rate limit error
              if (!shouldSucceed) {
                const rateLimitErr = new APIError(
                  429,
                  { code: "rate_limit_exceeded", type: "rate_limit_exceeded" },
                  `Rate limit exceeded. Please try again in ${suggestedRetrySeconds}s`,
                  {}
                );
                throw rateLimitErr;
              }

              // Otherwise return the events successfully
              if (openAiState.streamEvents!.length > 0) {
                const event = openAiState.streamEvents!.shift();
                return { done: false, value: event };
              }

              return { done: true, value: undefined };
            },
          }),
          controller: { abort: vi.fn() },
        };
      };

      // Mock the responses.create method
      openAiState.createSpy = vi.fn(async () => {
        return createMockStream();
      });

      const agent = new AgentLoop({
        model: "any",
        instructions: "",
        approvalPolicy: { mode: "auto" } as any,
        config: {
          model: "any",
          instructions: "",
          notify: false,
          rateLimits: {
            maxRetries: DEFAULT_RATE_LIMIT_MAX_RETRIES,
            initialRetryDelayMs: 100, // Use small values for faster testing
            maxRetryDelayMs: 10000,
            jitterFactor: 0.1,
          },
        },
        additionalWritableRoots: [],
        onItem: () => {},
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

      // Advance time to trigger the rate limit error
      await vi.advanceTimersByTimeAsync(100);

      // Advance time to cover the retry delay
      await vi.advanceTimersByTimeAsync(5000);

      // Complete the run
      await vi.advanceTimersByTimeAsync(1000);

      // Ensure the promise settles
      await runPromise;

      // Verify that setTimeout was called with a delay close to the suggested retry time
      // The expected delay should be close to the suggested retry time in ms
      const expectedDelayMs = suggestedRetrySeconds * 1000;

      // Find the setTimeout call for the retry delay
      const setTimeoutCalls = setTimeoutSpy.mock.calls;

      // Look for a setTimeout call with a delay close to our expected value
      // We can't check for an exact match due to jitter, but it should be close
      const hasExpectedDelay = setTimeoutCalls.some(call => {
        const delay = call[1] as number;
        // Allow for some margin of error due to jitter
        return Math.abs(delay - expectedDelayMs) < 1000;
      });

      expect(hasExpectedDelay).toBe(true);

    } finally {
      vi.useRealTimers();
    }
  });

  it("gives up after maximum retry attempts", async () => {
    // Enable fake timers for this test
    vi.useFakeTimers();

    try {
      // Import the APIError from the mocked openai module
      const { APIError } = await import("openai");

      // Set up stream events
      openAiState.streamEvents = [
        {
          type: "response.output_item.done",
          item: {
            type: "message",
            role: "assistant",
            content: [{ type: "input_text", text: "Hello!" }],
          },
        },
      ];

      // Configure to never succeed (require more retries than we'll allow)
      openAiState.shouldSucceedAfterRetries = 10; // This is higher than our maxRetries

      // Create a mock stream that will throw a rate limit error
      const createMockStream = () => {
        openAiState.streamAttempts!++;

        return {
          [Symbol.asyncIterator]: () => ({
            next: async () => {
              // Always throw a rate limit error
              const rateLimitErr = new APIError(
                429,
                { code: "rate_limit_exceeded", type: "rate_limit_exceeded" },
                "Rate limit exceeded",
                {}
              );
              throw rateLimitErr;
            },
          }),
          controller: { abort: vi.fn() },
        };
      };

      // Mock the responses.create method
      openAiState.createSpy = vi.fn(async () => {
        return createMockStream();
      });

      const received: Array<any> = [];

      // Use a smaller number of retries for faster testing
      const maxRetries = 3;

      const agent = new AgentLoop({
        model: "any",
        instructions: "",
        approvalPolicy: { mode: "auto" } as any,
        config: {
          model: "any",
          instructions: "",
          notify: false,
          rateLimits: {
            maxRetries: maxRetries,
            initialRetryDelayMs: 100, // Use small values for faster testing
            maxRetryDelayMs: 500,
            jitterFactor: 0.1,
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

      // Start the run but don't await it directly
      const runPromise = agent.run(userMsg as any);

      // Advance time to cover all retry attempts
      for (let i = 0; i <= maxRetries; i++) {
        await vi.advanceTimersByTimeAsync(100); // Initial attempt
        await vi.advanceTimersByTimeAsync(500); // Retry delay
      }

      // The promise should reject after all retries are exhausted
      await expect(runPromise).rejects.toThrow();

      // The stream should have been created at least maxRetries + 1 times (initial + retries)
      // The exact number might vary due to timing, but it should be at least the expected minimum
      expect(openAiState.streamAttempts).toBeGreaterThanOrEqual(maxRetries + 1);

    } finally {
      vi.useRealTimers();
    }
  });
});
