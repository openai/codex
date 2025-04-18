/**
 * Memory settings configuration menu component.
 *
 * This component allows users to configure memory-related settings.
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
    label: "Enable memory",
    value: "toggle",
  },
  {
    label: "Back to main menu",
    value: "back",
  },
];

// Props for the Memory Settings menu
type MemorySettingsMenuProps = {
  onBack: () => void;
};

/**
 * Memory settings configuration menu component.
 */
export function MemorySettingsMenu({
  onBack,
}: MemorySettingsMenuProps): React.ReactElement {
  const [message, setMessage] = useState<string | null>(null);

  // Handle menu selection
  const handleSelect = (item: MenuItem): void => {
    if (item.value === "back") {
      onBack();
      return;
    }

    // For now, just display a message for unimplemented features
    setMessage("Memory settings will be implemented in a future update");
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
        <Text>Memory Settings:</Text>
      </Box>
      <SelectInput items={items} onSelect={handleSelect} />
    </Box>
  );
}
