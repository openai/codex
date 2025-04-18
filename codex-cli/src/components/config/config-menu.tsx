/**
 * Main configuration menu component.
 *
 * This component displays the top-level configuration options and
 * handles navigation to sub-menus.
 */

import React, { useState } from "react";
import { Box, Text } from "ink";
import SelectInput from "../select-input/select-input.js";
import { ApiKeyMenu } from "../config/api-key-menu.js";
import { ModelSettingsMenu } from "./model-settings-menu.js";
import { ApprovalModeMenu } from "./approval-mode-menu.js";
import { MemorySettingsMenu } from "./memory-settings-menu.js";
import { HistorySettingsMenu } from "./history-settings-menu.js";
import { InstructionsMenu } from "./instructions-menu.js";
import { ProviderMenu } from "./provider-menu.js";

// Menu item type
type MenuItem = {
  label: string;
  value: string;
};

// Available menu options
const items: MenuItem[] = [
  {
    label: "Model Provider",
    value: "provider",
  },
  {
    label: "Model Provider API Key",
    value: "api-key",
  },
  {
    label: "Model Settings",
    value: "model",
  },
  {
    label: "Approval Mode",
    value: "approval",
  },
  {
    label: "Memory Settings",
    value: "memory",
  },
  {
    label: "History Settings",
    value: "history",
  },
  {
    label: "Instructions",
    value: "instructions",
  },
  {
    label: "Exit",
    value: "exit",
  },
];

/**
 * Main configuration menu component.
 */
export function ConfigMenu(): React.ReactElement {
  const [selectedMenu, setSelectedMenu] = useState<string | null>(null);

  // Handle menu selection
  const handleSelect = (item: MenuItem): void => {
    if (item.value === "exit") {
      // Exit the configuration UI
      if (typeof (global as any).__configExit === "function") {
        (global as any).__configExit();
      }
      return;
    }

    setSelectedMenu(item.value);
  };

  // Handle back button from sub-menus
  const handleBack = (): void => {
    setSelectedMenu(null);
  };

  // Render the selected sub-menu or the main menu
  if (selectedMenu === "provider") {
    return <ProviderMenu onBack={handleBack} />;
  } else if (selectedMenu === "api-key") {
    return <ApiKeyMenu onBack={handleBack} />;
  } else if (selectedMenu === "model") {
    return <ModelSettingsMenu onBack={handleBack} />;
  } else if (selectedMenu === "approval") {
    return <ApprovalModeMenu onBack={handleBack} />;
  } else if (selectedMenu === "memory") {
    return <MemorySettingsMenu onBack={handleBack} />;
  } else if (selectedMenu === "history") {
    return <HistorySettingsMenu onBack={handleBack} />;
  } else if (selectedMenu === "instructions") {
    return <InstructionsMenu onBack={handleBack} />;
  }

  // Main menu
  return (
    <Box flexDirection="column">
      <Box marginBottom={1}>
        <Text>Select a setting to configure:</Text>
      </Box>
      <SelectInput items={items} onSelect={handleSelect} />
    </Box>
  );
}
