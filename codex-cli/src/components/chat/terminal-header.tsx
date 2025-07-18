import type { AgentLoop } from "../../utils/agent/agent-loop.js";

import { log } from "../../utils/logger/log";
import { useMcpManager } from "../../utils/mcp";
import { Box, Text } from "ink";
import path from "node:path";
import React, { useEffect } from "react";

export interface TerminalHeaderProps {
  terminalRows: number;
  version: string;
  PWD: string;
  model: string;
  provider?: string;
  approvalPolicy: string;
  colorsByPolicy: Record<string, string | undefined>;
  agent?: AgentLoop;
  initialImagePaths?: Array<string>;
  flexModeEnabled?: boolean;
}

const TerminalHeader: React.FC<TerminalHeaderProps> = ({
  terminalRows,
  version,
  PWD,
  model,
  provider = "openai",
  approvalPolicy,
  colorsByPolicy,
  agent,
  initialImagePaths,
  flexModeEnabled = false,
}) => {
  // MCP status color mapping
  const mcpStatusColor = {
    idle: "gray",
    connecting: "yellow",
    connected: "green",
    error: "red",
  };

  const { stats: mcpStats } = useMcpManager();

  // Log when MCP stats change for debugging
  useEffect(() => {
    log(`[Terminal Header] MCP stats updated: ${JSON.stringify(mcpStats)}`);
  }, [mcpStats]);

  // Generate MCP status text
  const mcpStatusText = () => {
    if (!mcpStats) {
      return "● not initialized";
    }

    const {
      status,
      connectedServers,
      totalServers,
      toolsCount,
      erroredServers,
    } = mcpStats;

    if (status === "connected") {
      return `● ${connectedServers}/${totalServers} servers, ${toolsCount} tools`;
    } else if (status === "connecting") {
      return `● connecting...`;
    } else if (status === "error") {
      return `● ${erroredServers}/${totalServers} errors`;
    } else {
      return "● not connected";
    }
  };

  return (
    <>
      {terminalRows < 10 ? (
        // Compact header for small terminal windows
        <Text>
          ● Codex v{version} - {PWD} - {model} ({provider}) -{" "}
          <Text color={colorsByPolicy[approvalPolicy]}>{approvalPolicy}</Text>
          {flexModeEnabled ? " - flex-mode" : ""} - MCP:{" "}
          <Text color={mcpStatusColor[mcpStats?.status || "idle"]}>
            {mcpStatusText()}
          </Text>
        </Text>
      ) : (
        <>
          <Box borderStyle="round" paddingX={1} width={64}>
            <Text>
              ● OpenAI <Text bold>Codex</Text>{" "}
              <Text dimColor>
                (research preview) <Text color="blueBright">v{version}</Text>
              </Text>
            </Text>
          </Box>
          <Box
            borderStyle="round"
            borderColor="gray"
            paddingX={1}
            width={64}
            flexDirection="column"
          >
            <Text>
              localhost <Text dimColor>session:</Text>{" "}
              <Text color="magentaBright" dimColor>
                {agent?.sessionId ?? "<no-session>"}
              </Text>
            </Text>
            <Text dimColor>
              <Text color="blueBright">↳</Text> workdir: <Text bold>{PWD}</Text>
            </Text>
            <Text dimColor>
              <Text color="blueBright">↳</Text> model: <Text bold>{model}</Text>
            </Text>
            <Text dimColor>
              <Text color="blueBright">↳</Text> provider:{" "}
              <Text bold>{provider}</Text>
            </Text>
            <Text dimColor>
              <Text color="blueBright">↳</Text> approval:{" "}
              <Text bold color={colorsByPolicy[approvalPolicy]}>
                {approvalPolicy}
              </Text>
            </Text>
            <Text dimColor>
              <Text color="blueBright">↳</Text> mcp:{" "}
              <Text bold color={mcpStatusColor[mcpStats?.status || "idle"]}>
                {mcpStatusText()}
              </Text>
            </Text>
            {flexModeEnabled && (
              <Text dimColor>
                <Text color="blueBright">↳</Text> flex-mode:{" "}
                <Text bold>enabled</Text>
              </Text>
            )}
            {initialImagePaths?.map((img, idx) => (
              <Text key={img ?? idx} color="gray">
                <Text color="blueBright">↳</Text> image:{" "}
                <Text bold>{path.basename(img)}</Text>
              </Text>
            ))}
          </Box>
        </>
      )}
    </>
  );
};

export default TerminalHeader;
