import { Box, Text, useInput } from "ink";
import React from "react";

/**
 * An overlay that lists the available slash‑commands and their description.
 * The overlay is purely informational and can be dismissed with the Escape
 * key. Keeping the implementation extremely small avoids adding any new
 * dependencies or complex state handling.
 */
export default function HelpOverlay({
  onExit,
}: {
  onExit: () => void;
}): JSX.Element {
  useInput((input, key) => {
    if (input === "q" || input === "Q" || key.escape) {
      onExit();
    }
  });

  return (
    <Box
      flexDirection="column"
      borderStyle="round"
      borderColor="gray"
      padding={1}
    >
      <Text bold>Help</Text>
      <Box margin={1} flexDirection="column">
        <Text bold>Available commands</Text>
        <Box marginLeft={2} flexDirection="column">
          <Text>
            <Text bold>/help</Text> - Show this help
          </Text>
          <Text>
            <Text bold>/clear</Text> - Clear message history
          </Text>
          <Text>
            <Text bold>/diff</Text> - Show git diff of working directory
          </Text>
          <Text>
            <Text bold>/model</Text> - Change the model
          </Text>
          <Text>
            <Text bold>/approval</Text> - Change the approval mode
          </Text>
          <Text>
            <Text bold>/history</Text> - View message history
          </Text>
          <Text>
            <Text bold>/compact</Text> - Compact message history
          </Text>
          <Text>
            <Text bold>/mcp</Text> - Show MCP servers status
          </Text>
        </Box>
      </Box>

      <Box margin={1} flexDirection="column">
        <Text bold>Key bindings</Text>
        <Box marginLeft={2} flexDirection="column" gap={0}>
          <Text>
            <Text bold>Esc</Text> - Cancel model response
          </Text>
          <Text>
            <Text bold>Up/Down arrows</Text> - Navigate command history
          </Text>
          <Text>
            <Text bold>Tab</Text> - Auto-complete commands
          </Text>
        </Box>
      </Box>

      <Box margin={1} flexDirection="column">
        <Text bold>Approval modes</Text>
        <Box marginLeft={2} flexDirection="column" gap={0}>
          <Text>
            <Text bold>suggest</Text> - Ask before executing any commands
          </Text>
          <Text>
            <Text bold>auto-edit</Text> - Auto-approve file edits; prompt for
            commands
          </Text>
          <Text>
            <Text bold>full-auto</Text> - Auto-approve all operations when safe
          </Text>
        </Box>
      </Box>

      <Text color="gray" dimColor>
        Press ESC or q to go back
      </Text>
    </Box>
  );
}
