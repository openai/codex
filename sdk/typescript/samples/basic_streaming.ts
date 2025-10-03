#!/usr/bin/env -S NODE_NO_WARNINGS=1 pnpm ts-node-esm --files

import { createInterface } from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";

import { Codex } from "@openai/codex-sdk";
import type { ThreadEvent, ThreadItem } from "@openai/codex-sdk";
import path from "node:path";

const codexPathOverride =
  process.env.CODEX_EXECUTABLE ??
  path.join(process.cwd(), "..", "..", "codex-rs", "target", "debug", "codex");

const codex = new Codex({ codexPathOverride });
const thread = codex.startThread();
const rl = createInterface({ input, output });

const handleItemCompleted = (item: ThreadItem): void => {
  switch (item.type) {
    case "agent_message":
      console.log(`Assistant: ${item.text}`);
      break;
    case "reasoning":
      console.log(`Reasoning: ${item.text}`);
      break;
    case "command_execution": {
      const exitText = item.exit_code !== undefined ? ` Exit code ${item.exit_code}.` : "";
      console.log(`Command ${item.command} ${item.status}.${exitText}`);
      break;
    }
    case "file_change": {
      for (const change of item.changes) {
        console.log(`File ${change.kind} ${change.path}`);
      }
      break;
    }
  }
};

const handleItemUpdated = (item: ThreadItem): void => {
  switch (item.type) {
    case "todo_list": {
      console.log(`Todo:`);
      for (const todo of item.items) {
        console.log(`\t ${todo.completed ? "x" : " "} ${todo.text}`);
      }
      break;
    }
  }
};

const handleEvent = (event: ThreadEvent): void => {
  switch (event.type) {
    case "item.completed":
      handleItemCompleted(event.item);
      break;
    case "item.updated":
    case "item.started":
      handleItemUpdated(event.item);
      break;
    case "turn.completed":
      console.log(
        `Used ${event.usage.input_tokens} input tokens, ${event.usage.cached_input_tokens} cached input tokens, ${event.usage.output_tokens} output tokens.`,
      );
      break;
    case "turn.failed":
      console.error(`Turn failed: ${event.error.message}`);
      break;
  }
};

const main = async (): Promise<void> => {
  try {
    while (true) {
      const inputText = await rl.question(">");
      const trimmed = inputText.trim();
      if (trimmed.length === 0) {
        continue;
      }
      const { events } = await thread.runStreamed(inputText);
      for await (const event of events) {
        handleEvent(event);
      }
    }
  } finally {
    rl.close();
  }
};

main().catch((err) => {
  const message = err instanceof Error ? err.message : String(err);
  console.error(`Unexpected error: ${message}`);
  process.exit(1);
});
