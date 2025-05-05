import type { OverlayModeType } from "./terminal-chat.js";
import type { TerminalHeaderProps } from "./terminal-header.js";
import type { GroupedResponseItem } from "./use-message-grouping.js";
import type { ResponseItem } from "openai/resources/responses/responses.mjs";

import TerminalChatResponseItem from "./terminal-chat-response-item.js";
import TerminalHeader from "./terminal-header.js";
import { log } from "../../utils/logger/log";
import { useMcpManager } from "../../utils/mcp/index.js";
import { Box, Static } from "ink";
import React, { useEffect, useMemo, useState } from "react";

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
};

const TerminalMessageHistory: React.FC<TerminalMessageHistoryProps> = ({
  batch,
  headerProps,
  // `loading` and `thinkingSeconds` handled by input component now.
  loading: _loading,
  thinkingSeconds: _thinkingSeconds,
  fullStdout,
  setOverlayMode,
}) => {
  // Flatten batch entries to response items.
  const messages = useMemo(() => batch.map(({ item }) => item!), [batch]);

  // Get MCP stats for status checking
  const { stats } = useMcpManager();

  // State to track whether header should be rendered statically
  const [renderHeaderStatic, setRenderHeaderStatic] = useState(false);

  // Check MCP status to determine if we should render statically
  useEffect(() => {
    if (
      !renderHeaderStatic &&
      (stats.status === "connected" || stats.status === "error")
    ) {
      log(
        `[Terminal History] MCP status finalized: ${stats.status}, switching to static header`,
      );
      setRenderHeaderStatic(true);
    }
  }, [stats.status, renderHeaderStatic]);

  return (
    <Box flexDirection="column">
      {/* Conditionally render the header outside of Static if not in final state */}
      {!renderHeaderStatic && (
        <TerminalHeader key="dynamic-header" {...headerProps} />
      )}

      {/* The dedicated thinking indicator in the input area now displays the
          elapsed time, so we no longer render a separate counter here. */}
      <Static items={renderHeaderStatic ? ["header", ...messages] : messages}>
        {(item, index) => {
          if (renderHeaderStatic && item === "header") {
            return <TerminalHeader key="static-header" {...headerProps} />;
          }

          // Get the message item - at this point we know it's a ResponseItem
          const message = item as ResponseItem;

          // Suppress empty reasoning updates (i.e. items with an empty summary).
          const msg = message as unknown as { summary?: Array<unknown> };
          if (msg.summary?.length === 0) {
            return null;
          }
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
                setOverlayMode={setOverlayMode}
              />
            </Box>
          );
        }}
      </Static>
    </Box>
  );
};

export default React.memo(TerminalMessageHistory);
