/**
 * Model settings configuration menu component.
 * 
 * This component allows users to configure model-related settings
 * such as the default model and model parameters.
 */

import React, { useState, useEffect } from "react";
import { Box, Text } from "ink";
import SelectInput from "../select-input/select-input.js";
import Spinner from "../vendor/ink-spinner.js";
import { 
  DEFAULT_AGENTIC_MODEL, 
  CURRENT_PROVIDER, 
  getProviderDisplayName,
  getProviderModels,
  loadConfig, 
  updateModel 
} from "../../utils/config.js";

// Menu item type
type MenuItem = {
  label: string;
  value: string;
};

// Props for the Model Settings menu
type ModelSettingsMenuProps = {
  onBack: () => void;
};

/**
 * Model settings configuration menu component.
 */
export function ModelSettingsMenu({ onBack }: ModelSettingsMenuProps): React.ReactElement {
  const [loading, setLoading] = useState<boolean>(true);
  const [message, setMessage] = useState<string | null>(null);
  const [currentModel, setCurrentModel] = useState<string>(DEFAULT_AGENTIC_MODEL);
  const [setMessageType] = useState<(type: "success" | "error") => void>(() => () => {});
  const [currentProvider, setCurrentProvider] = useState<string>(CURRENT_PROVIDER);

  // Load current model setting
  useEffect(() => {
    const loadModelSettings = async (): Promise<void> => {
      try {
        // Load current model and provider from config
        const config = loadConfig();
        if (config.model) {
          setCurrentModel(config.model);
        }
        if (config.provider) {
          setCurrentProvider(config.provider);
        }
      } catch (error) {
        setMessage(`Error loading configuration: ${error instanceof Error ? error.message : String(error)}`);
        setMessageType("error");
      } finally {
        setLoading(false);
      }
    };

    void loadModelSettings();
  }, []);

  // Generate model selection menu items based on provider
  const getModelItems = (): MenuItem[] => {
    // Get models for the current provider
    const providerModels = getProviderModels(currentProvider);
    
    const modelItems: MenuItem[] = providerModels.map(model => ({
      label: `${model}${model === currentModel ? ' (current)' : ''}`,
      value: model,
    }));
    
    // Add back option
    modelItems.push({
      label: "Back to main menu",
      value: "back",
    });
    
    return modelItems;
  };

  // Handle model selection
  const handleSelect = (item: MenuItem): void => {
    if (item.value === "back") {
      onBack();
      return;
    }
    
    // If selecting the current model, do nothing
    if (item.value === currentModel) {
      return;
    }
    
    // Save selected model
    setLoading(true);
    try {
      const success = updateModel(item.value);
      if (success) {
        setCurrentModel(item.value);
        // Don't show a message for successful selection
      } else {
        setMessage("Failed to update model setting");
        setMessageType("error");
      }
    } catch (error) {
      setMessage(`Error saving model setting: ${error instanceof Error ? error.message : String(error)}`);
      setMessageType("error");
    } finally {
      setLoading(false);
    }
  };

  // Render loading spinner
  if (loading) {
    return (
      <Box>
        <Text color="green">
          <Spinner type="dots" />
        </Text>
        <Text> Loading...</Text>
      </Box>
    );
  }

  // Main menu with message if present
  return (
    <Box flexDirection="column">
      {message && (
        <Box marginBottom={1}>
          <Text color="red">{message}</Text>
        </Box>
      )}
      <Box marginBottom={1}>
        <Text>Select a model for {getProviderDisplayName(currentProvider)} (Current: <Text bold>{currentModel}</Text>):</Text>
      </Box>
      <SelectInput items={getModelItems()} onSelect={handleSelect} />
    </Box>
  );
}
