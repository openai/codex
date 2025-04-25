import type { TerminalHeaderProps } from "./terminal-header.js";
import type { GroupedResponseItem } from "./use-message-grouping.js";
import type { ResponseItem } from "openai/resources/responses/responses.mjs";

import TerminalChatResponseItem from "./terminal-chat-response-item.js";
import TerminalHeader from "./terminal-header.js";
import { Box, Static, Text } from "ink";
import React, { useMemo } from "react";

// A batch entry can either be a standalone response item or a grouped set of
// items (e.g. auto‑approved tool‑call batches) that should be rendered
// together.
type BatchEntry = { item?: ResponseItem; group?: GroupedResponseItem };
type MessageHistoryProps = {
  batch: Array<BatchEntry>;
  groupCounts: Record<string, number>;
  items: Array<ResponseItem>;
  userMsgCount: number;
  confirmationPrompt: React.ReactNode;
  loading: boolean;
  thinkingSeconds: number;
  headerProps: TerminalHeaderProps;
  fullStdout: boolean;
};

const MessageHistory: React.FC<MessageHistoryProps> = ({
  batch,
  headerProps,
  loading,
  thinkingSeconds,
  fullStdout,
}) => {
  const [messages, debug] = useMemo(
    () => [batch.map(({ item }) => item!), process.env["DEBUG"]],
    [batch],
  );

  return (
    <Box flexDirection="column">
      {loading && debug && (
        <Box marginTop={1}>
          <Text color="yellow">{`(${thinkingSeconds}s)`}</Text>
        </Box>
      )}
      <Static items={["header", ...messages]}>
        {(item, index) => {
          if (item === "header") {
            return <TerminalHeader key="header" {...headerProps} />;
          }

          // After the guard above `item` can only be a ResponseItem.
          const message = item as ResponseItem;
          return (
            <Box
              key={`${message.id}-${index}`}
              flexDirection="column"
              marginLeft={
                message.type === "message" && message.role === "user" ? 0 : 4
              }
              marginTop={
                message.type === "message" && message.role === "user" ? 0 : 1
              }
            >
              <TerminalChatResponseItem
                item={message}
                fullStdout={fullStdout}
              />
            </Box>
          );
        }}
      </Static>
    </Box>
  );
};

export default React.memo(MessageHistory);
