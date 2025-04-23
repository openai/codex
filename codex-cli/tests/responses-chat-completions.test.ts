import { describe, it, expect, vi, afterEach, beforeEach } from "vitest";
import type { OpenAI } from "openai";
import type {
  ResponseCreateInput,
  ResponseEvent,
} from "../src/utils/responses";
import type {
  ResponseInputItem,
  Tool,
  ResponseCreateParams,
  ResponseFunctionToolCallItem,
  ResponseFunctionToolCall,
} from "openai/resources/responses/responses";

// Define specific types for streaming and non-streaming params
type ResponseCreateParamsStreaming = ResponseCreateParams & { stream: true };
type ResponseCreateParamsNonStreaming = ResponseCreateParams & {
  stream?: false;
};

// Define additional type guard for tool calls done event
type ToolCallsDoneEvent = Extract<
  ResponseEvent,
  { type: "response.function_call_arguments.done" }
>;
type OutputTextDeltaEvent = Extract<
  ResponseEvent,
  { type: "response.output_text.delta" }
>;
type OutputTextDoneEvent = Extract<
  ResponseEvent,
  { type: "response.output_text.done" }
>;
type ResponseCompletedEvent = Extract<
  ResponseEvent,
  { type: "response.completed" }
>;

// Mock state to control the OpenAI client behavior
const openAiState: {
  createSpy?: ReturnType<typeof vi.fn>;
  createStreamSpy?: ReturnType<typeof vi.fn>;
} = {};

// Mock the OpenAI client
vi.mock("openai", () => {
  class FakeOpenAI {
    public chat = {
      completions: {
        create: (...args: Array<any>) => {
          if (args[0]?.stream) {
            return openAiState.createStreamSpy!(...args);
          }
          return openAiState.createSpy!(...args);
        },
      },
    };
  }

  return {
    __esModule: true,
    default: FakeOpenAI,
  };
});

// Helper function to create properly typed test inputs
function createTestInput(options: {
  model: string;
  userMessage: string;
  stream?: boolean;
  tools?: Array<Tool>;
  previousResponseId?: string;
}): ResponseCreateInput {
  const message: ResponseInputItem.Message = {
    type: "message",
    role: "user",
    content: [
      {
        type: "input_text" as const,
        text: options.userMessage,
      },
    ],
  };

  const input: ResponseCreateInput = {
    model: options.model,
    input: [message],
  };

  if (options.stream !== undefined) {
    // @ts-expect-error TypeScript doesn't recognize this is valid
    input.stream = options.stream;
  }

  if (options.tools) {
    input.tools = options.tools;
  }

  if (options.previousResponseId) {
    input.previous_response_id = options.previousResponseId;
  }

  return input;
}

// Type guard for function call content
function isFunctionCall(content: any): content is ResponseFunctionToolCall {
  return (
    content && typeof content === "object" && content.type === "function_call"
  );
}

// Additional type guard for tool call
function isToolCall(item: any): item is ResponseFunctionToolCallItem {
  return item && typeof item === "object" && item.type === "function";
}

// Type guards for various event types
export function _isToolCallsDoneEvent(
  event: ResponseEvent,
): event is ToolCallsDoneEvent {
  return event.type === "response.function_call_arguments.done";
}

function isOutputTextDeltaEvent(
  event: ResponseEvent,
): event is OutputTextDeltaEvent {
  return event.type === "response.output_text.delta";
}

function isOutputTextDoneEvent(
  event: ResponseEvent,
): event is OutputTextDoneEvent {
  return event.type === "response.output_text.done";
}

function isResponseCompletedEvent(
  event: ResponseEvent,
): event is ResponseCompletedEvent {
  return event.type === "response.completed";
}

// Helper function to create a mock stream for tool calls testing
function createToolCallsStream() {
  async function* fakeToolStream() {
    yield {
      id: "chatcmpl-123",
      model: "gpt-4o",
      choices: [
        {
          delta: { role: "assistant" },
          finish_reason: null,
          index: 0,
        },
      ],
    };
    yield {
      id: "chatcmpl-123",
      model: "gpt-4o",
      choices: [
        {
          delta: {
            tool_calls: [
              {
                index: 0,
                id: "call_123",
                type: "function",
                function: { name: "get_weather" },
              },
            ],
          },
          finish_reason: null,
          index: 0,
        },
      ],
    };
    yield {
      id: "chatcmpl-123",
      model: "gpt-4o",
      choices: [
        {
          delta: {
            tool_calls: [
              {
                index: 0,
                function: {
                  arguments: '{"location":"San Franci',
                },
              },
            ],
          },
          finish_reason: null,
          index: 0,
        },
      ],
    };
    yield {
      id: "chatcmpl-123",
      model: "gpt-4o",
      choices: [
        {
          delta: {
            tool_calls: [
              {
                index: 0,
                function: {
                  arguments: 'sco"}',
                },
              },
            ],
          },
          finish_reason: null,
          index: 0,
        },
      ],
    };
    yield {
      id: "chatcmpl-123",
      model: "gpt-4o",
      choices: [
        {
          delta: {},
          finish_reason: "tool_calls",
          index: 0,
        },
      ],
      usage: { prompt_tokens: 10, completion_tokens: 5, total_tokens: 15 },
    };
  }

  return fakeToolStream();
}

describe("responsesCreateViaChatCompletions", () => {
  // Using any type here to avoid import issues
  let responsesModule: any;

  beforeEach(async () => {
    vi.resetModules();
    responsesModule = await import("../src/utils/responses");
  });

  afterEach(() => {
    vi.resetAllMocks();
    openAiState.createSpy = undefined;
    openAiState.createStreamSpy = undefined;
  });

  describe("non-streaming mode", () => {
    it("should convert basic user message to chat completions format", async () => {
      // Setup mock response
      openAiState.createSpy = vi.fn().mockResolvedValue({
        id: "chat-123",
        model: "gpt-4o",
        choices: [
          {
            message: {
              role: "assistant",
              content: "This is a test response",
            },
            finish_reason: "stop",
          },
        ],
        usage: {
          prompt_tokens: 10,
          completion_tokens: 5,
          total_tokens: 15,
        },
      });

      const openaiClient = new (await import("openai")).default({
        apiKey: "test-key",
      }) as unknown as OpenAI;

      const inputMessage = createTestInput({
        model: "gpt-4o",
        userMessage: "Hello world",
        stream: false,
      });

      const result = await responsesModule.responsesCreateViaChatCompletions(
        openaiClient,
        inputMessage as ResponseCreateParams & { stream?: false | undefined },
      );

      // Verify OpenAI was called with correct parameters
      expect(openAiState.createSpy).toHaveBeenCalledTimes(1);

      // Skip type checking for mock objects in tests - this is acceptable for test code
      // @ts-ignore
      const callArgs = openAiState.createSpy?.mock?.calls?.[0]?.[0];
      if (callArgs) {
        expect(callArgs.model).toBe("gpt-4o");
        expect(callArgs.messages).toEqual([
          { role: "user", content: "Hello world" },
        ]);
        expect(callArgs.stream).toBe(false);
      }

      // Verify result format
      expect(result.id).toBeDefined();
      expect(result.object).toBe("response");
      expect(result.model).toBe("gpt-4o");
      expect(result.status).toBe("completed");
      expect(result.output).toHaveLength(1);

      // Use type guard to check the output item type
      const outputItem = result.output[0];
      expect(outputItem).toBeDefined();

      if (outputItem && outputItem.type === "message") {
        expect(outputItem.role).toBe("assistant");
        expect(outputItem.content).toHaveLength(1);

        const content = outputItem.content[0];
        if (content && content.type === "output_text") {
          expect(content.text).toBe("This is a test response");
        }
      }

      expect(result.usage?.total_tokens).toBe(15);
    });

    it("should handle function calling correctly", async () => {
      // Setup mock response with tool calls
      openAiState.createSpy = vi.fn().mockResolvedValue({
        id: "chat-456",
        model: "gpt-4o",
        choices: [
          {
            message: {
              role: "assistant",
              content: null,
              tool_calls: [
                {
                  id: "call_abc123",
                  type: "function",
                  function: {
                    name: "get_weather",
                    arguments: JSON.stringify({ location: "New York" }),
                  },
                },
              ],
            },
            finish_reason: "tool_calls",
          },
        ],
        usage: {
          prompt_tokens: 15,
          completion_tokens: 8,
          total_tokens: 23,
        },
      });

      const openaiClient = new (await import("openai")).default({
        apiKey: "test-key",
      }) as unknown as OpenAI;

      // Define function tool correctly
      const weatherTool = {
        type: "function" as const,
        name: "get_weather",
        description: "Get the current weather",
        strict: true,
        parameters: {
          type: "object",
          properties: {
            location: { type: "string" },
          },
          required: ["location"],
        },
      };

      const inputMessage = createTestInput({
        model: "gpt-4o",
        userMessage: "What's the weather in New York?",
        tools: [weatherTool as any],
        stream: false,
      });

      const result = await responsesModule.responsesCreateViaChatCompletions(
        openaiClient,
        inputMessage as ResponseCreateParams & { stream: false },
      );

      // Verify OpenAI was called with correct parameters
      expect(openAiState.createSpy).toHaveBeenCalledTimes(1);

      // Skip type checking for mock objects in tests
      // @ts-ignore
      const callArgs = openAiState.createSpy?.mock?.calls?.[0]?.[0];
      if (callArgs) {
        expect(callArgs.model).toBe("gpt-4o");
        expect(callArgs.tools).toHaveLength(1);
        expect(callArgs.tools[0].function.name).toBe("get_weather");
      }

      // Verify function call output directly instead of trying to check type
      expect(result.output).toHaveLength(1);

      const outputItem = result.output[0];
      if (outputItem && outputItem.type === "message") {
        const content = outputItem.content[0];

        // Use the type guard function
        expect(isFunctionCall(content)).toBe(true);

        // Using type assertion after type guard check
        if (isFunctionCall(content)) {
          // These properties should exist on ResponseFunctionToolCall
          expect((content as any).name).toBe("get_weather");
          expect(JSON.parse((content as any).arguments).location).toBe(
            "New York",
          );
        }
      }
    });

    it("should preserve conversation history", async () => {
      // First interaction
      openAiState.createSpy = vi.fn().mockResolvedValue({
        id: "chat-789",
        model: "gpt-4o",
        choices: [
          {
            message: {
              role: "assistant",
              content: "Hello! How can I help you?",
            },
            finish_reason: "stop",
          },
        ],
        usage: { prompt_tokens: 5, completion_tokens: 6, total_tokens: 11 },
      });

      const openaiClient = new (await import("openai")).default({
        apiKey: "test-key",
      }) as unknown as OpenAI;

      const firstInput = createTestInput({
        model: "gpt-4o",
        userMessage: "Hi there",
        stream: false,
      });

      const firstResponse =
        await responsesModule.responsesCreateViaChatCompletions(
          openaiClient,
          firstInput as unknown as ResponseCreateParamsNonStreaming & {
            stream?: false | undefined;
          },
        );

      // Reset the mock for second interaction
      openAiState.createSpy.mockReset();
      openAiState.createSpy = vi.fn().mockResolvedValue({
        id: "chat-790",
        model: "gpt-4o",
        choices: [
          {
            message: {
              role: "assistant",
              content: "I'm an AI assistant created by Anthropic.",
            },
            finish_reason: "stop",
          },
        ],
        usage: { prompt_tokens: 15, completion_tokens: 10, total_tokens: 25 },
      });

      // Second interaction with previous_response_id
      const secondInput = createTestInput({
        model: "gpt-4o",
        userMessage: "Who are you?",
        previousResponseId: firstResponse.id,
        stream: false,
      });

      await responsesModule.responsesCreateViaChatCompletions(
        openaiClient,
        secondInput as unknown as ResponseCreateParamsNonStreaming & {
          stream?: false | undefined;
        },
      );

      // Verify history was included in second call
      expect(openAiState.createSpy).toHaveBeenCalledTimes(1);

      // Skip type checking for mock objects in tests
      // @ts-ignore
      const secondCallArgs = openAiState.createSpy?.mock?.calls?.[0]?.[0];
      if (secondCallArgs) {
        // Should have 3 messages: original user, assistant response, and new user message
        expect(secondCallArgs.messages).toHaveLength(3);
        expect(secondCallArgs.messages[0].role).toBe("user");
        expect(secondCallArgs.messages[0].content).toBe("Hi there");
        expect(secondCallArgs.messages[1].role).toBe("assistant");
        expect(secondCallArgs.messages[1].content).toBe(
          "Hello! How can I help you?",
        );
        expect(secondCallArgs.messages[2].role).toBe("user");
        expect(secondCallArgs.messages[2].content).toBe("Who are you?");
      }
    });

    it("handles tools correctly", async () => {
      const testFunction = {
        type: "function" as const,
        name: "get_weather",
        description: "Get the weather",
        strict: true,
        parameters: {
          type: "object",
          properties: {
            location: {
              type: "string",
              description: "The location to get the weather for",
            },
          },
          required: ["location"],
        },
      };

      // Mock response with a tool call
      openAiState.createSpy = vi.fn().mockResolvedValue({
        id: "chatcmpl-123",
        created: Date.now(),
        model: "gpt-4o",
        object: "chat.completion",
        choices: [
          {
            message: {
              role: "assistant",
              content: null,
              tool_calls: [
                {
                  id: "call_123",
                  type: "function",
                  function: {
                    name: "get_weather",
                    arguments: JSON.stringify({ location: "San Francisco" }),
                  },
                },
              ],
            },
            finish_reason: "tool_calls",
            index: 0,
          },
        ],
      });

      const openaiClient = new (await import("openai")).default({
        apiKey: "test-key",
      }) as unknown as OpenAI;

      const inputMessage = createTestInput({
        model: "gpt-4o",
        userMessage: "What's the weather in San Francisco?",
        tools: [testFunction],
      });

      const result = await responsesModule.responsesCreateViaChatCompletions(
        openaiClient,
        inputMessage as unknown as ResponseCreateParamsNonStreaming,
      );

      expect(result.status).toBe("requires_action");

      // Cast result to include required_action to address TypeScript issues
      const resultWithAction = result as any;

      // Add null checks for required_action
      expect(resultWithAction.required_action).not.toBeNull();
      expect(resultWithAction.required_action?.type).toBe(
        "submit_tool_outputs",
      );

      // Safely access the tool calls with proper null checks
      const toolCalls =
        resultWithAction.required_action?.submit_tool_outputs?.tool_calls || [];
      expect(toolCalls.length).toBe(1);

      if (toolCalls.length > 0) {
        const toolCall = toolCalls[0];
        expect(toolCall.type).toBe("function");

        if (isToolCall(toolCall)) {
          // Access with type assertion after type guard
          expect((toolCall as any).function.name).toBe("get_weather");
          expect(JSON.parse((toolCall as any).function.arguments)).toEqual({
            location: "San Francisco",
          });
        }
      }

      // Only check model, messages, and tools in exact match
      expect(openAiState.createSpy).toHaveBeenCalledWith(
        expect.objectContaining({
          model: "gpt-4o",
          messages: [
            {
              role: "user",
              content: "What's the weather in San Francisco?",
            },
          ],
          tools: [
            expect.objectContaining({
              type: "function",
              function: {
                name: "get_weather",
                description: "Get the weather",
                parameters: {
                  type: "object",
                  properties: {
                    location: {
                      type: "string",
                      description: "The location to get the weather for",
                    },
                  },
                  required: ["location"],
                },
              },
            }),
          ],
        }),
      );
    });
  });

  describe("streaming mode", () => {
    it("should handle streaming responses correctly", async () => {
      // Mock an async generator for streaming
      async function* fakeStream() {
        yield {
          id: "chatcmpl-123",
          model: "gpt-4o",
          choices: [
            {
              delta: { role: "assistant" },
              finish_reason: null,
              index: 0,
            },
          ],
        };
        yield {
          id: "chatcmpl-123",
          model: "gpt-4o",
          choices: [
            {
              delta: { content: "Hello" },
              finish_reason: null,
              index: 0,
            },
          ],
        };
        yield {
          id: "chatcmpl-123",
          model: "gpt-4o",
          choices: [
            {
              delta: { content: " world" },
              finish_reason: null,
              index: 0,
            },
          ],
        };
        yield {
          id: "chatcmpl-123",
          model: "gpt-4o",
          choices: [
            {
              delta: {},
              finish_reason: "stop",
              index: 0,
            },
          ],
          usage: { prompt_tokens: 5, completion_tokens: 2, total_tokens: 7 },
        };
      }

      openAiState.createStreamSpy = vi.fn().mockResolvedValue(fakeStream());

      const openaiClient = new (await import("openai")).default({
        apiKey: "test-key",
      }) as unknown as OpenAI;

      const inputMessage = createTestInput({
        model: "gpt-4o",
        userMessage: "Say hello",
        stream: true,
      });

      const streamGenerator =
        await responsesModule.responsesCreateViaChatCompletions(
          openaiClient,
          inputMessage as unknown as ResponseCreateParamsStreaming & {
            stream: true;
          },
        );

      // Collect all events from the stream
      const events: Array<ResponseEvent> = [];
      for await (const event of streamGenerator) {
        events.push(event);
      }

      // Verify stream generation
      expect(events.length).toBeGreaterThan(0);

      // Check initial events
      const firstEvent = events[0];
      const secondEvent = events[1];
      expect(firstEvent?.type).toBe("response.created");
      expect(secondEvent?.type).toBe("response.in_progress");

      // Find content delta events using proper type guard
      const deltaEvents = events.filter(isOutputTextDeltaEvent);

      // Should have two delta events for "Hello" and " world"
      expect(deltaEvents).toHaveLength(2);
      expect(deltaEvents[0]?.delta).toBe("Hello");
      expect(deltaEvents[1]?.delta).toBe(" world");

      // Check final completion event with type guard
      const completionEvent = events.find(isResponseCompletedEvent);
      expect(completionEvent).toBeDefined();
      if (completionEvent) {
        expect(completionEvent.response.status).toBe("completed");
      }

      // Text should be concatenated
      const textDoneEvent = events.find(isOutputTextDoneEvent);
      expect(textDoneEvent).toBeDefined();
      if (textDoneEvent) {
        expect(textDoneEvent.text).toBe("Hello world");
      }
    });

    it("handles streaming with tool calls", async () => {
      // Mock a streaming response with tool calls
      const mockStream = createToolCallsStream();
      openAiState.createStreamSpy = vi.fn().mockReturnValue(mockStream);

      const openaiClient = new (await import("openai")).default({
        apiKey: "test-key",
      }) as unknown as OpenAI;

      const testFunction = {
        type: "function" as const,
        name: "get_weather",
        description: "Get the current weather",
        strict: true,
        parameters: {
          type: "object",
          properties: {
            location: { type: "string" },
          },
          required: ["location"],
        },
      };

      const inputMessage = createTestInput({
        model: "gpt-4o",
        userMessage: "What's the weather in San Francisco?",
        tools: [testFunction],
        stream: true,
      });

      const streamGenerator =
        await responsesModule.responsesCreateViaChatCompletions(
          openaiClient,
          inputMessage as unknown as ResponseCreateParamsStreaming,
        );

      // Collect all events from the stream
      const events: Array<ResponseEvent> = [];
      for await (const event of streamGenerator) {
        events.push(event);
      }

      // Verify stream generation
      expect(events.length).toBeGreaterThan(0);

      // Look for function call related events of any type related to tool calls
      const toolCallEvents = events.filter(
        (event) =>
          event.type.includes("function_call") ||
          event.type.includes("tool") ||
          (event.type === "response.output_item.added" &&
            "item" in event &&
            event.item?.type === "function_call"),
      );

      expect(toolCallEvents.length).toBeGreaterThan(0);

      // Check if we have the completed event which should contain the final result
      const completedEvent = events.find(isResponseCompletedEvent);
      expect(completedEvent).toBeDefined();

      if (completedEvent) {
        // Get the function call from the output array
        const functionCallItem = completedEvent.response.output.find(
          (item) => item.type === "function_call",
        );
        expect(functionCallItem).toBeDefined();

        if (functionCallItem && functionCallItem.type === "function_call") {
          expect(functionCallItem.name).toBe("get_weather");
          // The arguments is a JSON string, but we can check if it includes San Francisco
          expect(functionCallItem.arguments).toContain("San Francisco");
        }
      }
    });
  });
});
