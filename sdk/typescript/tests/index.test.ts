import path from "path";

import { describe, expect, it } from "@jest/globals";

import { Codex } from "../src/index.js";

import { assistantMessage, responseCompleted, responseStarted, sse, startResponsesTestProxy } from "./responsesProxy.js";


const codexExecPath = path.join(process.cwd(), "..", "..", "codex-rs", "target", "debug", "codex");

describe("Codex", () => {
  it("returns session events", async () => {
    const {url, close} = await startResponsesTestProxy({
      statusCode: 200,
      responseBody: sse(
        responseStarted(),
        assistantMessage("Hi!"),
        responseCompleted(),
      )
    });
    
    const client = new Codex({ executablePath: codexExecPath, baseUrl: url });

    const result = await client.createConversation().run("Hello, world!");
    
    const expectedItems = [
      {
        id: expect.any(String),
        item_type: "assistant_message",
        text: "Hi!",
      },
    ];
    expect(result.items).toEqual(expectedItems);

    await close();
  }, 20000);
});
