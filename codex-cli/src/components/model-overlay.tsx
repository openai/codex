import TypeaheadOverlay from "./typeahead-overlay.js";
import {
  getAvailableModels,
  getAvailableOpenRouterModels,
  RECOMMENDED_MODELS,
  OPENROUTER_RECOMMENDED_MODELS,
} from "../utils/model-utils.js";
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
  useOpenRouter?: boolean;
};

export default function ModelOverlay({
  currentModel,
  hasLastResponse,
  onSelect,
  onExit,
  useOpenRouter = false,
}: Props): JSX.Element {
  const [items, setItems] = useState<Array<{ label: string; value: string }>>(
    [],
  );

  useEffect(() => {
    (async () => {
      let models: string[] = [];
      let recommended: string[] = [];
      
      if (useOpenRouter) {
        // Get both OpenAI and OpenRouter models when OpenRouter is enabled
        const openAIModels = await getAvailableModels();
        const openRouterModels = await getAvailableOpenRouterModels();
        models = [...openAIModels, ...openRouterModels];
        
        // Combine recommended models from both sources
        recommended = [
          ...RECOMMENDED_MODELS.filter(m => openAIModels.includes(m)),
          ...OPENROUTER_RECOMMENDED_MODELS.filter(m => openRouterModels.includes(m))
        ];
      } else {
        // Only get OpenAI models when OpenRouter is disabled
        models = await getAvailableModels();
        recommended = RECOMMENDED_MODELS.filter((m) => models.includes(m));
      }

      // Filter out models that are already in the recommended list
      const others = models.filter((m) => !recommended.includes(m));

      const ordered = [...recommended, ...others.sort()];

      setItems(
        ordered.map((m) => ({
          label: recommended.includes(m) 
            ? `⭐ ${m}` 
            : m.includes('/') 
              ? `🔄 ${m}` // Mark OpenRouter models with a special icon
              : m,
          value: m,
        })),
      );
    })();
  }, [useOpenRouter]);

  // ---------------------------------------------------------------------------
  // If the conversation already contains a response we cannot change the model
  // anymore because the backend requires a consistent model across the entire
  // run.  In that scenario we replace the regular typeahead picker with a
  // simple message instructing the user to start a new chat.  The only
  // available action is to dismiss the overlay (Esc or Enter).
  // ---------------------------------------------------------------------------

  // Always register input handling so hooks are called consistently.
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
        <Box flexDirection="column">
          <Text>
            Current model: <Text color="greenBright">{currentModel}</Text>
          </Text>
          {useOpenRouter && (
            <Text>
              <Text color="cyan">OpenRouter</Text> enabled: Models with <Text color="cyan">🔄</Text> prefix are from OpenRouter
            </Text>
          )}
        </Box>
      }
      initialItems={items}
      currentValue={currentModel}
      onSelect={onSelect}
      onExit={onExit}
    />
  );
}
