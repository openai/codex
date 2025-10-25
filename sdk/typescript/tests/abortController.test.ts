import { describe, expect, it } from "@jest/globals";

import type { CodexExec, CodexExecArgs } from "../src/exec";
import type { CodexOptions } from "../src/codexOptions";
import { Thread } from "../src/thread";
import type { ThreadOptions } from "../src/threadOptions";

class AbortAwareExec {
  public calls: CodexExecArgs[] = [];

  async *run(args: CodexExecArgs): AsyncGenerator<string> {
    this.calls.push(args);
    yield JSON.stringify({ type: "thread.started", thread_id: "thread_123" });
    yield JSON.stringify({ type: "turn.started" });
    await waitForAbort(args.signal);
  }
}

function waitForAbort(signal?: AbortSignal): Promise<never> {
  return new Promise((_, reject) => {
    if (!signal) {
      reject(new Error("Missing abort signal"));
      return;
    }
    signal.addEventListener(
      "abort",
      () => {
        reject(signal.reason ?? new Error("aborted"));
      },
      { once: true },
    );
  });
}

describe("Thread turn cancellation", () => {
  const codexOptions = {} as CodexOptions;
  const threadOptions = {} as ThreadOptions;

  it("rejects run when the abort signal is triggered", async () => {
    const exec = new AbortAwareExec();
    const thread = new Thread(exec as unknown as CodexExec, codexOptions, threadOptions);
    const controller = new AbortController();
    const abortError = new Error("stop");

    const promise = thread.run("Hello", { signal: controller.signal });
    controller.abort(abortError);

    await expect(promise).rejects.toBe(abortError);
    expect(exec.calls[0]?.signal).toBe(controller.signal);
  });

  it("rejects runStreamed iterators when the abort signal is triggered", async () => {
    const exec = new AbortAwareExec();
    const thread = new Thread(exec as unknown as CodexExec, codexOptions, threadOptions);
    const controller = new AbortController();
    const abortError = new Error("stop");

    const { events } = await thread.runStreamed("Hello", { signal: controller.signal });
    await events.next();
    await events.next();
    const pending = events.next();
    controller.abort(abortError);

    await expect(pending).rejects.toBe(abortError);
    expect(exec.calls[0]?.signal).toBe(controller.signal);
  });
});
