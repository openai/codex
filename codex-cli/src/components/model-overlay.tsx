import TypeaheadOverlay from "./typeahead-overlay.js";
import {
  getAvailableModels,
  RECOMMENDED_OPENAI_MODELS,
  RECOMMENDED_OLLAMA_MODELS,
} from "../utils/model-utils.js";
import { isOllamaModel } from "../utils/ollama-client.js";
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
};

export default function ModelOverlay({
  currentModel,
  hasLastResponse,
  onSelect,
  onExit,
}: Props): JSX.Element {
  const [items, setItems] = useState<Array<{ label: string; value: string }>>(
    [],
  );

  useEffect(() => {
    (async () => {
      const models = await getAvailableModels();

      // Split the list into recommended OpenAI, recommended Ollama, and "other" models.
      const recommendedOpenAI = RECOMMENDED_OPENAI_MODELS.filter((m) => models.includes(m));
      const recommendedOllama = RECOMMENDED_OLLAMA_MODELS.map(m => `ollama:${m}`).filter((m) => models.includes(m));
      const others = models.filter((m) => 
        !recommendedOpenAI.includes(m) && !recommendedOllama.includes(m)
      );

      // Separate Ollama models from OpenAI models in the "others" category
      const otherOllamaModels = others.filter(m => isOllamaModel(m)).sort();
      const otherOpenAIModels = others.filter(m => !isOllamaModel(m)).sort();

      // Order: recommended OpenAI, other OpenAI, recommended Ollama, other Ollama
      const ordered = [
        ...recommendedOpenAI,
        ...otherOpenAIModels,
        ...recommendedOllama,
        ...otherOllamaModels
      ];

      setItems(
        ordered.map((m) => ({
          label: getModelLabel(m, recommendedOpenAI, recommendedOllama),
          value: m,
        })),
      );
    })();
  }, []);

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

  /**
   * Get a formatted label for a model.
   * 
   * @param model The model name.
   * @param recommendedOpenAI List of recommended OpenAI models.
   * @param recommendedOllama List of recommended Ollama models.
   * @returns A formatted label for the model.
   */
  function getModelLabel(
    model: string, 
    recommendedOpenAI: string[], 
    recommendedOllama: string[]
  ): string {
    if (recommendedOpenAI.includes(model)) {
      return `⭐ ${model}`;
    } else if (recommendedOllama.includes(model)) {
      return `🚀 ${model}`;
    } else if (isOllamaModel(model)) {
      return `🚀 ${model}`;
    } else {
      return model;
    }
  }

  return (
    <TypeaheadOverlay
      title="Switch model"
      description={
        <Text>
          Current model: <Text color={isOllamaModel(currentModel) ? "blueBright" : "greenBright"}>{currentModel}</Text>
        </Text>
      }
      initialItems={items}
      currentValue={currentModel}
      onSelect={onSelect}
      onExit={onExit}
    />
  );
}
