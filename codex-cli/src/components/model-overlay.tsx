import TypeaheadOverlay from "./typeahead-overlay.js";
import {
  getAvailableModels,
  RECOMMENDED_MODELS,
} from "../utils/model-utils.js";
import { providers } from "../utils/providers.js";
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
  onSelect: (model: string) => void;
  onSelectProvider?: (provider: string) => void;
  onExit: () => void;
  availableModels: Array<string>;
};

export default function ModelOverlay({
  currentModel,
  currentProvider = "openai",
  hasLastResponse,
  onSelect,
  onSelectProvider,
  onExit,
  availableModels,
}: Props): JSX.Element {
  const [items, setItems] = useState<Array<{ label: string; value: string }>>(
    [],
  );
  const [providerItems, _setProviderItems] = useState<
    Array<{ label: string; value: string }>
  >(Object.values(providers).map((p) => ({ label: p.name, value: p.name })));
  const [mode, setMode] = useState<"model" | "provider">("model");
  const [isLoading, setIsLoading] = useState<boolean>(false);

  // Set up model items based on availableModels and currentProvider
  useEffect(() => {
    if (currentProvider === "openai") {
      // For OpenAI, use the pre-fetched available models with recommended ones first
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
    } else {
      // For other providers, fetch their models
      setIsLoading(true);
      (async () => {
        try {
          const models = await getAvailableModels(currentProvider);
          setItems(
            models.map((m) => ({
              label: m,
              value: m,
            })),
          );
        } catch (error) {
          // Silently handle errors
        } finally {
          setIsLoading(false);
        }
      })();
    }
  }, [currentProvider, availableModels]);

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
      onSelect={onSelect}
      onExit={onExit}
    />
  );
}
