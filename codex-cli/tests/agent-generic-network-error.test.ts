import { describe, it, expect, vi } from "vitest";

// ---------------------------------------------------------------------------
//  Utility helpers & OpenAI mock (lightweight – focuses on network failures)
// ---------------------------------------------------------------------------

const openAiState: { createSpy?: ReturnType<typeof vi.fn> } = {};

vi.mock("openai", () => {
  class FakeOpenAI {
    public responses = {
      create: (...args: Array<any>) => openAiState.createSpy!(...args),
    };
  }

  class APIConnectionTimeoutError extends Error {}

  return {
    __esModule: true,
    default: FakeOpenAI,
    APIConnectionTimeoutError,
  };
});

// Stub approvals / formatting helpers – unrelated to network handling.
vi.mock("../src/approvals.js", () => ({
  __esModule: true,
  alwaysApprovedCommands: new Set<string>(),
  canAutoApprove: () => ({ type: "auto-approve", runInSandbox: false }) as any,
  isSafeCommand: () => null,
}));

vi.mock("../src/format-command.js", () => ({
  __esModule: true,
  formatCommandForDisplay: (c: Array<string>) => c.join(" "),
}));

// Silence debug logs so test output stays clean.
vi.mock("../src/utils/agent/log.js", () => ({
  __esModule: true,
  log: () => {},
  isLoggingEnabled: () => false,
}));

import { AgentLoop } from "../src/utils/agent/agent-loop.js";

describe("AgentLoop – generic network/server errors", () => {
  it("emits friendly system message instead of throwing on ECONNRESET", async () => {
    const netErr: any = new Error("socket hang up");
    netErr.code = "ECONNRESET";

    openAiState.createSpy = vi.fn(async () => {
      throw netErr;
    });

    const received: Array<any> = [];

    const agent = new AgentLoop({
      additionalWritableRoots: [],
      model: "any",
      instructions: "",
      approvalPolicy: { mode: "auto" } as any,
      onItem: (i) => received.push(i),
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });

    const userMsg = [
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "ping" }],
      },
    ];

    await expect(agent.run(userMsg as any)).resolves.not.toThrow();

    // give flush timers a chance
    await new Promise((r) => setTimeout(r, 20));

    const sysMsg = received.find(
      (i) =>
        i.role === "system" &&
        typeof i.content?.[0]?.text === "string" &&
        i.content[0].text.includes("Network error"),
    );

    expect(sysMsg).toBeTruthy();
  });

  it("emits user friendly message on HTTP 500 from OpenAI", async () => {
    const serverErr: any = new Error("Internal Server Error");
    serverErr.status = 500;

    openAiState.createSpy = vi.fn(async () => {
      throw serverErr;
    });

    const received: Array<any> = [];

    const agent = new AgentLoop({
      additionalWritableRoots: [],
      model: "any",
      instructions: "",
      approvalPolicy: { mode: "auto" } as any,
      onItem: (i) => received.push(i),
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });

    const userMsg = [
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "ping" }],
      },
    ];

    await expect(agent.run(userMsg as any)).resolves.not.toThrow();

    await new Promise((r) => setTimeout(r, 20));

    const sysMsg = received.find(
      (i) =>
        i.role === "system" &&
        typeof i.content?.[0]?.text === "string" &&
        i.content[0].text.includes("error"),
    );

    expect(sysMsg).toBeTruthy();
  });
});
