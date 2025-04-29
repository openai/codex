import { describe, it, expect, vi, beforeEach } from "vitest";
import { AgentLoop } from "../src/utils/agent/agent-loop";
import { loadPrompts, adaptPromptForProvider } from "../src/prompts/loader";
import type { ApprovalPolicy } from "../src/approvals";

// Mock required dependencies
vi.mock("../src/utils/config", () => ({
  getApiKey: vi.fn().mockReturnValue("test-api-key"),
  getBaseUrl: vi.fn().mockReturnValue("https://api.test.com"),
  OPENAI_TIMEOUT_MS: 60000,
  OPENAI_ORGANIZATION: "test-org",
  OPENAI_PROJECT: "test-project",
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

    // Get provider-adapted prompts
    const openaiAdapted = adaptPromptForProvider(
      openaiPrompts.systemPrompt,
      "openai",
    );
    const claudeAdapted = adaptPromptForProvider(
      claudePrompts.systemPrompt,
      "anthropic",
    );
    const geminiAdapted = adaptPromptForProvider(
      geminiPrompts.systemPrompt,
      "gemini",
    );

    // Each provider should have properly adapted system prompts
    expect(openaiAdapted).toContain("FINAL REMINDER");
    expect(claudeAdapted).toContain(
      "CRITICALLY IMPORTANT INSTRUCTIONS FOR TOOL USAGE",
    );
    expect(geminiAdapted).toContain("FOLLOW THESE STEPS PRECISELY");

    // Verify the adapted prompts are different from each other
    expect(openaiAdapted).not.toEqual(claudeAdapted);
    expect(openaiAdapted).not.toEqual(geminiAdapted);
    expect(claudeAdapted).not.toEqual(geminiAdapted);
  });
});
