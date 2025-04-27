import type { ResponseItem } from "openai/resources/responses/responses.mjs";

import { Box, Text, useInput } from "ink";
import React, { useMemo, useState } from "react";

type Props = {
  items: Array<ResponseItem>;
  onExit: () => void;
};

type Mode = "commands" | "files";

export default function HistoryOverlay({ items, onExit }: Props): JSX.Element {
  const [mode, setMode] = useState<Mode>("commands");
  const [cursor, setCursor] = useState(0);

  const { commands, files } = useMemo(
    () => formatHistoryForDisplay(items),
    [items],
  );

  const list = mode === "commands" ? commands : files;

  useInput((input, key) => {
    if (key.escape) {
      onExit();
      return;
    }

    if (input === "c") {
      setMode("commands");
      setCursor(0);
      return;
    }
    if (input === "f") {
      setMode("files");
      setCursor(0);
      return;
    }

    if (key.downArrow || input === "j") {
      setCursor((c) => Math.min(list.length - 1, c + 1));
    } else if (key.upArrow || input === "k") {
      setCursor((c) => Math.max(0, c - 1));
    } else if (key.pageDown) {
      setCursor((c) => Math.min(list.length - 1, c + 10));
    } else if (key.pageUp) {
      setCursor((c) => Math.max(0, c - 10));
    } else if (input === "g") {
      setCursor(0);
    } else if (input === "G") {
      setCursor(list.length - 1);
    }
  });

  const rows = process.stdout.rows || 24;
  const headerRows = 2;
  const footerRows = 1;
  const maxVisible = Math.max(4, rows - headerRows - footerRows);

  const firstVisible = Math.min(
    Math.max(0, cursor - Math.floor(maxVisible / 2)),
    Math.max(0, list.length - maxVisible),
  );
  const visible = list.slice(firstVisible, firstVisible + maxVisible);

  return (
    <Box
      flexDirection="column"
      borderStyle="round"
      borderColor="gray"
      width={100}
    >
      <Box paddingX={1}>
        <Text bold>
          {mode === "commands" ? "Commands run" : "Files touched"} (
          {list.length})
        </Text>
      </Box>
      <Box flexDirection="column" paddingX={1}>
        {visible.map((txt, idx) => {
          const absIdx = firstVisible + idx;
          const selected = absIdx === cursor;
          return (
            <Text key={absIdx} color={selected ? "cyan" : undefined}>
              {selected ? "› " : "  "}
              {txt}
            </Text>
          );
        })}
      </Box>
      <Box paddingX={1}>
        <Text dimColor>
          esc Close ↑↓ Scroll PgUp/PgDn g/G First/Last c Commands f Files
        </Text>
      </Box>
    </Box>
  );
}

function formatHistoryForDisplay(items: Array<ResponseItem>): {
  commands: Array<string>;
  files: Array<string>;
} {
  const commands: Array<string> = [];
  const filesSet = new Set<string>();

  for (const item of items) {
    const userPrompt = processUserMessage(item);
    if (userPrompt) {
      commands.push(userPrompt);
      continue;
    }

    // ------------------------------------------------------------------
    // We are interested in tool calls which – for the OpenAI client – are
    // represented as `function_call` response items. Skip everything else.
    if (item.type !== "function_call") {
      continue;
    }

    const { name: toolName, arguments: argsString } = item as unknown as {
      name: unknown;
      arguments: unknown;
    };

    if (typeof argsString !== "string") {
      // Malformed – still record the tool name to give users maximal context.
      if (typeof toolName === "string" && toolName.length > 0) {
        commands.push(toolName);
      }
      continue;
    }

    // Best‑effort attempt to parse the JSON arguments. We never throw on parse
    // failure – the history view must be resilient to bad data.
    let argsJson: unknown = undefined;
    try {
      argsJson = JSON.parse(argsString);
    } catch {
      argsJson = undefined;
    }

    // 1) Shell / exec‑like tool calls expose a `cmd` or `command` property
    //    that is an array of strings. These are rendered as the joined command
    //    line for familiarity with traditional shells.
    const argsObj = argsJson as Record<string, unknown> | undefined;
    const cmdArray: Array<string> | undefined = Array.isArray(argsObj?.["cmd"])
      ? (argsObj!["cmd"] as Array<string>)
      : Array.isArray(argsObj?.["command"])
        ? (argsObj!["command"] as Array<string>)
        : undefined;

    if (cmdArray && cmdArray.length > 0) {
      commands.push(processCommandArray(cmdArray, filesSet));
      continue; // We processed this as a command; no need to treat as generic tool call.
    }

    // 2) Non‑exec tool calls – we fall back to recording the tool name plus a
    //    short argument representation to give users an idea of what
    //    happened.
    if (typeof toolName === "string" && toolName.length > 0) {
      commands.push(processNonExecTool(toolName, argsJson, filesSet));
    }
  }

  return { commands, files: Array.from(filesSet) };
}

function processUserMessage(item: ResponseItem): string | null {
  if (
    item.type === "message" &&
    (item as unknown as { role?: string }).role === "user"
  ) {
    // TODO: We're ignoring images/files here.
    const parts =
      (item as unknown as { content?: Array<unknown> }).content ?? [];
    const texts: Array<string> = [];
    if (Array.isArray(parts)) {
      for (const part of parts) {
        if (part && typeof part === "object" && "text" in part) {
          const t = (part as unknown as { text?: string }).text;
          if (typeof t === "string" && t.length > 0) {
            texts.push(t);
          }
        }
      }
    }

    if (texts.length > 0) {
      const fullPrompt = texts.join(" ");
      // Truncate very long prompts so the history view stays legible.
      return fullPrompt.length > 120
        ? `> ${fullPrompt.slice(0, 117)}…`
        : `> ${fullPrompt}`;
    }
  }
  return null;
}

function processCommandArray(
  cmdArray: Array<string>,
  filesSet: Set<string>,
): string {
  const cmd = cmdArray.join(" ");

  // Heuristic for file paths in command args
  for (const part of cmdArray) {
    if (!part.startsWith("-") && part.includes("/")) {
      filesSet.add(part);
    }
  }

  // Special‑case apply_patch so we can extract the list of modified files
  if (cmdArray[0] === "apply_patch" || cmdArray.includes("apply_patch")) {
    const patchTextMaybe = cmdArray.find((s) => s.includes("*** Begin Patch"));
    if (typeof patchTextMaybe === "string") {
      const lines = patchTextMaybe.split("\n");
      for (const line of lines) {
        const m = line.match(/^[-+]{3} [ab]\/(.+)$/);
        if (m && m[1]) {
          filesSet.add(m[1]);
        }
      }
    }
  }

  return cmd;
}

function processNonExecTool(
  toolName: string,
  argsJson: unknown,
  filesSet: Set<string>,
): string {
  let summary = toolName;

  if (argsJson && typeof argsJson === "object") {
    // Extract a few common argument keys to make the summary more useful
    // without being overly verbose.
    const interestingKeys = ["path", "file", "filepath", "filename", "pattern"];
    for (const key of interestingKeys) {
      const val = (argsJson as Record<string, unknown>)[key];
      if (typeof val === "string") {
        summary += ` ${val}`;
        if (val.includes("/")) {
          filesSet.add(val);
        }
        break;
      }
    }
  }

  return summary;
}
