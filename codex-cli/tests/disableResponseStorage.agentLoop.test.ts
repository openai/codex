/**
 * codex-cli/tests/disableResponseStorage.agentLoop.test.ts
 *
 * Verifies AgentLoop's request-building logic for both values of
 * disableResponseStorage.
 */

import { describe, it, expect, vi } from "vitest";
import { AgentLoop } from "../src/utils/agent/agent-loop";
import type { AppConfig } from "../src/utils/config";
import { ReviewDecision } from "../src/utils/agent/review";

/* ─────────── 1.  Spy + module mock ─────────────────────────────── */
const createSpy = vi.fn().mockResolvedValue({
  data: { id: "resp_123", status: "completed", output: [] },
});

vi.mock("openai", () => ({
  default: class {
    public responses = { create: createSpy };
  },
  APIConnectionTimeoutError: class extends Error {},
}));

/* ─────────── 2.  Parametrised tests ─────────────────────────────── */
describe.each([
  { flag: true, title: "omits previous_response_id & sets store:false" },
  { flag: false, title: "sends previous_response_id & allows store:true" },
])("AgentLoop with disableResponseStorage=%s", ({ flag, title }) => {
  /* build a fresh config for each case */
  const cfg: AppConfig = {
    model: "codex-mini-latest",
    provider: "openai",
    instructions: "",
    disableResponseStorage: flag,
    notify: false,
  };

  it(title, async () => {
    /* reset spy per iteration */
    createSpy.mockClear();

    const loop = new AgentLoop({
      model: cfg.model,
      provider: cfg.provider,
      config: cfg,
      instructions: "",
      approvalPolicy: "suggest",
      disableResponseStorage: flag,
      additionalWritableRoots: [],
      onItem() {},
      onLoading() {},
      getCommandConfirmation: async () => ({ review: ReviewDecision.YES }),
      onLastResponseId() {},
    });

    await loop.run([
      {
        type: "message",
        role: "user",
        content: [{ type: "input_text", text: "hello" }],
      },
    ]);

    expect(createSpy).toHaveBeenCalledTimes(1);

    const call = createSpy.mock.calls[0];
    if (!call) {
      throw new Error("Expected createSpy to have been called at least once");
    }
    const payload: any = call[0];

    if (flag) {
      /* behaviour when ZDR is *on* */
      expect(payload).not.toHaveProperty("previous_response_id");
      if (payload.input) {
        payload.input.forEach((m: any) => {
          expect(m.store === undefined ? false : m.store).toBe(false);
        });
      }
    } else {
      /* behaviour when ZDR is *off* */
      expect(payload).toHaveProperty("previous_response_id");
      if (payload.input) {
        payload.input.forEach((m: any) => {
          if ("store" in m) {
            expect(m.store).not.toBe(false);
          }
        });
      }
    }
  });
});
