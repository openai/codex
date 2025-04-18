/**
 * Provider selection menu component.
 *
 * This component allows users to select the model provider.
 */

import React, { useState, useEffect } from "react";
import { Box, Text } from "ink";
import SelectInput from "../select-input/select-input.js";
import Spinner from "../vendor/ink-spinner.js";
import { 
  CURRENT_PROVIDER, 
  AVAILABLE_PROVIDERS, 
  loadConfig, 
  updateProvider 
} from "../../utils/config.js";

// Menu item type
type MenuItem = {
  label: string;
  value: string;
};

// Generate provider menu items
const getProviderItems = (): MenuItem[] => {
  // Create menu items from available providers
  const providerItems: MenuItem[] = Object.entries(AVAILABLE_PROVIDERS).map(([key, config]) => ({
    label: `${config.displayName}`,
    value: key,
  }));
  
  // Add back option
  providerItems.push({
    label: "Back to main menu",
    value: "back",
  });
  
  return providerItems;
};

// Props for the Provider menu
type ProviderMenuProps = {
  onBack: () => void;
};

/**
 * Provider selection menu component.
 */
export function ProviderMenu({ onBack }: ProviderMenuProps): React.ReactElement {
  const [currentProvider, setCurrentProvider] = useState<string>(CURRENT_PROVIDER);
  const [loading, setLoading] = useState<boolean>(true);
  const [message, setMessage] = useState<string | null>(null);
  const [setMessageType] = useState<(type: "success" | "error") => void>(() => () => {});

  // Load current provider on component mount
  useEffect(() => {
    const loadCurrentProvider = async (): Promise<void> => {
      try {
        const config = loadConfig();
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

    void loadCurrentProvider();
  }, []);

  // Handle provider selection
  const handleSelect = (item: MenuItem): void => {
    if (item.value === "back") {
      onBack();
      return;
    }

    // If selecting the current provider, do nothing
    if (item.value === currentProvider) {
      return;
    }

    setLoading(true);
    setMessage(null);

    // Save the provider to config
    try {
      const success = updateProvider(item.value);
      if (success) {
        // Set the provider without showing a message
        setCurrentProvider(item.value);
      } else {
        setMessage(`Failed to update provider`);
        setMessageType("error");
      }
    } catch (error) {
      setMessage(`Error saving provider: ${error instanceof Error ? error.message : String(error)}`);
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
        <Text>Select provider (Current: <Text bold>{AVAILABLE_PROVIDERS[currentProvider]?.displayName || currentProvider}</Text>):</Text>
      </Box>
      <SelectInput items={getProviderItems()} onSelect={handleSelect} />
    </Box>
  );
}
