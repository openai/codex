import type { OverlayModeType } from "./terminal-chat.js";
import type { TerminalHeaderProps } from "./terminal-header.js";
import type { GroupedResponseItem } from "./use-message-grouping.js";
import type {
  ResponseItem,
  ResponseFunctionToolCall,
} from "openai/resources/responses/responses.mjs";
import type { FileOpenerScheme } from "src/utils/config.js";

import TerminalChatResponseItem from "./terminal-chat-response-item.js";
import TerminalHeader from "./terminal-header.js";
import { formatCommandForDisplay } from "../../format-command.js";
import {
  classifyRunningTitle,
  classifySuccessTitle,
  classifyFailureTitle,
  extractBeforeFirstUnquotedPipe,
} from "./tools/classify.js";
import { CommandStatus } from "./tools/command-status.js";
import { parseToolCall, parseToolCallOutput } from "../../utils/parsers.js";
import { Box, Text } from "ink";
import React, { useMemo } from "react";

// A batch entry can either be a standalone response item or a grouped set of
// items (e.g. auto‑approved tool‑call batches) that should be rendered
// together.
type BatchEntry = { item?: ResponseItem; group?: GroupedResponseItem };
type TerminalMessageHistoryProps = {
  batch: Array<BatchEntry>;
  groupCounts: Record<string, number>;
  items: Array<ResponseItem>;
  userMsgCount: number;
  confirmationPrompt: React.ReactNode;
  loading: boolean;
  thinkingSeconds: number;
  headerProps: TerminalHeaderProps;
  fullStdout: boolean;
  setOverlayMode: React.Dispatch<React.SetStateAction<OverlayModeType>>;
  fileOpener: FileOpenerScheme | undefined;
};

const TerminalMessageHistory: React.FC<TerminalMessageHistoryProps> = ({
  batch,
  headerProps,
  // `loading` and `thinkingSeconds` handled by input component now.
  loading: _loading,
  thinkingSeconds: _thinkingSeconds,
  fullStdout,
  setOverlayMode,
  fileOpener,
}) => {
  // Build a dynamic view that collapses command tool-calls with their outputs
  // into a single renderable entry that transitions from "⏳ Running" to
  // "⚡ Ran" (and other states) by swapping the rendered item.
  const sourceItems: Array<ResponseItem> = useMemo(
    () => batch.map(({ item }) => item!).filter(Boolean) as Array<ResponseItem>,
    [batch],
  );

  type CommandState = "running" | "success" | "failure" | "aborted";
  type DisplayItem =
    | { kind: "response"; item: ResponseItem }
    | {
        kind: "command";
        key: string;
        commandText: string;
        workdir?: string;
        state: CommandState;
        outputText?: string;
        exitCode?: number;
        customSuccessTitle?: string;
        customRunningTitle?: string;
      };

  const displayItems: Array<DisplayItem> = useMemo(() => {
    const outputsByCallId = new Map<
      string,
      { output: string; exit?: number }
    >();
    const consumedOutputs = new Set<string>();

    for (const it of sourceItems) {
      // Include Outputs from function calls (local shell output uses SDK‑unknown type)
      if (
        (it as { type?: string }).type === "function_call_output" ||
        (it as { type?: string }).type ===
          ("local_shell_call_output" as unknown as string)
      ) {
        const callId = (it as unknown as { call_id?: string }).call_id;
        if (!callId) {
          continue;
        }
        const outputRaw = (it as { output?: string }).output ?? "{}";
        const { output, metadata } = parseToolCallOutput(outputRaw);
        outputsByCallId.set(callId, {
          output,
          exit:
            typeof metadata?.exit_code === "number"
              ? metadata.exit_code
              : undefined,
        });
      }
    }

    const result: Array<DisplayItem> = [];
    for (const it of sourceItems) {
      // Collapse tool calls with their outputs (local shell call uses SDK‑unknown type)
      if (
        (it as { type?: string }).type === "function_call" ||
        (it as { type?: string }).type ===
          ("local_shell_call" as unknown as string)
      ) {
        // Compute stable call id
        const anyIt = it as unknown as {
          call_id?: string;
          id?: string;
          action?: { command?: Array<string>; working_directory?: string };
        };
        const callId: string | undefined = anyIt.call_id ?? anyIt.id;

        let commandText = "";
        let workdir: string | undefined = undefined;
        if (it.type === "function_call") {
          const details = parseToolCall(
            it as unknown as ResponseFunctionToolCall,
          );
          commandText = details?.cmdReadableText ?? commandText;
          workdir = details?.workdir;
        } else {
          // local_shell_call
          const cmdArr: Array<string> = Array.isArray(anyIt.action?.command)
            ? (anyIt.action?.command as Array<string>)
            : [];
          commandText = formatCommandForDisplay(cmdArr);
          workdir = anyIt.action?.working_directory;
        }

        const out = callId ? outputsByCallId.get(callId) : undefined;
        if (callId && out) {
          consumedOutputs.add(callId);
        }

        const exit = out?.exit;
        const isAborted = out?.output === "aborted";
        const state: CommandState = isAborted
          ? "aborted"
          : typeof exit === "number" && exit !== 0
            ? "failure"
            : out
              ? "success"
              : "running";

        // Optional human-readable titles for running/success
        const customSuccessTitle = classifySuccessTitle(
          commandText,
          out?.output,
        );
        const customRunningTitle = classifyRunningTitle(commandText);

        result.push({
          kind: "command",
          key: callId ?? `${it.id}`,
          commandText,
          workdir,
          state,
          outputText: out?.output,
          exitCode: exit,
          customSuccessTitle,
          customRunningTitle,
        });
        continue;
      }

      // Skip standalone outputs that have already been merged into their calls
      if (
        (it as { type?: string }).type === "function_call_output" ||
        (it as { type?: string }).type ===
          ("local_shell_call_output" as unknown as string)
      ) {
        const callId = (it as unknown as { call_id?: string }).call_id;
        if (callId && consumedOutputs.has(callId)) {
          continue;
        }
      }

      // Suppress all reasoning items from history – thinking indicator shows summaries.
      if ((it as { type?: string }).type === "reasoning") {
        continue;
      }

      // Default: render original item
      result.push({ kind: "response", item: it });
    }
    return result;
  }, [sourceItems]);

  // Group consecutive successful "📖 Read <file>" items
  type ReadGroupItem = {
    kind: "read_group";
    key: string;
    files: Array<string>;
  };
  type ListGroupItem = { kind: "list_group"; key: string; total: number };
  const renderItems: Array<DisplayItem | ReadGroupItem | ListGroupItem> =
    useMemo(() => {
      const items: Array<DisplayItem | ReadGroupItem | ListGroupItem> = [];
      for (let i = 0; i < displayItems.length; i += 1) {
        const d = displayItems[i]!;
        if (
          d.kind === "command" &&
          d.state === "success" &&
          typeof d.customSuccessTitle === "string" &&
          /^(?:●|📖)\s+Read\s+/.test(d.customSuccessTitle)
        ) {
          const files: Array<string> = [];
          const seen = new Set<string>();
          let j = i;
          while (
            j < displayItems.length &&
            displayItems[j]!.kind === "command" &&
            (displayItems[j] as typeof d).state === "success" &&
            typeof (displayItems[j] as typeof d).customSuccessTitle ===
              "string" &&
            /^(?:●|📖)\s+Read\s+/.test(
              (displayItems[j] as typeof d).customSuccessTitle as string,
            )
          ) {
            const title = (displayItems[j] as typeof d).customSuccessTitle!;
            const fname = title.replace(/^(?:●|📖)\s+Read\s+/, "");
            if (!seen.has(fname)) {
              files.push(fname);
              seen.add(fname);
            }
            j += 1;
          }
          items.push({ kind: "read_group", key: d.key, files });
          i = j - 1;
        } else if (
          d.kind === "command" &&
          d.state === "success" &&
          typeof d.customSuccessTitle === "string" &&
          /^📁 Listed\s+\d+\s+paths/.test(d.customSuccessTitle)
        ) {
          // Sum consecutive Listed counts into a single running total
          let total = 0;
          let j = i;
          while (
            j < displayItems.length &&
            displayItems[j]!.kind === "command" &&
            (displayItems[j] as typeof d).state === "success" &&
            typeof (displayItems[j] as typeof d).customSuccessTitle ===
              "string" &&
            /^📁 Listed\s+\d+\s+paths/.test(
              (displayItems[j] as typeof d).customSuccessTitle as string,
            )
          ) {
            const title = (displayItems[j] as typeof d)
              .customSuccessTitle as string;
            const m = title.match(/📁 Listed\s+(\d+)\s+paths/);
            const n = m ? Number(m[1]) : 0;
            total += n;
            j += 1;
          }
          items.push({ kind: "list_group", key: d.key, total });
          i = j - 1;
        } else {
          items.push(d);
        }
      }
      return items;
    }, [displayItems]);

  return (
    <Box flexDirection="column">
      <TerminalHeader {...headerProps} />
      {renderItems.map((d, index) => {
        if (d.kind === "response") {
          const message = d.item;
          return (
            <Box
              key={`${message.id}-${index}`}
              flexDirection="column"
              marginLeft={0}
              marginTop={index === 0 ? 0 : 1}
              marginBottom={0}
            >
              <TerminalChatResponseItem
                item={message}
                fullStdout={fullStdout}
                setOverlayMode={setOverlayMode}
                fileOpener={fileOpener}
              />
            </Box>
          );
        }

        const prev = renderItems[index - 1];
        const prevIsUserMessage =
          prev &&
          (prev as any).kind === "response" &&
          (prev as any).item?.type === "message" &&
          (prev as any).item?.role === "user";
        if (d.kind === "list_group") {
          return (
            <Box
              key={`${d.key}-${index}`}
              flexDirection="column"
              marginTop={index === 0 ? 0 : 1}
            >
              <CommandStatus title={`● Listed ${d.total} paths`} />
            </Box>
          );
        }

        // Render grouped reads
        if (d.kind === "read_group") {
          const n = d.files.length;
          const header = `● Read ${n} ${n === 1 ? "file" : "files"}`;
          return (
            <Box
              key={`${d.key}-${index}`}
              flexDirection="column"
              marginTop={index === 0 ? 0 : 1}
            >
              <CommandStatus title={header} />
              {d.files.map((f, idx) => (
                <Box
                  key={`${d.key}-${index}-${idx}`}
                  flexDirection="row"
                  marginLeft={2}
                >
                  <Text dimColor>{idx === 0 ? "⎿ " : "  "}</Text>
                  <Text dimColor>{f}</Text>
                </Box>
              ))}
            </Box>
          );
        }

        // Render combined command item with state swapping.
        let title: string;
        if (d.state === "running") {
          title = d.customRunningTitle ?? `● Running ${d.commandText}`;
        } else if (d.state === "success") {
          title = d.customSuccessTitle ?? `● Ran ${d.commandText}`;
        } else if (d.state === "failure") {
          const custom = classifyFailureTitle(d.commandText, d.outputText);
          if (custom) {
            title = custom;
          } else {
            const beforePipe = extractBeforeFirstUnquotedPipe(d.commandText);
            const firstWord = beforePipe.trim().split(/\s+/)[0] || beforePipe;
            title = `⨯ Failed ${firstWord}`;
          }
        } else {
          title = `● Aborted ${d.commandText}`;
        }
        const outputForDisplay =
          d.state !== "running" ? d.outputText : undefined;
        return (
          <CommandStatus
            key={`${d.key}-${index}`}
            title={title}
            workdir={d.workdir}
            outputText={outputForDisplay}
            fullStdout={fullStdout}
            marginTop={index === 0 ? 0 : 1}
          />
        );
      })}
    </Box>
  );
};

export default React.memo(TerminalMessageHistory);
// (helpers moved to ./tools/*)
