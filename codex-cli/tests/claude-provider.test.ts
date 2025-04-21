import { describe, test, expect, vi, beforeEach } from "vitest";
// Mock the Anthropic SDK for ClaudeProvider tests
import Anthropic from "@anthropic-ai/sdk";
vi.mock("@anthropic-ai/sdk");
import { ClaudeProvider } from "../src/utils/providers/claude-provider";

describe("ClaudeProvider", () => {
  let provider: ClaudeProvider;
  
  beforeEach(() => {
    // Create a new provider instance before each test
    provider = new ClaudeProvider();
    
    // Mock environment variables for testing
    vi.stubEnv("CLAUDE_API_KEY", "test-claude-api-key");
  });
  
  test("should have correct id and name", () => {
    expect(provider.id).toBe("claude");
    expect(provider.name).toBe("Claude");
  });
  
  test("getModels returns the expected static model list", async () => {
    const models = await provider.getModels();
    expect(models).toEqual([
      "claude-3-5-sonnet-20240620",
      "claude-3-opus-20240229",
      "claude-3-haiku-20240307",
    ]);
  });
  
  test("should fallback to recommended models when API key is not set", async () => {
    // Remove API key
    vi.stubEnv("CLAUDE_API_KEY", "");
    vi.stubEnv("ANTHROPIC_API_KEY", "");
    
    // Call getModels
    const models = await provider.getModels();
    
    // Should return recommended models
    expect(models.length).toBeGreaterThan(0);
    expect(models.some(model => model.startsWith("claude-3"))).toBe(true);
  });
  
  test("should return appropriate defaults for different Claude models", () => {
    // Test opus model defaults
    const opusDefaults = provider.getModelDefaults("claude-3-opus-20240229");
    expect(opusDefaults.supportsToolCalls).toBe(true);
    expect(opusDefaults.contextWindowSize).toBeGreaterThan(100000);
    
    // Test sonnet model defaults
    const sonnetDefaults = provider.getModelDefaults("claude-3-sonnet-20240229");
    expect(sonnetDefaults.supportsToolCalls).toBe(true);
    
    // Test haiku model defaults
    const haikuDefaults = provider.getModelDefaults("claude-3-haiku-20240307");
    expect(haikuDefaults.supportsToolCalls).toBe(true);
  });
  
  test("should format Claude errors correctly", () => {
    // Rate limit error
    const rateLimitError = { status: 429, message: "Rate limit exceeded" };
    expect(provider.isRateLimitError(rateLimitError)).toBe(true);
    expect(provider.formatErrorMessage(rateLimitError)).toContain("Rate limit exceeded");
    
    // Context length error
    const contextError = { 
      status: 400, 
      message: "Input is too long, max tokens exceeded" 
    };
    expect(provider.isContextLengthError(contextError)).toBe(true);
    expect(provider.formatErrorMessage(contextError)).toContain("Input is too long");
    
    // Authentication error
    const authError = { status: 401, message: "Unauthorized access" };
    expect(provider.isInvalidRequestError(authError)).toBe(true);
    expect(provider.formatErrorMessage(authError)).toContain("Unauthorized access");
  });
  
  test("runCompletion (non-stream) should call messages.create and map response", async () => {
    // Mock Anthropic SDK client
    const createMock = vi.fn().mockResolvedValue({ id: "dummy-id", model: "dummy-model", content: [{ type: "text", text: "OK" }] });
    const streamMock = vi.fn();
    // Configure the mocked Anthropic constructor
    (Anthropic as unknown as vi.Mock).mockImplementation(() => ({ messages: { create: createMock, stream: streamMock } }));
    // Use the provider instance from beforeEach
    // Prepare params
    const params = {
      model: "dummy-model",
      messages: [{ role: "user", content: "hi" }],
      stream: false,
      temperature: 0.3,
      maxTokens: 10,
      tools: [],
      config: { providers: { claude: { apiKey: "key" } } }
    } as any;
    // Call runCompletion
    const result = await provider.runCompletion(params);
    // Ensure messages.create was called
    expect(createMock).toHaveBeenCalledWith(expect.objectContaining({
      model: "dummy-model",
      messages: expect.any(Array),
      system: undefined,
      temperature: 0.3,
      max_tokens: 10,
      stream: false
    }));
    // Validate output mapping
    expect(result).toMatchObject({
      id: "dummy-id",
      model: "dummy-model",
      output: [
        { type: "message", role: "assistant", content: [{ type: "output_text", text: "OK" }] }
      ]
    });
  });
});