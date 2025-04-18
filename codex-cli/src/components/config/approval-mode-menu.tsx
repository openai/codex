/**
 * Approval mode configuration menu component.
 *
 * This component allows users to select the approval mode for Codex.
 */

import React, { useState, useEffect } from "react";
import { Box, Text } from "ink";
import SelectInput from "../select-input/select-input.js";
import Spinner from "../vendor/ink-spinner.js";
import { AutoApprovalMode } from "../../utils/auto-approval-mode.js";
import { loadConfig, updateApprovalMode } from "../../utils/config.js";

// Menu item type
type MenuItem = {
  label: string;
  value: string;
};

// Generate approval mode menu items
const getApprovalModeItems = (): MenuItem[] => {
  // Create menu items for approval modes
  const modeItems: MenuItem[] = [
    {
      label: `suggest - Suggest changes but ask for approval`,
      value: AutoApprovalMode.SUGGEST,
    },
    {
      label: `auto-edit - Automatically approve edits, but prompt for commands`,
      value: AutoApprovalMode.AUTO_EDIT,
    },
    {
      label: `full-auto - Automatically run commands in a sandbox; only prompt for failures`,
      value: AutoApprovalMode.FULL_AUTO,
    },
    {
      label: "Back to main menu",
      value: "back" as any,
    },
  ];
  
  return modeItems;
};

// Props for the Approval Mode menu
type ApprovalModeMenuProps = {
  onBack: () => void;
};

/**
 * Approval mode configuration menu component.
 */
export function ApprovalModeMenu({
  onBack,
}: ApprovalModeMenuProps): React.ReactElement {
  const [message, setMessage] = useState<string | null>(null);
  const [currentMode, setCurrentMode] = useState<string>(AutoApprovalMode.SUGGEST);
  const [loading, setLoading] = useState<boolean>(true);
  const [setMessageType] = useState<(type: "success" | "error") => void>(() => () => {});

  // Load current approval mode on component mount
  useEffect(() => {
    const loadCurrentMode = async (): Promise<void> => {
      try {
        const config = loadConfig();
        if (config.approvalMode) {
          setCurrentMode(config.approvalMode);
        }
      } catch (error) {
        setMessage(`Error loading configuration: ${error instanceof Error ? error.message : String(error)}`);
        setMessageType("error");
      } finally {
        setLoading(false);
      }
    };

    void loadCurrentMode();
  }, []);

  // Handle menu selection
  const handleSelect = (item: MenuItem): void => {
    if (item.value === "back") {
      onBack();
      return;
    }

    // If selecting the current mode, do nothing
    if (item.value === currentMode) {
      return;
    }

    setLoading(true);
    setMessage(null);

    // Save the approval mode to config
    try {
      const success = updateApprovalMode(item.value as AutoApprovalMode);
      if (success) {
        // Set the approval mode without showing a message
        setCurrentMode(item.value);
      } else {
        setMessage(`Failed to update approval mode`);
        setMessageType("error");
      }
    } catch (error) {
      setMessage(`Error saving approval mode: ${error instanceof Error ? error.message : String(error)}`);
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
        <Text>Select approval mode (Current: <Text bold>{currentMode}</Text>):</Text>
      </Box>
      <SelectInput items={getApprovalModeItems()} onSelect={handleSelect} />
    </Box>
  );
}
