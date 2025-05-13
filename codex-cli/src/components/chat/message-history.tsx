import type { TerminalHeaderProps } from "./terminal-header.js";
import type { GroupedResponseItem } from "./use-message-grouping.js";
import type { ResponseItem } from "openai/resources/responses/responses.mjs";
import type { FileOpenerScheme } from "src/utils/config.js";

import TerminalChatResponseItem from "./terminal-chat-response-item.js";
import TerminalHeader from "./terminal-header.js";
import { Box, Static } from "ink";
import React from "react";

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
  headerProps: TerminalHeaderProps;
  fileOpener: FileOpenerScheme | undefined;
};

const MessageHistory: React.FC<MessageHistoryProps> = ({
  batch,
  headerProps,
  fileOpener,
}) => {
  const messages = batch.map(({ item }) => item!);

  return (
    <Box flexDirection="column">
      {/*
       * The Static component receives a mixed array of the literal string
       * "header" plus the streamed ResponseItem objects.  After filtering out
       * the header entry we can safely treat the remaining values as
       * ResponseItem, however TypeScript cannot infer the refined type from
       * the runtime check and therefore reports property‑access errors.
       *
       * A short cast after the refinement keeps the implementation tidy while
       * preserving type‑safety.
       */}
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
              borderStyle={
                message.type === "message" && message.role === "user"
                  ? "round"
                  : undefined
              }
              borderColor={
                message.type === "message" && message.role === "user"
                  ? "gray"
                  : undefined
              }
              marginLeft={
                message.type === "message" && message.role === "user" ? 0 : 4
              }
              marginTop={
                message.type === "message" && message.role === "user" ? 0 : 1
              }
            >
              <TerminalChatResponseItem
                item={message}
                fileOpener={fileOpener}
              />
            </Box>
          );
        }}
      </Static>
    </Box>
  );
};

export default React.memo(MessageHistory);
