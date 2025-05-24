import { Box, Text, useInput } from "ink";
import React from "react";

type ServerStatus = {
  name: string;
  connected: boolean;
};

type Props = {
  serversStatus: Array<ServerStatus>;
  onExit: () => void;
};

export default function MCPOverlay({
  serversStatus,
  onExit,
}: Props): React.ReactElement {
  // Handle keyboard input
  useInput((input, key) => {
    if (input === "q" || input === "Q" || key.escape) {
      onExit();
    }
  });

  if (serversStatus.length === 0) {
    return (
      <Box
        flexDirection="column"
        padding={1}
        borderStyle="round"
        borderColor="gray"
      >
        <Text bold>MCP Servers</Text>
        <Text>No MCP servers configured.</Text>
        <Text color="gray" dimColor>
          Press ESC or q to go back
        </Text>
      </Box>
    );
  }

  return (
    <Box
      flexDirection="column"
      padding={1}
      borderStyle="round"
      borderColor="gray"
    >
      <Text bold>MCP Servers</Text>

      <Box flexDirection="column" marginY={1}>
        {serversStatus.map((server) => (
          <Box key={server.name}>
            <Text>
              <Text color={server.connected ? "green" : "red"}>●</Text>{" "}
              <Text bold>{server.name}</Text>
              <Text> - </Text>
              <Text color={server.connected ? "green" : "red"}>
                {server.connected ? "Connected" : "Disconnected"}
              </Text>
            </Text>
          </Box>
        ))}
      </Box>

      <Text color="gray" dimColor>
        Press ESC or q to go back
      </Text>
    </Box>
  );
}
