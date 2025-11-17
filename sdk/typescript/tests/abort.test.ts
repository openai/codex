import path from "node:path";

import { describe, expect, it } from "@jest/globals";

import { Codex } from "../src/codex";

import {
  assistantMessage,
  responseCompleted,
  responseStarted,
  shell_call as shellCall,
  sse,
  SseResponseBody,
  startResponsesTestProxy,
} from "./responsesProxy";

const codexExecPath = path.join(process.cwd(), "..", "..", "codex-rs", "target", "debug", "codex");

function* infiniteShellCall(): Generator<SseResponseBody> {
  while (true) {
    yield sse(responseStarted(), shellCall(), responseCompleted());
  }
}

describe("AbortSignal support", () => {
  it("aborts run() when signal is aborted", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: infiniteShellCall(),
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });
      const thread = client.startThread();

      // Create an abort controller and abort it immediately
      const controller = new AbortController();
      controller.abort("Test abort");

      // The operation should fail because the signal is already aborted
      await expect(thread.run("Hello, world!", { signal: controller.signal })).rejects.toThrow();
    } finally {
      await close();
    }
  });

  it("aborts runStreamed() when signal is aborted", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: infiniteShellCall(),
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });
      const thread = client.startThread();

      // Create an abort controller and abort it immediately
      const controller = new AbortController();
      controller.abort("Test abort");

      const { events } = await thread.runStreamed("Hello, world!", { signal: controller.signal });

      // Attempting to iterate should fail
      let iterationStarted = false;
      try {
        for await (const event of events) {
          iterationStarted = true;
          // Should not get here
          expect(event).toBeUndefined();
        }
        // If we get here, the test should fail
        throw new Error(
          "Expected iteration to throw due to aborted signal, but it completed successfully",
        );
      } catch (error) {
        // We expect an error to be thrown
        expect(iterationStarted).toBe(false); // Should fail before any iteration
        expect(error).toBeDefined();
      }
    } finally {
      await close();
    }
  });

  it("aborts run() when signal is aborted during execution", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: infiniteShellCall(),
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });
      const thread = client.startThread();

      const controller = new AbortController();

      // Start the operation and abort it immediately after
      const runPromise = thread.run("Hello, world!", { signal: controller.signal });

      // Abort after a tiny delay to simulate aborting during execution
      setTimeout(() => controller.abort("Aborted during execution"), 10);

      // The operation should fail
      await expect(runPromise).rejects.toThrow();
    } finally {
      await close();
    }
  });

  it("aborts runStreamed() when signal is aborted during iteration", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: infiniteShellCall(),
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });
      const thread = client.startThread();

      const controller = new AbortController();

      const { events } = await thread.runStreamed("Hello, world!", { signal: controller.signal });

      // Abort during iteration
      let eventCount = 0;
      await expect(
        (async () => {
          for await (const event of events) {
            void event; // Consume the event
            eventCount++;
            // Abort after first event
            if (eventCount === 5) {
              controller.abort("Aborted during iteration");
            }
            // Continue iterating - should eventually throw
          }
        })(),
      ).rejects.toThrow();
    } finally {
      await close();
    }
  });

  it("completes normally when signal is not aborted", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [sse(responseStarted(), assistantMessage("Hi!"), responseCompleted())],
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });
      const thread = client.startThread();

      const controller = new AbortController();

      // Don't abort - should complete successfully
      const result = await thread.run("Hello, world!", { signal: controller.signal });

      expect(result.finalResponse).toBe("Hi!");
      expect(result.items).toHaveLength(1);
    } finally {
      await close();
    }
  });
});
