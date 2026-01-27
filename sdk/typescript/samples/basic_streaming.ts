#!/usr/bin/env -S NODE_NO_WARNINGS=1 pnpm ts-node-esm --files

import { createInterface } from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";

import { Codex } from "@openai/codex-sdk";
import type { ThreadEvent, ThreadItem } from "@openai/codex-sdk";
import { codexPathOverride } from "./helpers.ts";

const codex = new Codex({ codexPathOverride: codexPathOverride() });
const thread = codex.startThread();
const rl = createInterface({ input, output });

let running = true;

/* Graceful shutdown */
process.on("SIGINT", () => {
  console.log("\nExiting Codex CLI...");
  running = false;
  rl.close();
  process.exit(0);
});

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

    default:
      // Future-proof: log unknown item types for debugging
      console.debug(`Unhandled completed item type: ${(item as any).type}`);
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

    default:
      console.debug(`Unhandled updated item type: ${(item as any).type}`);
  }
};

const handleEvent = (event: ThreadEvent): void => {
  switch (event.type) {
    case "item.completed":
      handleItemCompleted(event.item);
      break;

    case "item.updated":
    case "item.started":
      if (event.item) handleItemUpdated(event.item);
      break;

    case "turn.completed":
      console.log(
        `Used ${event.usage.input_tokens} input tokens, ${event.usage.cached_input_tokens} cached input tokens, ${event.usage.output_tokens} output tokens.`,
      );
      break;

    case "turn.failed":
      console.error(`Turn failed: ${event.error.message}`);
      break;

    default:
      console.debug(`Unhandled event type: ${(event as any).type}`);
  }
};

const main = async (): Promise<void> => {
  try {
    while (running) {
      const inputText = await rl.question("You > ");
      const trimmed = inputText.trim();

      if (trimmed.length === 0) continue;

      try {
        const { events } = await thread.runStreamed(trimmed);

        for await (const event of events) {
          handleEvent(event);
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err);
        console.error(`Turn error: ${message}`);
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
