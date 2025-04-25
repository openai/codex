// ---------------------------------------------------------------------------
// Regression test for the "thinking time" counter. Today the implementation
// keeps a *single* start‑time across many requests which means that every
// subsequent command will show an ever‑increasing number such as
// "thinking for 4409s", "thinking for 4424s", … even though the individual
// turn only took a couple of milliseconds. Each request should start its own
// independent timer.
//
// We mark the spec with `.fails()` so that the overall suite remains green
// until the underlying bug is fixed. When the implementation is corrected the
// expectations below will turn green – Vitest will then error and remind us to
// remove the `.fails` flag.
// ---------------------------------------------------------------------------

import { AgentLoop } from "../src/utils/agent/agent-loop.js";
import { describe, it, expect, vi } from "vitest";

// --- OpenAI mock -----------------------------------------------------------

/**
 * Fake stream that yields a single `response.completed` after a configurable
 * delay. This allows us to simulate different thinking times for successive
 * requests while using Vitest's fake timers.
 */
class FakeStream {
  public controller = { abort: vi.fn() };
  private delay: number;

  constructor(delay: number) {
    this.delay = delay; // milliseconds
  }

  async *[Symbol.asyncIterator]() {
    if (this.delay > 0) {
      // Wait the configured delay – fake timers will fast‑forward.
      await new Promise((r) => setTimeout(r, this.delay));
    }

    yield {
      type: "response.completed",
      response: {
        id: `resp-${Date.now()}`,
        status: "completed",
        output: [
          {
            type: "message",
            role: "assistant",
            id: "m1",
            content: [{ type: "text", text: "done" }],
          },
        ],
      },
    } as any;
  }
}

/**
 * Fake OpenAI client that returns a slower stream for the *first* call and a
 * faster one for the second so we can verify that per‑task timers reset while
 * the global counter accumulates.
 */
vi.mock("openai", () => {
  let callCount = 0;
  class FakeOpenAI {
    public responses = {
      create: async () => {
        callCount += 1;
        return new FakeStream(callCount === 1 ? 10_000 : 500); // 10s vs 0.5s
      },
    };
  }
  class APIConnectionTimeoutError extends Error {}
  return { __esModule: true, default: FakeOpenAI, APIConnectionTimeoutError };
});

// Stub helpers referenced indirectly so we do not pull in real FS/network
vi.mock("../src/approvals.js", () => ({
  __esModule: true,
  isSafeCommand: () => null,
}));

vi.mock("../src/format-command.js", () => ({
  __esModule: true,
  formatCommandForDisplay: (c: Array<string>) => c.join(" "),
}));

// Suppress file‑system logging in tests.
vi.mock("../src/utils/agent/log.js", () => ({
  __esModule: true,
  log: () => {},
  isLoggingEnabled: () => false,
}));

describe("thinking time counter", () => {
  // Use fake timers for *all* tests in this suite
  vi.useFakeTimers();

  // Re‐use this array to collect all onItem callbacks
  let items: Array<any>;

  // Helper that runs two agent turns (10s + 0.5s) and populates `items`
  async function runScenario() {
    items = [];

    const agent = new AgentLoop({
      config: {} as any,
      model: "any",
      instructions: "",
      approvalPolicy: { mode: "auto" } as any,
      additionalWritableRoots: [],
      onItem: (i) => items.push(i),
      onLoading: () => {},
      getCommandConfirmation: async () => ({ review: "yes" }) as any,
      onLastResponseId: () => {},
    });

    const userMsg = {
      type: "message",
      role: "user",
      content: [{ type: "input_text", text: "do it" }],
    } as any;

    // 1️⃣ First request – simulated 10s thinking time
    agent.run([userMsg]);
    await vi.advanceTimersByTimeAsync(11_000); // 10s + flush margin

    // 2️⃣ Second request – simulated 0.5s thinking time
    agent.run([userMsg]);
    await vi.advanceTimersByTimeAsync(1_000); // 0.5s + flush margin
  }

  // TODO: this is disabled
  it.fails("reports correct per-task thinking time per command", async () => {
    await runScenario();

    const perTaskMsgs = items.filter(
      (i) =>
        i.role === "system" &&
        i.content?.[0]?.text?.startsWith("🤔  Thinking time:"),
    );

    expect(perTaskMsgs.length).toBe(2);

    const perTaskDurations = perTaskMsgs.map((m) => {
      const match = m.content[0].text.match(/Thinking time: (\d+) s/);
      return match ? parseInt(match[1]!, 10) : NaN;
    });

    // First run ~10s, second run ~0.5s
    expect(perTaskDurations[0]).toBeGreaterThanOrEqual(9);
    expect(perTaskDurations[1]).toBeLessThan(3);
  });

  // TODO: this is disabled
  it.fails("reports correct global thinking time accumulation", async () => {
    await runScenario();

    const globalMsgs = items.filter(
      (i) =>
        i.role === "system" &&
        i.content?.[0]?.text?.startsWith("⏱  Total thinking time:"),
    );

    expect(globalMsgs.length).toBe(2);

    const globalDurations = globalMsgs.map((m) => {
      const match = m.content[0].text.match(/Total thinking time: (\d+) s/);
      return match ? parseInt(match[1]!, 10) : NaN;
    });

    // Total after second run should exceed total after first
    expect(globalDurations[1]! as number).toBeGreaterThan(globalDurations[0]!);
  });
});
