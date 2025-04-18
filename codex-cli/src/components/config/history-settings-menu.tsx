/**
 * History settings configuration menu component.
 *
 * This component allows users to configure history-related settings
 * such as saving history and sensitive patterns.
 */

import React, { useState } from "react";
import { Box, Text } from "ink";
import SelectInput from "../select-input/select-input.js";

// Menu item type
type MenuItem = {
  label: string;
  value: string;
};

// Available menu options
const items: MenuItem[] = [
  {
    label: "Save history",
    value: "toggle-save",
  },
  {
    label: "Maximum history size",
    value: "max-size",
  },
  {
    label: "Configure sensitive patterns",
    value: "patterns",
  },
  {
    label: "Back to main menu",
    value: "back",
  },
];

// Props for the History Settings menu
type HistorySettingsMenuProps = {
  onBack: () => void;
};

/**
 * History settings configuration menu component.
 */
export function HistorySettingsMenu({
  onBack,
}: HistorySettingsMenuProps): React.ReactElement {
  const [message, setMessage] = useState<string | null>(null);

  // Handle menu selection
  const handleSelect = (item: MenuItem): void => {
    if (item.value === "back") {
      onBack();
      return;
    }

    // For now, just display a message for unimplemented features
    setMessage("History settings will be implemented in a future update");
  };

  // Main menu with message if present
  return (
    <Box flexDirection="column">
      {message && (
        <Box marginBottom={1}>
          <Text>{message}</Text>
        </Box>
      )}
      <Box marginBottom={1}>
        <Text>History Settings:</Text>
      </Box>
      <SelectInput items={items} onSelect={handleSelect} />
    </Box>
  );
}
