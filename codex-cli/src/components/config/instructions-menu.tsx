/**
 * Instructions configuration menu component.
 *
 * This component allows users to edit or reset the instructions file.
 */

import React, { useState } from "react";
import { Box, Text } from "ink";
import SelectInput from "../select-input/select-input.js";
import { spawnSync } from "child_process";
import { INSTRUCTIONS_FILEPATH } from "../../utils/config.js";

// Menu item type
type MenuItem = {
  label: string;
  value: string;
};

// Available menu options
const items: MenuItem[] = [
  {
    label: "Edit instructions",
    value: "edit",
  },
  {
    label: "Reset to default",
    value: "reset",
  },
  {
    label: "Back to main menu",
    value: "back",
  },
];

// Props for the Instructions menu
type InstructionsMenuProps = {
  onBack: () => void;
};

/**
 * Instructions configuration menu component.
 */
export function InstructionsMenu({
  onBack,
}: InstructionsMenuProps): React.ReactElement {
  const [message, setMessage] = useState<string | null>(null);

  // Handle menu selection
  const handleSelect = (item: MenuItem): void => {
    if (item.value === "back") {
      onBack();
      return;
    }

    if (item.value === "edit") {
      // Open the instructions file in the user's editor
      const editor =
        process.env["EDITOR"] ||
        (process.platform === "win32" ? "notepad" : "vi");
      spawnSync(editor, [INSTRUCTIONS_FILEPATH], { stdio: "inherit" });
      setMessage("Instructions file opened in editor");
    } else if (item.value === "reset") {
      // For now, just display a message for unimplemented features
      setMessage("Reset instructions will be implemented in a future update");
    }
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
        <Text>Instructions:</Text>
      </Box>
      <SelectInput items={items} onSelect={handleSelect} />
    </Box>
  );
}
