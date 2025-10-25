#!/usr/bin/env -S NODE_NO_WARNINGS=1 pnpm ts-node-esm --files

import { createInterface } from "node:readline/promises";
import { stdin as input, stdout as output } from "node:process";

import { Codex } from "@openai/codex-sdk";
import type { McpServerDetails, McpServerSummary } from "@openai/codex-sdk";

import { codexPathOverride } from "./helpers.ts";

const codex = new Codex({ codexPathOverride: codexPathOverride() });
const manager = codex.mcp;
const rl = createInterface({ input, output });
type TempOverride = "enable-once" | "disable-once";
const tempOverrides = new Map<string, TempOverride>();

const formatTransport = (server: McpServerSummary | McpServerDetails): string => {
  if (server.transport.type === "stdio") {
    const args = server.transport.args?.length ? ` ${server.transport.args.join(" ")}` : "";
    return `${server.transport.command}${args}`;
  }
  return server.transport.url;
};

const describeServer = (server: McpServerSummary | McpServerDetails): string => {
  const status = server.enabled ? "enabled" : "disabled";
  return `${server.name} - ${status} (${formatTransport(server)})`;
};

const ask = async (question: string): Promise<string | null> => {
  try {
    return await rl.question(question);
  } catch (error) {
    if (error instanceof Error && error.message === "readline was closed") {
      return null;
    }
    throw error;
  }
};

const refreshServerDetails = async (name: string): Promise<McpServerDetails | null> => {
  try {
    return await manager.get(name);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(`Failed to read updated configuration: ${message}`);
    return null;
  }
};

const describeServerWithTag = (
  server: McpServerSummary | McpServerDetails,
  tempTag?: string,
): string => {
  const base = describeServer(server);
  return tempTag ? `${tempTag} ${base}` : base;
};

const tempTagFor = (server: McpServerSummary | McpServerDetails): string | undefined => {
  const override = tempOverrides.get(server.name);
  if (!override) {
    return undefined;
  }
  if (override === "enable-once") {
    if (server.enabled) {
      tempOverrides.delete(server.name);
      return undefined;
    }
    return "(temp enable)";
  }
  if (!server.enabled) {
    tempOverrides.delete(server.name);
    return undefined;
  }
  return "(temp disable)";
};

const setTempOverride = (server: McpServerSummary, action: TempOverride): void => {
  if (action === "enable-once") {
    if (server.enabled) {
      tempOverrides.delete(server.name);
    } else {
      tempOverrides.set(server.name, action);
    }
    return;
  }
  if (!server.enabled) {
    tempOverrides.delete(server.name);
  } else {
    tempOverrides.set(server.name, action);
  }
};

const printServerSnapshot = (
  server: McpServerSummary | McpServerDetails,
  tempTag?: string,
): void => {
  const resolvedTag = tempTag ?? tempTagFor(server);
  console.log();
  console.log(describeServerWithTag(server, resolvedTag));
  if ("enabled_tools" in server) {
    const enabledList =
      server.enabled_tools && server.enabled_tools.length ? server.enabled_tools.join(", ") : "all";
    console.log(`Enabled tools: ${enabledList}`);
    const disabledList =
      server.disabled_tools && server.disabled_tools.length
        ? server.disabled_tools.join(", ")
        : "none";
    console.log(`Disabled tools: ${disabledList}`);
    if (!server.enabled) {
      console.log("Server is currently disabled.");
    }
  }
};

type Action = "enable-once" | "disable-once" | "enable" | "disable";

const promptAction = async (): Promise<Action | "quit" | null> => {
  while (true) {
    const answer = await ask(
      "\nChoose an action:\n" +
        "  1) Temporarily enable (enable once)\n" +
        "  2) Temporarily disable (disable once)\n" +
        "  3) Persistently enable\n" +
        "  4) Persistently disable\n" +
        "Enter 1-4, or q to quit: ",
    );

    if (answer === null) {
      return null;
    }

    const trimmed = answer.trim().toLowerCase();
    switch (trimmed) {
      case "1":
      case "enable once":
      case "enable-once":
        return "enable-once";
      case "2":
      case "disable once":
      case "disable-once":
        return "disable-once";
      case "3":
      case "persist enable":
      case "persist-enable":
        return "enable";
      case "4":
      case "persist disable":
      case "persist-disable":
        return "disable";
      case "q":
      case "quit":
      case "exit":
        return "quit";
      default:
        console.log("Option not recognised. Try again.");
    }
  }
};

const promptServerSelection = async (
  servers: McpServerSummary[],
): Promise<McpServerSummary | undefined | null> => {
  while (true) {
    const answer = await ask("Select a server by number (Enter cancels, q to quit): ");
    if (answer === null) {
      return null;
    }

    const trimmed = answer.trim().toLowerCase();
    if (trimmed === "") {
      return undefined;
    }
    if (trimmed === "q" || trimmed === "quit" || trimmed === "exit") {
      return null;
    }
    if (!/^\d+$/.test(trimmed)) {
      console.log("Selection not recognised. Try again.");
      continue;
    }

    const index = Number.parseInt(trimmed, 10);
    if (index < 1 || index > servers.length) {
      console.log("Selection not recognised. Try again.");
      continue;
    }

    return servers[index - 1];
  }
};

const interactiveLoop = async (): Promise<void> => {
  console.log("Codex MCP manager. Ctrl+C or choose q to quit.");

  while (true) {
    const servers = await manager.list();
    if (!servers.length) {
      console.log(
        "No MCP servers configured. Use `codex mcp add <name> ...` to register one before running this sample.",
      );
      return;
    }

    console.log("\nConfigured MCP servers:");
    servers.forEach((server, index) => {
      const tag = tempTagFor(server);
      console.log(`${index + 1}. ${describeServerWithTag(server, tag)}`);
    });

    const action = await promptAction();
    if (action === null) {
      console.log("Input closed. Exiting.");
      return;
    }
    if (action === "quit") {
      return;
    }

    const server = await promptServerSelection(servers);
    if (server === null) {
      return;
    }
    if (!server) {
      console.log("Selection cancelled. Refreshing list.");
      continue;
    }

    switch (action) {
      case "enable-once":
        manager.enableOnce(server.name);
        setTempOverride(server, "enable-once");
        console.log(`Temporarily enabled ${server.name} for this Codex instance.`);
        printServerSnapshot(server);
        continue;
      case "disable-once":
        manager.disableOnce(server.name);
        setTempOverride(server, "disable-once");
        console.log(`Temporarily disabled ${server.name} for this Codex instance.`);
        printServerSnapshot(server);
        continue;
      case "enable":
        tempOverrides.delete(server.name);
        await manager.enable(server.name);
        console.log(`Saved ${server.name} as enabled in your Codex configuration.`);
        break;
      case "disable":
        tempOverrides.delete(server.name);
        await manager.disable(server.name);
        console.log(`Saved ${server.name} as disabled in your Codex configuration.`);
        break;
    }

    const details = await refreshServerDetails(server.name);
    printServerSnapshot(details ?? server);
  }
};

const main = async (): Promise<void> => {
  try {
    await interactiveLoop();
  } finally {
    rl.close();
  }
};

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`Failed: ${message}`);
  process.exit(1);
});
