/**
 * API Key configuration menu component.
 *
 * This component allows users to securely set or delete API keys
 * for model providers.
 */

import React, { useState } from "react";
import { Box, Text, useInput } from "ink";
import SelectInput from "../select-input/select-input.js";
import TextInput from "../vendor/ink-text-input.js";
import Spinner from "../vendor/ink-spinner.js";
import { storeApiKey } from "../../utils/keychain.js";
import { CURRENT_PROVIDER, setApiKey } from "../../utils/config.js";

// Menu item type
type MenuItem = {
  label: string;
  value: string;
};

// Available menu options
const items: MenuItem[] = [
  {
    label: "Set new API Key",
    value: "set",
  },
  {
    label: "Delete existing API Key",
    value: "delete",
  },
  {
    label: "Back to main menu",
    value: "back",
  },
];

// Props for the API Key menu
type ApiKeyMenuProps = {
  onBack: () => void;
};

/**
 * API Key configuration menu component.
 */
export function ApiKeyMenu({ onBack }: ApiKeyMenuProps): React.ReactElement {
  const [selectedAction, setSelectedAction] = useState<string | null>(null);
  const [apiKey, setApiKeyValue] = useState<string>("");
  const [storeInKeychain, setStoreInKeychain] = useState<boolean>(true);
  const [confirmDelete, setConfirmDelete] = useState<boolean>(false);
  const [loading, setLoading] = useState<boolean>(false);
  const [message, setMessage] = useState<string | null>(null);
  const [messageType, setMessageType] = useState<"success" | "error">(
    "success",
  );

  // Handle menu selection
  const handleSelect = (item: MenuItem): void => {
    if (item.value === "back") {
      onBack();
      return;
    }

    setSelectedAction(item.value);
    setMessage(null);

    // If delete option is selected, show confirmation
    if (item.value === "delete") {
      handleDeleteAction();
    }
  };

  // Handle API key input submission
  const handleApiKeySubmit = async (): Promise<void> => {
    if (!apiKey.trim()) {
      setMessage("API Key cannot be empty");
      setMessageType("error");
      return;
    }

    setLoading(true);
    setMessage(null);

    try {
      // Import keychain utility dynamically to avoid circular dependencies
      // Store API key in keychain
      const success = await storeApiKey(CURRENT_PROVIDER, apiKey);

      if (success) {
        // Set API key for current session
        setApiKey(apiKey);
        setMessage("API Key saved successfully");
        setMessageType("success");
        setSelectedAction(null);
      } else {
        setMessage("Failed to save API Key to system keychain");
        setMessageType("error");
      }
    } catch (error) {
      setMessage(
        `Error saving API Key: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      setMessageType("error");
    } finally {
      setLoading(false);
    }
  };

  // Handle keychain storage toggle
  useInput((input) => {
    if (selectedAction === "set" && (input === "y" || input === "n")) {
      setStoreInKeychain(input === "y");
    }

    if (selectedAction === "delete" && (input === "y" || input === "n")) {
      if (input === "y") {
        void handleDeleteConfirm(true);
      } else {
        setConfirmDelete(false);
        setSelectedAction(null);
      }
    }
  });

  // Handle API key deletion confirmation
  const handleDeleteConfirm = async (confirmed: boolean): Promise<void> => {
    if (!confirmed) {
      setConfirmDelete(false);
      setSelectedAction(null);
      return;
    }

    setLoading(true);
    setMessage(null);

    try {
      // Import keychain utility dynamically to avoid circular dependencies
      const { deleteApiKey } = await import("../../utils/keychain.js");
      const { setApiKey, CURRENT_PROVIDER } = await import(
        "../../utils/config.js"
      );

      // Delete API key from keychain
      const success = await deleteApiKey(CURRENT_PROVIDER);

      if (success) {
        // Clear API key for current session
        setApiKey("");
        setMessage("API Key deleted successfully");
        setMessageType("success");
      } else {
        setMessage("Failed to delete API Key from system keychain");
        setMessageType("error");
      }

      setSelectedAction(null);
      setConfirmDelete(false);
    } catch (error) {
      setMessage(
        `Error deleting API Key: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      setMessageType("error");
    } finally {
      setLoading(false);
    }
  };

  // This function is called when the user selects the delete option
  const handleDeleteAction = (): void => {
    setConfirmDelete(true);
  };

  // Render loading spinner
  if (loading) {
    return (
      <Box>
        <Text color="green">
          <Spinner type="dots" />
        </Text>
        <Text> Processing...</Text>
      </Box>
    );
  }

  // Render delete confirmation
  if (selectedAction === "delete" && confirmDelete) {
    // If user selects delete and confirms, handle the deletion
    return (
      <Box flexDirection="column">
        <Box marginBottom={1}>
          <Text>
            Are you sure you want to delete your stored API Key? (y/n)
          </Text>
        </Box>
      </Box>
    );
  }

  // Render API key input
  if (selectedAction === "set") {
    return (
      <Box flexDirection="column">
        <Box marginBottom={1}>
          <Text>Enter your API Key:</Text>
        </Box>
        <Box marginBottom={1}>
          <TextInput
            value={apiKey}
            onChange={setApiKeyValue}
            onSubmit={handleApiKeySubmit}
            mask="*"
            placeholder="API Key"
          />
        </Box>
        <Box marginBottom={1}>
          <Text>
            Store API Key securely in system keychain? (y/n){" "}
            {storeInKeychain ? "Yes" : "No"}
          </Text>
        </Box>
        <Box marginBottom={1}>
          <Text dimColor>Press Enter to confirm</Text>
        </Box>
      </Box>
    );
  }

  // Render main menu with message if present
  return (
    <Box flexDirection="column">
      {message && (
        <Box marginBottom={1}>
          <Text color={messageType === "success" ? "green" : "red"}>
            {message}
          </Text>
        </Box>
      )}
      <Box marginBottom={1}>
        <Text>Model Provider API Key:</Text>
      </Box>
      <SelectInput items={items} onSelect={handleSelect} />
    </Box>
  );
}
