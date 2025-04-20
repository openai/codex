import TypeaheadOverlay from "./typeahead-overlay.js";
import { RECOMMENDED_MODELS } from "../utils/model-utils.js";
import { Box, Text, useInput } from "ink";
import React, { useEffect, useState } from "react";

/**
 * Props for <ModelOverlay>.
 *
 * When `hasLastResponse` is true the user has already received at least one
 * assistant response in the current session which means switching models is no
 * longer supported – the overlay should therefore show an error and only allow
 * the user to close it.
 */
type Props = {
  currentModel: string;
  hasLastResponse: boolean;
  onSelect: (model: string) => void;
  onExit: () => void;
  availableModels: Array<string>;
};

export default function ModelOverlay({
  currentModel,
  hasLastResponse,
  onSelect,
  onExit,
  availableModels,
}: Props): JSX.Element {
  const [items, setItems] = useState<Array<{ label: string; value: string }>>(
    [],
  );

  useEffect(() => {
    const recommended = RECOMMENDED_MODELS.filter((m) =>
      availableModels.includes(m),
    );
    const others = availableModels.filter((m) => !recommended.includes(m));
    const ordered = [...recommended, ...others.sort()];

    setItems(
      ordered.map((m) => ({
        label: recommended.includes(m) ? `⭐ ${m}` : m,
        value: m,
      })),
    );
  }, [availableModels]);

  useInput((_input, key) => {
    if (hasLastResponse && (key.escape || key.return)) {
      onExit();
    }
  });

  if (hasLastResponse) {
    return (
      <Box
        flexDirection="column"
        borderStyle="round"
        borderColor="gray"
        width={80}
      >
        <Box paddingX={1}>
          <Text bold color="red">
            Unable to switch model
          </Text>
        </Box>
        <Box paddingX={1}>
          <Text>
            You can only pick a model before the assistant sends its first
            response. To use a different model please start a new chat.
          </Text>
        </Box>
        <Box paddingX={1}>
          <Text dimColor>press esc or enter to close</Text>
        </Box>
      </Box>
    );
  }

  return (
    <TypeaheadOverlay
      title="Switch model"
      description={
        <Text>
          Current model: <Text color="greenBright">{currentModel}</Text>
        </Text>
      }
      initialItems={items}
      currentValue={currentModel}
      onSelect={onSelect}
      onExit={onExit}
    />
  );
}
