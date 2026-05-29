import { describe, expect, it } from "@jest/globals";

import { ThreadEvent } from "../src/index";

import {
  assistantMessage,
  responseCompleted,
  responseStarted,
  sse,
  startResponsesTestProxy,
} from "./responsesProxy";
import { createMockClient } from "./testCodex";

describe("Codex", () => {
  it("returns thread events", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [sse(responseStarted(), assistantMessage("Hi!"), responseCompleted())],
    });
    const { client, cleanup } = createMockClient(url);

    try {
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
            cached_input_tokens: 12,
            input_tokens: 42,
            output_tokens: 5,
            reasoning_output_tokens: 0,
          },
        },
      ]);
      expect(thread.id).toEqual(expect.any(String));
    } finally {
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
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
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
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
      cleanup();
      await close();
    }
  });

  it("applies output schema turn options when streaming", async () => {
    const { url, close, requests } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Structured response", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });
    const { client, cleanup } = createMockClient(url);

    const schema = {
      type: "object",
      properties: {
        answer: { type: "string" },
      },
      required: ["answer"],
      additionalProperties: false,
    } as const;

    try {
      const thread = client.startThread();
      const streamed = await thread.runStreamed("structured", { outputSchema: schema });
      await drainEvents(streamed.events);

      expect(requests.length).toBeGreaterThanOrEqual(1);
      const payload = requests[0];
      expect(payload).toBeDefined();
      const text = payload!.json.text;
      expect(text).toBeDefined();
      expect(text?.format).toEqual({
        name: "codex_output_schema",
        type: "json_schema",
        strict: true,
        schema,
      });
    } finally {
      cleanup();
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
