import React from "react";
import { Box, Text } from "ink";
import chalk from "chalk";

// Define the props based on how it's used in TerminalChat
type TerminalChatToolCallCommandProps = {
  commandForDisplay: string;
  explanation?: string; // It's used with and without explanation
  // Add any other props needed for displaying the command/explanation
};

// This component is responsible for rendering a proposed shell command
// within the confirmation prompt area of the chat UI.
export function TerminalChatToolCallCommand({
  commandForDisplay,
  explanation,
}: TerminalChatToolCallCommandProps) {
  return (
    <Box flexDirection="column" width="100%">
      {/* You can customize the rendering here */}
      <Box>
        <Text bold>Proposed command:</Text>
      </Box>
      <Box marginTop={1}>
        {/* Render the command, perhaps in a distinct color */}
        <Text color="yellow">{commandForDisplay}</Text>
      </Box>
      {explanation && (
        <Box marginTop={1} flexDirection="column">
          <Box>
            <Text bold>Explanation:</Text>
          </Box>
          <Box marginTop={0}>
            {/* Render the explanation */}
            <Text>{explanation}</Text>
          </Box>
        </Box>
      )}
      {/* Add other UI elements if needed for the command display */}
    </Box>
  );
}
