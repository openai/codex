import path from "node:path";

import { describe, expect, it } from "@jest/globals";

import { Codex } from "../src/codex";
import { ThreadEvent } from "../src/index";

import {
  assistantMessage,
  responseCompleted,
  responseStarted,
  sse,
  startResponsesTestProxy,
} from "./responsesProxy";

const codexExecPath = path.join(process.cwd(), "..", "..", "codex-rs", "target", "debug", "codex");

describe("Codex", () => {
  it("returns thread events", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [sse(responseStarted(), assistantMessage("Hi!"), responseCompleted())],
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread();
      const result = await thread.runStreamed("Hello, world!");

      const events: ThreadEvent[] = [];
      for await (const event of result.events) {
        events.push(event);
      }

      expect(events).toEqual([
        {
          type: "thread.started",
          thread_id: expect.any(String),
        },
        {
          type: "turn.started",
        },
        {
          type: "item.completed",
          item: {
            id: "item_0",
            type: "agent_message",
            text: "Hi!",
          },
        },
        {
          type: "turn.completed",
          usage: {
            cached_input_tokens: 0,
            input_tokens: 0,
            output_tokens: 0,
          },
        },
      ]);
      expect(thread.id).toEqual(expect.any(String));
    } finally {
      await close();
    }
  });

  it("sends previous items when runStreamed is called twice", async () => {
    const { url, close, requests } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("First response", "item_1"),
          responseCompleted("response_1"),
        ),
        sse(
          responseStarted("response_2"),
          assistantMessage("Second response", "item_2"),
          responseCompleted("response_2"),
        ),
      ],
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread();
      const first = await thread.runStreamed("first input");
      await drainEvents(first.events);

      const second = await thread.runStreamed("second input");
      await drainEvents(second.events);

      // Check second request continues the same thread
      expect(requests.length).toBeGreaterThanOrEqual(2);
      const secondRequest = requests[1];
      expect(secondRequest).toBeDefined();
      const payload = secondRequest!.json;

      const assistantEntry = payload.input.find(
        (entry: { role: string }) => entry.role === "assistant",
      );
      expect(assistantEntry).toBeDefined();
      const assistantText = assistantEntry?.content?.find(
        (item: { type: string; text: string }) => item.type === "output_text",
      )?.text;
      expect(assistantText).toBe("First response");
    } finally {
      await close();
    }
  });

  it("resumes thread by id when streaming", async () => {
    const { url, close, requests } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("First response", "item_1"),
          responseCompleted("response_1"),
        ),
        sse(
          responseStarted("response_2"),
          assistantMessage("Second response", "item_2"),
          responseCompleted("response_2"),
        ),
      ],
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const originalThread = client.startThread();
      const first = await originalThread.runStreamed("first input");
      await drainEvents(first.events);

      const resumedThread = client.resumeThread(originalThread.id!);
      const second = await resumedThread.runStreamed("second input");
      await drainEvents(second.events);

      expect(resumedThread.id).toBe(originalThread.id);

      expect(requests.length).toBeGreaterThanOrEqual(2);
      const secondRequest = requests[1];
      expect(secondRequest).toBeDefined();
      const payload = secondRequest!.json;

      const assistantEntry = payload.input.find(
        (entry: { role: string }) => entry.role === "assistant",
      );
      expect(assistantEntry).toBeDefined();
      const assistantText = assistantEntry?.content?.find(
        (item: { type: string; text: string }) => item.type === "output_text",
      )?.text;
      expect(assistantText).toBe("First response");
    } finally {
      await close();
    }
  });
});

async function drainEvents(events: AsyncGenerator<ThreadEvent>): Promise<void> {
  let done = false;
  do {
    done = (await events.next()).done ?? false;
  } while (!done);
}
