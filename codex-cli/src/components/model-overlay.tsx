import TypeaheadOverlay from "./typeahead-overlay.js";
import {
  getAvailableModels,
  RECOMMENDED_MODELS as _RECOMMENDED_MODELS,
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
  currentProvider?: string;
  hasLastResponse: boolean;
  providers?: Record<string, { name: string; baseURL: string; envKey: string }>;
  onSelect: (allModels: Array<string>, model: string) => void;
  onSelectProvider?: (provider: string) => void;
  onExit: () => void;
};

export default function ModelOverlay({
  currentModel,
  providers = {},
  currentProvider = "openai",
  hasLastResponse,
  onSelect,
  onSelectProvider,
  onExit,
}: Props): JSX.Element {
  const [items, setItems] = useState<Array<{ label: string; value: string }>>(
    [],
  );
  const [providerItems, _setProviderItems] = useState<
    Array<{ label: string; value: string }>
  >(Object.values(providers).map((p) => ({ label: p.name, value: p.name })));
  const [mode, setMode] = useState<"model" | "provider">("model");
  const [isLoading, setIsLoading] = useState<boolean>(true);

  // This effect will run when the provider changes to update the model list
  useEffect(() => {
    setIsLoading(true);
    (async () => {
      try {
        const models = await getAvailableModels(currentProvider);
        // Convert the models to the format needed by TypeaheadOverlay
        setItems(
          models.map((m) => ({
            label: m,
            value: m,
          })),
        );
      } catch (error) {
        // Silently handle errors - remove console.error
        // console.error("Error loading models:", error);
      } finally {
        setIsLoading(false);
      }
    })();
  }, [currentProvider]);

  // ---------------------------------------------------------------------------
  // If the conversation already contains a response we cannot change the model
  // anymore because the backend requires a consistent model across the entire
  // run.  In that scenario we replace the regular typeahead picker with a
  // simple message instructing the user to start a new chat.  The only
  // available action is to dismiss the overlay (Esc or Enter).
  // ---------------------------------------------------------------------------

  // Register input handling for switching between model and provider selection
  useInput((_input, key) => {
    if (hasLastResponse && (key.escape || key.return)) {
      onExit();
    } else if (!hasLastResponse) {
      if (key.tab) {
        setMode(mode === "model" ? "provider" : "model");
      }
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

  if (mode === "provider") {
    return (
      <TypeaheadOverlay
        title="Select provider"
        description={
          <Box flexDirection="column">
            <Text>
              Current provider:{" "}
              <Text color="greenBright">{currentProvider}</Text>
            </Text>
            <Text dimColor>press tab to switch to model selection</Text>
          </Box>
        }
        initialItems={providerItems}
        currentValue={currentProvider}
        onSelect={(provider) => {
          if (onSelectProvider) {
            onSelectProvider(provider);
            // Immediately switch to model selection so user can pick a model for the new provider
            setMode("model");
          }
        }}
        onExit={onExit}
      />
    );
  }

  return (
    <TypeaheadOverlay
      title="Select model"
      description={
        <Box flexDirection="column">
          <Text>
            Current model: <Text color="greenBright">{currentModel}</Text>
          </Text>
          <Text>
            Current provider: <Text color="greenBright">{currentProvider}</Text>
          </Text>
          {isLoading && <Text color="yellow">Loading models...</Text>}
          <Text dimColor>press tab to switch to provider selection</Text>
        </Box>
      }
      initialItems={items}
      currentValue={currentModel}
      onSelect={(selectedModel) =>
        onSelect(
          items?.map((m) => m.value),
          selectedModel,
        )
      }
      onExit={onExit}
    />
  );
}
