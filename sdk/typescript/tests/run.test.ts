import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { codexExecSpy } from "./codexExecSpy";
import { describe, expect, it } from "@jest/globals";

import {
  assistantMessage,
  responseCompleted,
  responseStarted,
  sse,
  responseFailed,
  startResponsesTestProxy,
  SseResponseBody,
} from "./responsesProxy";
import { createMockClient, createTestClient } from "./testCodex";

describe("Codex", () => {
  it("returns thread events", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [sse(responseStarted(), assistantMessage("Hi!"), responseCompleted())],
    });
    const { client, cleanup } = createMockClient(url);

    try {
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
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
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
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
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
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
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
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
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
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
      const thread = client.startThread({
        modelReasoningEffort: "high",
      });
      await thread.run("apply reasoning effort");

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      expectPair(commandArgs, ["--config", 'model_reasoning_effort="high"']);
    } finally {
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
      const thread = client.startThread({
        networkAccessEnabled: true,
      });
      await thread.run("test network access");

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      expectPair(commandArgs, ["--config", "sandbox_workspace_write.network_access=true"]);
    } finally {
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
      const thread = client.startThread({
        webSearchEnabled: true,
      });
      await thread.run("test web search");

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      expectPair(commandArgs, ["--config", 'web_search="live"']);
    } finally {
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
      const thread = client.startThread({
        webSearchMode: "cached",
      });
      await thread.run("test web search mode");

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      expectPair(commandArgs, ["--config", 'web_search="cached"']);
    } finally {
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
      const thread = client.startThread({
        webSearchEnabled: false,
      });
      await thread.run("test web search disabled");

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      expectPair(commandArgs, ["--config", 'web_search="disabled"']);
    } finally {
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
      const thread = client.startThread({
        approvalPolicy: "on-request",
      });
      await thread.run("test approval policy");

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      expectPair(commandArgs, ["--config", 'approval_policy="on-request"']);
    } finally {
      cleanup();
      restore();
      await close();
    }
  });

  it("passes CodexOptions config overrides as TOML --config flags", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Config overrides applied", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });

    const { args: spawnArgs, restore } = codexExecSpy();
    const { client, cleanup } = createTestClient({
      baseUrl: url,
      apiKey: "test",
      config: {
        approval_policy: "never",
        sandbox_workspace_write: { network_access: true },
        retry_budget: 3,
        tool_rules: { allow: ["git status", "git diff"] },
      },
    });

    try {
      const thread = client.startThread();
      await thread.run("apply config overrides");

      const commandArgs = spawnArgs[0];
      expect(commandArgs).toBeDefined();
      expectPair(commandArgs, ["--config", 'approval_policy="never"']);
      expectPair(commandArgs, ["--config", "sandbox_workspace_write.network_access=true"]);
      expectPair(commandArgs, ["--config", "retry_budget=3"]);
      expectPair(commandArgs, ["--config", 'tool_rules.allow=["git status", "git diff"]']);
    } finally {
      cleanup();
      restore();
      await close();
    }
  });

  it("lets thread options override CodexOptions config overrides", async () => {
    const { url, close } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [
        sse(
          responseStarted("response_1"),
          assistantMessage("Thread overrides applied", "item_1"),
          responseCompleted("response_1"),
        ),
      ],
    });

    const { args: spawnArgs, restore } = codexExecSpy();
    const { client, cleanup } = createTestClient({
      baseUrl: url,
      apiKey: "test",
      config: { approval_policy: "never" },
    });

    try {
      const thread = client.startThread({ approvalPolicy: "on-request" });
      await thread.run("override approval policy");

      const commandArgs = spawnArgs[0];
      const approvalPolicyOverrides = collectConfigValues(commandArgs, "approval_policy");
      expect(approvalPolicyOverrides).toEqual([
        'approval_policy="never"',
        'approval_policy="on-request"',
      ]);
      expect(approvalPolicyOverrides.at(-1)).toBe('approval_policy="on-request"');
    } finally {
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
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
      cleanup();
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

    const { client, cleanup } = createMockClient(url);

    try {
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
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
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
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
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
      cleanup();
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
    const workingDirectory = fs.mkdtempSync(path.join(os.tmpdir(), "codex-working-dir-"));
    const { client, cleanup } = createTestClient({
      baseUrl: url,
      apiKey: "test",
    });

    try {
      const thread = client.startThread({
        workingDirectory,
        skipGitRepoCheck: true,
      });
      await thread.run("use custom working directory");

      const commandArgs = spawnArgs[0];
      expectPair(commandArgs, ["--cd", workingDirectory]);
    } finally {
      cleanup();
      fs.rmSync(workingDirectory, { recursive: true, force: true });
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
    const workingDirectory = fs.mkdtempSync(path.join(os.tmpdir(), "codex-working-dir-"));
    const { client, cleanup } = createTestClient({
      baseUrl: url,
      apiKey: "test",
    });

    try {
      const thread = client.startThread({
        workingDirectory,
      });
      await expect(thread.run("use custom working directory")).rejects.toThrow(
        /Not inside a trusted directory/,
      );
    } finally {
      cleanup();
      fs.rmSync(workingDirectory, { recursive: true, force: true });
      await close();
    }
  });

  it("sets the codex sdk originator header", async () => {
    const { url, close, requests } = await startResponsesTestProxy({
      statusCode: 200,
      responseBodies: [sse(responseStarted(), assistantMessage("Hi!"), responseCompleted())],
    });
    const { client, cleanup } = createMockClient(url);

    try {
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
      cleanup();
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
    const { client, cleanup } = createMockClient(url);

    try {
      const thread = client.startThread();
      await expect(thread.run("fail")).rejects.toThrow("stream disconnected before completion:");
    } finally {
      cleanup();
      await close();
    }
  }, 10000); // TODO(pakrym): remove timeout
});

/**
 * Given a list of args to `codex` and a `key`, collects all `--config`
 * overrides for that key.
 */
function collectConfigValues(args: string[] | undefined, key: string): string[] {
  if (!args) {
    throw new Error("args is undefined");
  }

  const values: string[] = [];
  for (let i = 0; i < args.length; i += 1) {
    if (args[i] !== "--config") {
      continue;
    }

    const override = args[i + 1];
    if (override?.startsWith(`${key}=`)) {
      values.push(override);
    }
  }
  return values;
}

function expectPair(args: string[] | undefined, pair: [string, string]) {
  if (!args) {
    throw new Error("args is undefined");
  }
  const index = args.findIndex((arg, i) => arg === pair[0] && args[i + 1] === pair[1]);
  if (index === -1) {
    throw new Error(`Pair ${pair[0]} ${pair[1]} not found in args`);
  }
  expect(args[index + 1]).toBe(pair[1]);
}
