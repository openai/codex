import { Box, Text } from "ink";
import React, { useMemo } from "react";

type TextCompletionProps = {
  /**
   * Array of text completion options to display in the list
   */
  completions: Array<string>;

  /**
   * Maximum number of completion items to show at once in the view
   */
  displayLimit: number;

  /**
   * Index of the currently selected completion in the completions array
   */
  selectedCompletion: number;
};

function TerminalChatCompletions({
  completions,
  selectedCompletion,
  displayLimit,
}: TextCompletionProps): JSX.Element {
  const visibleItems = useMemo(() => {
    // Try to keep selection centered in view
    let startIndex = Math.max(
      0,
      selectedCompletion - Math.floor(displayLimit / 2),
    );

    // Fix window position when at the end of the list
    if (completions.length - startIndex < displayLimit) {
      startIndex = Math.max(0, completions.length - displayLimit);
    }

    const endIndex = Math.min(completions.length, startIndex + displayLimit);

    return completions.slice(startIndex, endIndex).map((completion, index) => ({
      completion,
      originalIndex: index + startIndex,
    }));
  }, [completions, selectedCompletion, displayLimit]);

  return (
    <Box flexDirection="column">
      {visibleItems.map(({ completion, originalIndex }) => (
        <Text
          key={completion}
          dimColor={originalIndex !== selectedCompletion}
          underline={originalIndex === selectedCompletion}
          backgroundColor={
            originalIndex === selectedCompletion ? "blackBright" : undefined
          }
        >
          {completion}
        </Text>
      ))}
    </Box>
  );
}

export default TerminalChatCompletions;
