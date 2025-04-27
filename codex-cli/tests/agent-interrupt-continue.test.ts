import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { AgentLoop } from "../src/utils/agent/agent-loop.js";

// Create a state holder for our mocks
const openAiState = {
  createSpy: vi.fn(),
};

// Mock the OpenAI client
vi.mock("openai", () => {
  return {
    default: class MockOpenAI {
      responses = {
        create: openAiState.createSpy,
      };
    },
  };
});

describe("Agent interrupt and continue", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.resetAllMocks();
  });

  it("allows continuing after interruption", async () => {
    // Track received items
    const received: Array<any> = [];
    let loadingState = false;

    // Create the agent
    const agent = new AgentLoop({
      additionalWritableRoots: [],
      model: "test-model",
      instructions: "",
      approvalPolicy: { mode: "auto" } as any,
      config: {
        model: "test-model",
        instructions: "",
        notify: false,
      },
      onItem: (item) => received.push(item),
      onLoading: (loading) => {
        loadingState = loading;
      },
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });

    // First user message
    const firstMessage = [
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "first message" }],
      },
    ];

    // Setup the first mock response
    openAiState.createSpy.mockImplementation(() => {
      // Return a mock stream object
      return {
        controller: {
          abort: vi.fn(),
        },
        on: (event: string, callback: (...args: Array<any>) => void) => {
          if (event === "message") {
            // Schedule a message to be delivered
            setTimeout(() => {
              callback({
                type: "message",
                role: "assistant",
                content: [{ type: "input_text", text: "First response" }],
              });
            }, 10);
          }
          return { controller: { abort: vi.fn() } };
        },
      };
    });

    // Start the first run
    const firstRunPromise = agent.run(firstMessage as any);

    // Advance timers to allow the stream to start
    await vi.advanceTimersByTimeAsync(5);

    // Interrupt the agent
    agent.cancel();

    // Verify loading state is reset
    expect(loadingState).toBe(false);

    // Second user message
    const secondMessage = [
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "second message" }],
      },
    ];

    // Reset the mock to track the second call
    openAiState.createSpy.mockClear();

    // Setup the second mock response
    openAiState.createSpy.mockImplementation(() => {
      // Return a mock stream object
      return {
        controller: {
          abort: vi.fn(),
        },
        on: (event: string, callback: (...args: Array<any>) => void) => {
          if (event === "message") {
            // Schedule a message to be delivered
            setTimeout(() => {
              callback({
                type: "message",
                role: "assistant",
                content: [{ type: "input_text", text: "Second response" }],
              });
            }, 10);
          }
          return { controller: { abort: vi.fn() } };
        },
      };
    });

    // Start the second run
    const secondRunPromise = agent.run(secondMessage as any);

    // Advance timers to allow the second stream to complete
    await vi.advanceTimersByTimeAsync(20);

    // Ensure both promises resolve
    await Promise.all([firstRunPromise, secondRunPromise]);

    // Verify the second API call was made
    expect(openAiState.createSpy).toHaveBeenCalled();

    // Verify that the agent can process new input after cancellation
    expect(loadingState).toBe(false);
  });
});
