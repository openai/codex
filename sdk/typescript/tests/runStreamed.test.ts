import path from "path";

import { describe, expect, it } from "@jest/globals";

import { Codex, ConversationEvent } from "../src/index.js";

import {
  assistantMessage,
  responseCompleted,
  responseStarted,
  sse,
  startResponsesTestProxy,
} from "./responsesProxy.js";

const codexExecPath = path.join(process.cwd(), "..", "..", "codex-rs", "target", "debug", "codex");

describe("Codex", () => {
  it("returns session events", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBody: sse(responseStarted(), assistantMessage("Hi!"), responseCompleted()),
    });

    try {
      const client = new Codex({ executablePath: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread();
      const result = await thread.runStreamed("Hello, world!");
  
      const events: ConversationEvent[] = []
      for await (const event of result.events) {
        events.push(event);
      }
  
      expect(events).toEqual([
        {
          type: "session.created",
          session_id: expect.any(String),
        },
        {
          type: "turn.started",
        },
        {
          type: "item.completed",
          item: {
            id: "item_0",
            item_type: "assistant_message",
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
      
    }
    finally {
      await close();
    }
  });
});
