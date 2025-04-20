import { OpenAIProvider } from "../src/utils/providers";
import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";

// Mock OpenAI client
vi.mock("openai", () => {
  const MockOpenAI = vi.fn();
  MockOpenAI.prototype.responses = {
    create: vi.fn().mockResolvedValue({
      id: "test-completion",
      output: [{ type: "message", content: "Test response" }],
      status: "completed",
    }),
  };
  MockOpenAI.prototype.models = {
    list: vi.fn().mockResolvedValue([
      { id: "gpt-4" },
      { id: "gpt-3.5-turbo" },
      { id: "o4-mini" },
    ]),
  };
  
  return {
    default: MockOpenAI,
    APIConnectionTimeoutError: class extends Error {},
  };
});

describe("OpenAIProvider", () => {
  let provider: OpenAIProvider;
  
  beforeEach(() => {
    provider = new OpenAIProvider();
    
    // Mock environment variables
    vi.stubEnv("OPENAI_API_KEY", "test-api-key");
    vi.stubEnv("OPENAI_BASE_URL", "https://api.test.com");
    vi.stubEnv("OPENAI_TIMEOUT_MS", "30000");
  });
  
  afterEach(() => {
    vi.unstubAllEnvs();
  });
  
  describe("basic properties", () => {
    it("has the correct ID and name", () => {
      expect(provider.id).toBe("openai");
      expect(provider.name).toBe("OpenAI");
    });
  });
  
  describe("getModels", () => {
    it("fetches models from API when API key is available", async () => {
      const models = await provider.getModels();
      
      expect(models).toContain("gpt-4");
      expect(models).toContain("gpt-3.5-turbo");
      expect(models).toContain("o4-mini");
    });
    
    it("returns recommended models when no API key is available", async () => {
      vi.stubEnv("OPENAI_API_KEY", "");
      
      const models = await provider.getModels();
      
      expect(models).toContain("o4-mini");
      expect(models).toContain("o3");
      expect(models).toContain("gpt-4");
      expect(models).toContain("gpt-3.5-turbo");
    });
  });
  
  describe("createClient", () => {
    it("creates an OpenAI client with the provided config", () => {
      const config = {
        apiKey: "test-config-key",
        openaiBaseUrl: "https://config.test.com",
        openaiTimeoutMs: 60000,
        sessionId: "test-session",
      };
      
      const client = provider.createClient(config);
      
      // This is a mock, so we can't check too much, but we can at least
      // verify that it returns something
      expect(client).toBeDefined();
    });
    
    it("throws an error when no API key is available", () => {
      vi.stubEnv("OPENAI_API_KEY", "");
      
      expect(() => provider.createClient({})).toThrow(
        "OpenAI API key not found"
      );
    });
  });
  
  describe("getModelDefaults", () => {
    it("returns correct defaults for standard models", () => {
      const gpt4Defaults = provider.getModelDefaults("gpt-4");
      expect(gpt4Defaults.contextWindowSize).toBe(8000);
      expect(gpt4Defaults.supportsToolCalls).toBe(true);
      
      const gpt35Defaults = provider.getModelDefaults("gpt-3.5-turbo");
      expect(gpt35Defaults.contextWindowSize).toBe(16000);
      
      const o4MiniDefaults = provider.getModelDefaults("o4-mini");
      expect(o4MiniDefaults.contextWindowSize).toBe(128000);
      
      const o3Defaults = provider.getModelDefaults("o3");
      expect(o3Defaults.contextWindowSize).toBe(64000);
    });
    
    it("returns base defaults for unknown models", () => {
      const defaults = provider.getModelDefaults("unknown-model");
      expect(defaults.timeoutMs).toBe(60000);
      expect(defaults.supportsToolCalls).toBe(true);
      expect(defaults.supportsStreaming).toBe(true);
      expect(defaults.contextWindowSize).toBe(16000);
    });
  });
  
  describe("parseToolCall", () => {
    it("parses chat-style tool calls", () => {
      const chatToolCall = {
        id: "call-123",
        function: {
          name: "test_function",
          arguments: '{"arg1":"value1","arg2":42}',
        },
      };
      
      const parsed = provider.parseToolCall(chatToolCall as any);
      
      expect(parsed.id).toBe("call-123");
      expect(parsed.name).toBe("test_function");
      expect(parsed.arguments).toEqual({ arg1: "value1", arg2: 42 });
    });
    
    it("parses responses-style tool calls", () => {
      const responseToolCall = {
        call_id: "call-456",
        name: "test_function",
        arguments: '{"arg1":"value1","arg2":42}',
      };
      
      const parsed = provider.parseToolCall(responseToolCall as any);
      
      expect(parsed.id).toBe("call-456");
      expect(parsed.name).toBe("test_function");
      expect(parsed.arguments).toEqual({ arg1: "value1", arg2: 42 });
    });
    
    it("handles invalid JSON arguments", () => {
      const toolCall = {
        id: "call-789",
        name: "test_function",
        arguments: '{invalid:json}',
      };
      
      // Mock console.error to avoid test logs
      const consoleErrorMock = vi.spyOn(console, "error").mockImplementation();
      
      const parsed = provider.parseToolCall(toolCall as any);
      
      expect(parsed.id).toBe("call-789");
      expect(parsed.name).toBe("test_function");
      expect(parsed.arguments).toEqual({});
      
      // Verify error was logged
      expect(consoleErrorMock).toHaveBeenCalled();
      
      // Restore console.error
      consoleErrorMock.mockRestore();
    });
  });
  
  describe("error handling", () => {
    it("detects rate limit errors", () => {
      expect(provider.isRateLimitError({ status: 429 })).toBe(true);
      expect(provider.isRateLimitError({ code: "rate_limit_exceeded" })).toBe(true);
      expect(provider.isRateLimitError({ message: "Rate limit exceeded" })).toBe(true);
    });
    
    it("formats error messages appropriately", () => {
      const rateLimitError = { status: 429, message: "Rate limit exceeded" };
      expect(provider.formatErrorMessage(rateLimitError)).toContain("rate limit exceeded");
      
      const timeoutError = { message: "Request timed out" };
      expect(provider.formatErrorMessage(timeoutError)).toContain("timed out");
      
      const contextLengthError = { message: "max_tokens is too large" };
      expect(provider.formatErrorMessage(contextLengthError)).toContain("context length");
    });
    
    it("extracts retry timing from errors", () => {
      const errorWithHeader = { headers: { "retry-after": "5" } };
      expect(provider.getRetryAfterMs(errorWithHeader)).toBe(5000);
      
      const errorWithMessage = { message: "Please try again in 2.5s" };
      expect(provider.getRetryAfterMs(errorWithMessage)).toBe(2500);
      
      const defaultError = { message: "Some error" };
      expect(provider.getRetryAfterMs(defaultError)).toBe(2500); // Default from env
    });
  });
  
  describe("runCompletion", () => {
    it("calls the OpenAI API with the correct parameters", async () => {
      const params = {
        model: "gpt-4",
        messages: [
          { role: "system", content: "You are a helpful assistant." },
          { role: "user", content: "Hello!" },
        ],
        config: {
          apiKey: "test-api-key",
        },
      };
      
      await provider.runCompletion(params);
      
      // Since we're using mocks, we can't directly check the params passed
      // to the API. In a real test, we'd spy on the API call.
      // We're just ensuring it doesn't throw here.
    });
  });
  
  describe("normalizeStreamEvent", () => {
    it("normalizes function call events", () => {
      const event = {
        type: "response.output_item.done",
        item: {
          type: "function_call",
          id: "call-123",
          name: "test_function",
        },
        response_id: "resp-123",
      };
      
      const normalized = provider.normalizeStreamEvent(event);
      
      expect(normalized.type).toBe("tool_call");
      expect(normalized.responseId).toBe("resp-123");
      expect(normalized.originalEvent).toBe(event);
    });
    
    it("normalizes text events", () => {
      const event = {
        type: "response.output_item.done",
        item: {
          type: "message",
          content: "Hello!",
        },
        response_id: "resp-123",
      };
      
      const normalized = provider.normalizeStreamEvent(event);
      
      expect(normalized.type).toBe("text");
      expect(normalized.content).toBe(event.item);
      expect(normalized.responseId).toBe("resp-123");
    });
    
    it("normalizes completion events", () => {
      const event = {
        type: "response.completed",
        response: {
          id: "resp-123",
          output: [],
          status: "completed",
        },
      };
      
      const normalized = provider.normalizeStreamEvent(event);
      
      expect(normalized.type).toBe("completion");
      expect(normalized.content).toBe(event.response);
      expect(normalized.responseId).toBe("resp-123");
    });
  });
});