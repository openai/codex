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
  responseFailed,
  startResponsesTestProxy,
  SseResponseBody,
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
      expect(result.usage).toEqual({
        cached_input_tokens: 12,
        input_tokens: 42,
        output_tokens: 5,
      });
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

  it("passes modelReasoningEffort to exec", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Reasoning effort applied", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });

    const { args: spawnArgs, restore } = codexExecSpy();

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread({
        modelReasoningEffort: "high",
      });
      await thread.run("apply reasoning effort");

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      expectPair(commandArgs, ["--config", 'model_reasoning_effort="high"']);
    } finally {
      restore();
      await close();
    }
  });

  it("passes networkAccessEnabled to exec", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Network access enabled", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });

    const { args: spawnArgs, restore } = codexExecSpy();

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread({
        networkAccessEnabled: true,
      });
      await thread.run("test network access");

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      expectPair(commandArgs, ["--config", "sandbox_workspace_write.network_access=true"]);
    } finally {
      restore();
      await close();
    }
  });

  it("passes webSearchEnabled to exec", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Web search enabled", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });

    const { args: spawnArgs, restore } = codexExecSpy();

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread({
        webSearchEnabled: true,
      });
      await thread.run("test web search");

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      expectPair(commandArgs, ["--config", 'web_search="live"']);
    } finally {
      restore();
      await close();
    }
  });

  it("passes webSearchMode to exec", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Web search cached", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });

    const { args: spawnArgs, restore } = codexExecSpy();

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread({
        webSearchMode: "cached",
      });
      await thread.run("test web search mode");

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      expectPair(commandArgs, ["--config", 'web_search="cached"']);
    } finally {
      restore();
      await close();
    }
  });

  it("passes webSearchEnabled false to exec", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Web search disabled", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });

    const { args: spawnArgs, restore } = codexExecSpy();

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread({
        webSearchEnabled: false,
      });
      await thread.run("test web search disabled");

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      expectPair(commandArgs, ["--config", 'web_search="disabled"']);
    } finally {
      restore();
      await close();
    }
  });

  it("passes approvalPolicy to exec", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Approval policy set", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });

    const { args: spawnArgs, restore } = codexExecSpy();

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread({
        approvalPolicy: "on-request",
      });
      await thread.run("test approval policy");

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      expectPair(commandArgs, ["--config", 'approval_policy="on-request"']);
    } finally {
      restore();
      await close();
    }
  });

  it("allows overriding the env passed to the Codex CLI", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Custom env", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });

    const { envs: spawnEnvs, restore } = codexExecSpy();
    process.env.CODEX_ENV_SHOULD_NOT_LEAK = "leak";

    try {
      const client = new Codex({
        codexPathOverride: codexExecPath,
        baseUrl: url,
        apiKey: "test",
        env: { CUSTOM_ENV: "custom" },
      });

      const thread = client.startThread();
      await thread.run("custom env");

      const spawnEnv = spawnEnvs[0];
      expect(spawnEnv).toBeDefined();
      if (!spawnEnv) {
        throw new Error("Spawn env missing");
      }
      expect(spawnEnv.CUSTOM_ENV).toBe("custom");
      expect(spawnEnv.CODEX_ENV_SHOULD_NOT_LEAK).toBeUndefined();
      expect(spawnEnv.OPENAI_BASE_URL).toBe(url);
      expect(spawnEnv.CODEX_API_KEY).toBe("test");
      expect(spawnEnv.CODEX_INTERNAL_ORIGINATOR_OVERRIDE).toBeDefined();
    } finally {
      delete process.env.CODEX_ENV_SHOULD_NOT_LEAK;
      restore();
      await close();
    }
  });

  it("passes additionalDirectories as repeated flags", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Additional directories applied", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });

    const { args: spawnArgs, restore } = codexExecSpy();

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread({
        additionalDirectories: ["../backend", "/tmp/shared"],
      });
      await thread.run("test additional dirs");

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      if (!commandArgs) {
        throw new Error("Command args missing");
      }

      // Find the --add-dir flags
      const addDirArgs: string[] = [];
      for (let i = 0; i < commandArgs.length; i += 1) {
        if (commandArgs[i] === "--add-dir") {
          addDirArgs.push(commandArgs[i + 1] ?? "");
        }
      }
      expect(addDirArgs).toEqual(["../backend", "/tmp/shared"]);
    } finally {
      restore();
      await close();
    }
  });

  it("writes output schema to a temporary file and forwards it", async () => {
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

    const { args: spawnArgs, restore } = codexExecSpy();

    const schema = {
      type: "object",
      properties: {
        answer: { type: "string" },
      },
      required: ["answer"],
      additionalProperties: false,
    } as const;

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread();
      await thread.run("structured", { outputSchema: schema });

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

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      const schemaFlagIndex = commandArgs!.indexOf("--output-schema");
      expect(schemaFlagIndex).toBeGreaterThan(-1);
      const schemaPath = commandArgs![schemaFlagIndex + 1];
      expect(typeof schemaPath).toBe("string");
      if (typeof schemaPath !== "string") {
        throw new Error("--output-schema flag missing path argument");
      }
      expect(fs.existsSync(schemaPath)).toBe(false);
    } finally {
      restore();
      await close();
    }
  });
  it("combines structured text input segments", async () => {
    const { url, close, requests } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Combined input applied", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread();
      await thread.run([
        { type: "text", text: "Describe file changes" },
        { type: "text", text: "Focus on impacted tests" },
      ]);

      const payload = requests[0];
      expect(payload).toBeDefined();
      const lastUser = payload!.json.input.at(-1);
      expect(lastUser?.content?.[0]?.text).toBe("Describe file changes\n\nFocus on impacted tests");
    } finally {
      await close();
    }
  });
  it("forwards images to exec", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Images applied", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });

    const { args: spawnArgs, restore } = codexExecSpy();
    const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), "codex-images-"));
    const imagesDirectoryEntries: [string, string] = [
      path.join(tempDir, "first.png"),
      path.join(tempDir, "second.jpg"),
    ];
    imagesDirectoryEntries.forEach((image, index) => {
      fs.writeFileSync(image, `image-${index}`);
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread();
      await thread.run([
        { type: "text", text: "describe the images" },
        { type: "local_image", path: imagesDirectoryEntries[0] },
        { type: "local_image", path: imagesDirectoryEntries[1] },
      ]);

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      const forwardedImages: string[] = [];
      for (let i = 0; i < commandArgs!.length; i += 1) {
        if (commandArgs![i] === "--image") {
          forwardedImages.push(commandArgs![i + 1] ?? "");
        }
      }
      expect(forwardedImages).toEqual(imagesDirectoryEntries);
    } finally {
      fs.rmSync(tempDir, { recursive: true, force: true });
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

  it("sets the codex sdk originator header", async () => {
    const { url, close, requests } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [sse(responseStarted(), assistantMessage("Hi!"), responseCompleted())],
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });

      const thread = client.startThread();
      await thread.run("Hello, originator!");

      expect(requests.length).toBeGreaterThan(0);
      const originatorHeader = requests[0]!.headers["originator"];
      if (Array.isArray(originatorHeader)) {
        expect(originatorHeader).toContain("codex_sdk_ts");
      } else {
        expect(originatorHeader).toBe("codex_sdk_ts");
      }
    } finally {
      await close();
    }
  });
  it("throws ThreadRunError on turn failures", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: (function* (): Generator<SseResponseBody> {
        yield sse(responseStarted("response_1"));
        while (true) {
          yield sse(responseFailed("rate limit exceeded"));
        }
      })(),
    });

    try {
      const client = new Codex({ codexPathOverride: codexExecPath, baseUrl: url, apiKey: "test" });
      const thread = client.startThread();
      await expect(thread.run("fail")).rejects.toThrow("stream disconnected before completion:");
    } finally {
      await close();
    }
  }, 10000); // TODO(pakrym): remove timeout
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
