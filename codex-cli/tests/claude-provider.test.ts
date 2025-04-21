import { describe, test, expect, vi, beforeEach } from "vitest";
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
  
  test("should return available models", async () => {
    // Spy on listModels
    const mockListModels = vi.fn().mockResolvedValue({
      data: [
        { id: "claude-3-opus-20240229" },
        { id: "claude-3-sonnet-20240229" },
        { id: "claude-3-haiku-20240307" },
      ]
    });
    
    // Create a mock client
    const mockClient = {
      listModels: mockListModels
    };
    
    // Mock the createClient method to return our mock client
    vi.spyOn(provider, "createClient").mockReturnValue(mockClient as any);
    
    // Call getModels
    const models = await provider.getModels();
    
    // Verify the returned models
    expect(models).toContain("claude-3-opus-20240229");
    expect(models).toContain("claude-3-sonnet-20240229");
    expect(models).toContain("claude-3-haiku-20240307");
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
    expect(provider.formatErrorMessage(rateLimitError)).toContain("rate limit exceeded");
    
    // Context length error
    const contextError = { 
      status: 400, 
      message: "Input is too long, max tokens exceeded" 
    };
    expect(provider.isContextLengthError(contextError)).toBe(true);
    expect(provider.formatErrorMessage(contextError)).toContain("context length");
    
    // Authentication error
    const authError = { 
      status: 401, 
      error: { type: "authentication_error" } 
    };
    expect(provider.isInvalidRequestError(authError)).toBe(true);
    expect(provider.formatErrorMessage(authError)).toContain("authentication failed");
  });
});