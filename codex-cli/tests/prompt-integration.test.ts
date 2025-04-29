import { describe, it, expect, vi, beforeEach } from "vitest";
import { AgentLoop } from "../src/utils/agent/agent-loop";
import { loadPrompts } from "../src/prompts/loader";
import type { ApprovalPolicy } from "../src/approvals";

// Mock required dependencies
vi.mock("../src/utils/config", () => ({
  getApiKey: vi.fn().mockReturnValue("test-api-key"),
  getBaseUrl: vi.fn().mockReturnValue("https://api.test.com"),
}));

describe("Prompt system integration with agent loop", () => {
  let mockConfig: {
    model: string;
    instructions: string;
    provider: string;
    [key: string]: unknown;
  };
  let mockApprovalPolicy: ApprovalPolicy;

  beforeEach(() => {
    mockConfig = {
      model: "test-model",
      instructions: "Test instructions",
      provider: "openai",
    };

    mockApprovalPolicy = "suggest";
  });

  it("integrates with OpenAI models correctly", () => {
    const agent = new AgentLoop({
      model: "gpt-4-turbo",
      provider: "openai",
      config: mockConfig,
      instructions: mockConfig.instructions,
      approvalPolicy: mockApprovalPolicy,
      additionalWritableRoots: [],
      onItem: vi.fn(),
      onLoading: vi.fn(),
      getCommandConfirmation: vi.fn(),
      onLastResponseId: vi.fn(),
    });

    expect(agent).toBeDefined();
  });

  it("integrates with Anthropic models correctly", () => {
    const agent = new AgentLoop({
      model: "claude-3-opus",
      provider: "anthropic",
      config: mockConfig,
      instructions: mockConfig.instructions,
      approvalPolicy: mockApprovalPolicy,
      additionalWritableRoots: [],
      onItem: vi.fn(),
      onLoading: vi.fn(),
      getCommandConfirmation: vi.fn(),
      onLastResponseId: vi.fn(),
    });

    expect(agent).toBeDefined();
  });

  it("integrates with Gemini models correctly", () => {
    const agent = new AgentLoop({
      model: "gemini-pro",
      provider: "gemini",
      config: mockConfig,
      instructions: mockConfig.instructions,
      approvalPolicy: mockApprovalPolicy,
      additionalWritableRoots: [],
      onItem: vi.fn(),
      onLoading: vi.fn(),
      getCommandConfirmation: vi.fn(),
      onLastResponseId: vi.fn(),
    });

    expect(agent).toBeDefined();
  });

  it("adapts prompts correctly based on provider", () => {
    // Load prompts for all providers
    const openaiPrompts = loadPrompts({
      cwd: "/",
      provider: "openai",
      model: "gpt-4-turbo",
    });

    const claudePrompts = loadPrompts({
      cwd: "/",
      provider: "anthropic",
      model: "claude-3-opus",
    });

    const geminiPrompts = loadPrompts({
      cwd: "/",
      provider: "gemini",
      model: "gemini-pro",
    });

    // Each provider should have properly adapted system prompts
    expect(openaiPrompts.systemPrompt).toContain("FINAL REMINDER");
    expect(claudePrompts.systemPrompt).toContain(
      "CRITICALLY IMPORTANT INSTRUCTIONS FOR TOOL USAGE",
    );
    expect(geminiPrompts.systemPrompt).toContain(
      "FOLLOW THESE STEPS PRECISELY",
    );

    // Verify the adapted prompts are different for each provider
    expect(openaiPrompts.systemPrompt).not.toEqual(claudePrompts.systemPrompt);
    expect(openaiPrompts.systemPrompt).not.toEqual(geminiPrompts.systemPrompt);
    expect(claudePrompts.systemPrompt).not.toEqual(geminiPrompts.systemPrompt);
  });
});
