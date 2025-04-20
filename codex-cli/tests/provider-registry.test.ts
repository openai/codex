import { BaseProvider, LLMProvider, ProviderRegistry } from "../src/utils/providers";
import type { AppConfig } from "../src/utils/config";
import { describe, it, expect, beforeEach } from "vitest";

// Mock provider implementation for testing
class MockProvider extends BaseProvider implements LLMProvider {
  constructor(public id: string, public name: string) {
    super();
  }
  
  async getModels() {
    return [`${this.id}-model-1`, `${this.id}-model-2`];
  }
  
  createClient() {
    return { id: this.id };
  }
  
  async runCompletion() {
    return { id: "mock-completion" };
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
      id: rawToolCall.id || "mock-id",
      name: rawToolCall.name || "mock-name",
      arguments: rawToolCall.arguments || {},
    };
  }
  
  formatTools(tools: any[]) {
    return tools;
  }
  
  normalizeStreamEvent(event: any) {
    return {
      type: "text",
      content: "mock content",
      responseId: "mock-id",
      originalEvent: event,
    };
  }
}

describe("ProviderRegistry", () => {
  beforeEach(() => {
    // Clear registry before each test
    ProviderRegistry.clearProviders();
  });
  
  it("registers and retrieves providers", () => {
    const openaiProvider = new MockProvider("openai", "OpenAI");
    const claudeProvider = new MockProvider("claude", "Claude");
    
    ProviderRegistry.register(openaiProvider);
    ProviderRegistry.register(claudeProvider);
    
    expect(ProviderRegistry.getProviderById("openai")).toBe(openaiProvider);
    expect(ProviderRegistry.getProviderById("claude")).toBe(claudeProvider);
    expect(ProviderRegistry.getAllProviders().length).toBe(2);
    expect(ProviderRegistry.hasProvider("openai")).toBe(true);
    expect(ProviderRegistry.hasProvider("nonexistent")).toBe(false);
  });
  
  it("returns the default provider", () => {
    const openaiProvider = new MockProvider("openai", "OpenAI");
    ProviderRegistry.register(openaiProvider);
    
    expect(ProviderRegistry.getDefaultProvider()).toBe(openaiProvider);
    expect(ProviderRegistry.getDefaultProviderId()).toBe("openai");
  });
  
  it("allows changing the default provider", () => {
    const openaiProvider = new MockProvider("openai", "OpenAI");
    const claudeProvider = new MockProvider("claude", "Claude");
    
    ProviderRegistry.register(openaiProvider);
    ProviderRegistry.register(claudeProvider);
    
    ProviderRegistry.setDefaultProviderId("claude");
    
    expect(ProviderRegistry.getDefaultProvider()).toBe(claudeProvider);
    expect(ProviderRegistry.getDefaultProviderId()).toBe("claude");
  });
  
  it("throws when setting a non-existent provider as default", () => {
    expect(() => {
      ProviderRegistry.setDefaultProviderId("nonexistent");
    }).toThrow();
  });
  
  it("throws when no providers are registered", () => {
    expect(() => {
      ProviderRegistry.getDefaultProvider();
    }).toThrow("No LLM providers registered");
  });
  
  it("selects the correct provider based on model name", () => {
    const openaiProvider = new MockProvider("openai", "OpenAI");
    const claudeProvider = new MockProvider("claude", "Claude");
    
    ProviderRegistry.register(openaiProvider);
    ProviderRegistry.register(claudeProvider);
    
    expect(ProviderRegistry.getProviderForModel("gpt-4")).toBe(openaiProvider);
    expect(ProviderRegistry.getProviderForModel("o4-mini")).toBe(openaiProvider);
    expect(ProviderRegistry.getProviderForModel("claude-3-opus")).toBe(claudeProvider);
    
    // Unknown model should return default
    expect(ProviderRegistry.getProviderForModel("unknown-model")).toBe(openaiProvider);
  });
  
  it("falls back to default provider if requested provider not found", () => {
    const openaiProvider = new MockProvider("openai", "OpenAI");
    ProviderRegistry.register(openaiProvider);
    
    // Claude model but no Claude provider registered
    expect(ProviderRegistry.getProviderForModel("claude-3-opus")).toBe(openaiProvider);
  });
});