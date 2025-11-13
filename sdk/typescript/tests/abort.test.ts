import path from "node:path";

import { describe, expect, it } from "@jest/globals";

import { Codex } from "../src/codex";

import {
  assistantMessage,
  responseCompleted,
  responseStarted,
  sse,
  startResponsesTestProxy,
} from "./responsesProxy";

const codexExecPath = path.join(process.cwd(), "..", "..", "codex-rs", "target", "debug", "codex");

describe("AbortSignal support", () => {
  it("aborts run() when signal is aborted", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [sse(responseStarted(), assistantMessage("Hi!"), responseCompleted())],
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
      responseBodies: [sse(responseStarted(), assistantMessage("Hi!"), responseCompleted())],
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
      responseBodies: [sse(responseStarted(), assistantMessage("Hi!"), responseCompleted())],
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
      responseBodies: [sse(responseStarted(), assistantMessage("Hi!"), responseCompleted())],
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });
      const thread = client.startThread();

      const controller = new AbortController();

      const { events } = await thread.runStreamed("Hello, world!", { signal: controller.signal });

      // Abort during iteration
      let eventCount = 0;
      await expect(async () => {
        for await (const event of events) {
          void event; // Consume the event
          eventCount++;
          // Abort after first event
          if (eventCount === 1) {
            controller.abort("Aborted during iteration");
          }
          // Continue iterating - should eventually throw
        }
      }).rejects.toThrow();
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

  it("works without a signal (backward compatibility)", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [sse(responseStarted(), assistantMessage("Hi!"), responseCompleted())],
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });
      const thread = client.startThread();

      // Should work fine without any signal
      const result = await thread.run("Hello, world!");

      expect(result.finalResponse).toBe("Hi!");
      expect(result.items).toHaveLength(1);
    } finally {
      await close();
    }
  });

  it("actually kills a long-running process when aborted", async () => {
    // Create a server that continuously returns tool call requests, forcing codex into an infinite execution loop
    const http = await import("node:http");
    let serverClosed = false;

    const server = http.createServer((req, res) => {
      if (req.method === "POST" && req.url === "/responses") {
        res.statusCode = 200;
        res.setHeader("content-type", "text/event-stream");

        // Start the response
        res.write("event: response.created\n");
        res.write('data: {"type":"response.created","response":{"id":"resp_test"}}\n\n');

        let toolCallCount = 0;
        // Continuously stream tool call events in a loop, simulating an agent that keeps executing tools
        const interval = setInterval(() => {
          if (serverClosed) {
            clearInterval(interval);
            return;
          }
          toolCallCount++;
          // Stream a tool call in progress
          res.write("event: response.output_item.done\n");
          res.write(
            `data: {"type":"response.output_item.done","item":{"id":"tool_${toolCallCount}","type":"mcp_tool_call","server":"test_server","tool":"infinite_loop","arguments":{},"status":"in_progress"}}\n\n`,
          );

          // Then immediately complete it
          setTimeout(() => {
            if (!serverClosed) {
              res.write("event: response.output_item.done\n");
              res.write(
                `data: {"type":"response.output_item.done","item":{"id":"tool_${toolCallCount}","type":"mcp_tool_call","server":"test_server","tool":"infinite_loop","arguments":{},"result":{"content":[{"type":"text","text":"keep going"}],"structured_content":null},"status":"completed"}}\n\n`,
              );
            }
          }, 50);
        }, 100); // New tool call every 100ms

        // Clean up on client disconnect
        req.on("close", () => {
          clearInterval(interval);
        });
        res.on("close", () => {
          clearInterval(interval);
        });
      } else {
        res.statusCode = 404;
        res.end();
      }
    });

    const url = await new Promise<string>((resolve, reject) => {
      server.listen(0, "127.0.0.1", () => {
        const address = server.address();
        if (!address || typeof address === "string") {
          reject(new Error("Unable to determine server address"));
          return;
        }
        resolve(`http://${address.address}:${address.port}`);
      });
      server.once("error", reject);
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });
      const thread = client.startThread();

      const controller = new AbortController();
      const startTime = Date.now();

      // Start the long-running operation with infinite tool calls
      const runPromise = thread.runStreamed("Start infinite tool loop", {
        signal: controller.signal,
      });

      // Abort after 200ms (should be mid-execution with several tool calls processed)
      setTimeout(() => controller.abort("Test timeout"), 200);

      // The operation should fail due to abort
      await expect(async () => {
        const { events } = await runPromise;
        for await (const event of events) {
          void event; // Should be interrupted mid-stream
        }
      }).rejects.toThrow();

      const elapsed = Date.now() - startTime;

      // Verify we aborted quickly (not after natural completion)
      // Should complete within ~300ms (200ms wait + abort overhead)
      // If abort didn't work, it would hang indefinitely as tool calls keep coming
      expect(elapsed).toBeLessThan(500);
    } finally {
      serverClosed = true;
      await new Promise<void>((resolve, reject) => {
        server.close((err) => {
          if (err) reject(err);
          else resolve();
        });
      });
    }
  }, 10000); // 10s timeout for the test itself
});
