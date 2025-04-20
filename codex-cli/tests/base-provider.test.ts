import { BaseProvider, ProviderErrorType } from "../src/utils/providers";
import { describe, it, expect, beforeEach } from "vitest";

// Mock provider implementation for testing
class TestProvider extends BaseProvider {
  id = "test";
  name = "Test Provider";
  
  async getModels() {
    return ["test-model-1", "test-model-2"];
  }
  
  createClient() {
    return { id: this.id };
  }
  
  async runCompletion() {
    return { id: "test-completion" };
  }
  
  getModelDefaults() {
    return {
      timeoutMs: 30000,
      supportsToolCalls: true,
      supportsStreaming: true,
      contextWindowSize: 8192,
    };
  }
  
  parseToolCall(rawToolCall: any) {
    return {
      id: rawToolCall.id || "test-id",
      name: rawToolCall.name || "test-name",
      arguments: rawToolCall.arguments || {},
    };
  }
  
  formatTools(tools: any[]) {
    return tools;
  }
  
  normalizeStreamEvent(event: any) {
    return {
      type: "text",
      content: "test content",
      responseId: "test-id",
      originalEvent: event,
    };
  }
}

describe("BaseProvider", () => {
  let provider: TestProvider;
  
  beforeEach(() => {
    provider = new TestProvider();
  });
  
  describe("isModelSupported", () => {
    it("returns true for supported models", async () => {
      const isSupported = await provider.isModelSupported("test-model-1");
      expect(isSupported).toBe(true);
    });
    
    it("returns false for unsupported models", async () => {
      const isSupported = await provider.isModelSupported("unsupported-model");
      expect(isSupported).toBe(false);
    });
  });
  
  describe("error detection", () => {
    it("detects rate limit errors", () => {
      expect(provider.isRateLimitError({ status: 429 })).toBe(true);
      expect(provider.isRateLimitError({ code: "rate_limit_exceeded" })).toBe(true);
      expect(provider.isRateLimitError({ type: "rate_limit_exceeded" })).toBe(true);
      expect(provider.isRateLimitError({ message: "Rate limit exceeded" })).toBe(true);
      expect(provider.isRateLimitError({ message: "Too many requests" })).toBe(true);
      expect(provider.isRateLimitError({ status: 400 })).toBe(false);
    });
    
    it("detects timeout errors", () => {
      expect(provider.isTimeoutError({ name: "AbortError" })).toBe(true);
      expect(provider.isTimeoutError({ code: "ETIMEDOUT" })).toBe(true);
      expect(provider.isTimeoutError({ message: "Request timed out" })).toBe(true);
      expect(provider.isTimeoutError({ status: 400 })).toBe(false);
    });
    
    it("detects connection errors", () => {
      expect(provider.isConnectionError({ code: "ECONNRESET" })).toBe(true);
      expect(provider.isConnectionError({ code: "ECONNREFUSED" })).toBe(true);
      expect(provider.isConnectionError({ message: "Network error" })).toBe(true);
      expect(provider.isConnectionError({ status: 400 })).toBe(false);
    });
    
    it("detects context length errors", () => {
      expect(provider.isContextLengthError({ code: "context_length_exceeded" })).toBe(true);
      expect(provider.isContextLengthError({ message: "Maximum context length exceeded" })).toBe(true);
      expect(provider.isContextLengthError({ message: "Too many tokens" })).toBe(true);
      expect(provider.isContextLengthError({ status: 400 })).toBe(false);
    });
    
    it("detects invalid request errors", () => {
      expect(provider.isInvalidRequestError({ status: 400 })).toBe(true);
      expect(provider.isInvalidRequestError({ type: "invalid_request_error" })).toBe(true);
      expect(provider.isInvalidRequestError({ status: 500 })).toBe(false);
      expect(provider.isInvalidRequestError({ status: 429 })).toBe(false);
    });
  });
  
  describe("error formatting", () => {
    it("formats error messages from different structures", () => {
      expect(provider.formatErrorMessage({ message: "Test error" }))
        .toBe("API Error: Test error");
      
      expect(provider.formatErrorMessage({ error: { message: "Nested error" } }))
        .toBe("API Error: Nested error");
      
      expect(provider.formatErrorMessage({}))
        .toBe("Unknown API error occurred");
    });
  });
  
  describe("retry timing", () => {
    it("extracts retry-after from headers", () => {
      const error = { headers: { "retry-after": "5" } };
      expect(provider.getRetryAfterMs(error)).toBe(5000);
    });
    
    it("extracts retry timing from error message", () => {
      const error = { message: "Rate limit exceeded. Retry again in 2.5s" };
      expect(provider.getRetryAfterMs(error)).toBe(2500);
    });
    
    it("returns default retry time if not specified", () => {
      const error = { message: "Rate limit exceeded" };
      expect(provider.getRetryAfterMs(error)).toBe(2500);
    });
  });
  
  describe("error standardization", () => {
    it("standardizes rate limit errors", () => {
      const error = { status: 429, message: "Rate limit exceeded" };
      const standardized = provider.standardizeError(error);
      
      expect(standardized.type).toBe(ProviderErrorType.RATE_LIMIT);
      expect(standardized.message).toBe("API Error: Rate limit exceeded");
      expect(standardized.retryable).toBe(true);
      expect(standardized.originalError).toBe(error);
    });
    
    it("standardizes timeout errors", () => {
      const error = { code: "ETIMEDOUT", message: "Request timed out" };
      const standardized = provider.standardizeError(error);
      
      expect(standardized.type).toBe(ProviderErrorType.TIMEOUT);
      expect(standardized.retryable).toBe(true);
    });
    
    it("standardizes context length errors", () => {
      const error = { code: "context_length_exceeded", message: "Too many tokens" };
      const standardized = provider.standardizeError(error);
      
      expect(standardized.type).toBe(ProviderErrorType.CONTEXT_LENGTH);
      expect(standardized.retryable).toBe(false);
    });
    
    it("standardizes authentication errors", () => {
      const error = { status: 401, message: "Invalid API key" };
      const standardized = provider.standardizeError(error);
      
      expect(standardized.type).toBe(ProviderErrorType.AUTHENTICATION);
      expect(standardized.retryable).toBe(false);
    });
    
    it("standardizes server errors", () => {
      const error = { status: 500, message: "Internal server error" };
      const standardized = provider.standardizeError(error);
      
      expect(standardized.type).toBe(ProviderErrorType.SERVER);
      expect(standardized.retryable).toBe(true);
    });
    
    it("standardizes unknown errors", () => {
      const error = { message: "Some weird error" };
      const standardized = provider.standardizeError(error);
      
      expect(standardized.type).toBe(ProviderErrorType.UNKNOWN);
      expect(standardized.retryable).toBe(false);
    });
  });
});