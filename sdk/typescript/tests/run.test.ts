import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { codexExecSpy } from "./codexExecSpy";
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

describe("Codex", () => {
  it("returns thread events", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [sse(responseStarted(), assistantMessage("Hi!"), responseCompleted())],
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread();
      const result = await thread.run("Hello, world!");

      const expectedItems = [
        {
          id: expect.any(String),
          type: "agent_message",
          text: "Hi!",
        },
      ];
      expect(result.items).toEqual(expectedItems);
      expect(thread.id).toEqual(expect.any(String));
    } finally {
      await close();
    }
  });

  it("sends previous items when run is called twice", async () => {
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
      await thread.run("first input");
      await thread.run("second input");

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

  it("continues the thread when run is called twice with options", async () => {
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
      await thread.run("first input");
      await thread.run("second input");

      // Check second request continues the same thread
      expect(requests.length).toBeGreaterThanOrEqual(2);
      const secondRequest = requests[1];
      expect(secondRequest).toBeDefined();
      const payload = secondRequest!.json;

      expect(payload.input.at(-1)!.content![0]!.text).toBe("second input");
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

  it("resumes thread by id", async () => {
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
      await originalThread.run("first input");

      const resumedThread = client.resumeThread(originalThread.id!);
      const result = await resumedThread.run("second input");

      expect(resumedThread.id).toBe(originalThread.id);
      expect(result.finalResponse).toBe("Second response");

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

  it("passes turn options to exec", async () => {
    const { url, close, requests } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Turn options applied", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });

    const { args: spawnArgs, restore } = codexExecSpy();

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread({
        model: "gpt-test-1",
        sandboxMode: "workspace-write",
      });
      await thread.run("apply options");

      const payload = requests[0];
      expect(payload).toBeDefined();
      const json = payload!.json as { model?: string } | undefined;

      expect(json?.model).toBe("gpt-test-1");
      expect(spawnArgs.length).toBeGreaterThan(0);
      const commandArgs = spawnArgs[0];

      expectPair(commandArgs, ["--sandbox", "workspace-write"]);
      expectPair(commandArgs, ["--model", "gpt-test-1"]);
    } finally {
      restore();
      await close();
    }
  });
  it("runs in provided working directory", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Working directory applied", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });

    const { args: spawnArgs, restore } = codexExecSpy();

    try {
      const workingDirectory = fs.mkdtempSync(path.join(os.tmpdir(), "codex-working-dir-"));
      const client = new Codex({
        codexPathOverride: codexExecPath,
        baseUrl: url,
        apiKey: "test",
      });

      const thread = client.startThread({
        workingDirectory,
        skipGitRepoCheck: true,
      });
      await thread.run("use custom working directory");

      const commandArgs = spawnArgs[0];
      expectPair(commandArgs, ["--cd", workingDirectory]);
    } finally {
      restore();
      await close();
    }
  });

  it("throws if working directory is not git and no skipGitRepoCheck is provided", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Working directory applied", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });

    try {
      const workingDirectory = fs.mkdtempSync(path.join(os.tmpdir(), "codex-working-dir-"));
      const client = new Codex({
        codexPathOverride: codexExecPath,
        baseUrl: url,
        apiKey: "test",
      });

      const thread = client.startThread({
        workingDirectory,
      });
      await expect(thread.run("use custom working directory")).rejects.toThrow(
        /Not inside a trusted directory/,
      );
    } finally {
      await close();
    }
  });
});
function expectPair(args: string[] | undefined, pair: [string, string]) {
  if (!args) {
    throw new Error("Args is undefined");
  }
  const index = args.indexOf(pair[0]);
  if (index === -1) {
    throw new Error(`Pair ${pair[0]} not found in args`);
  }
  expect(args[index + 1]).toBe(pair[1]);
}
