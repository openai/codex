import TypeaheadOverlay from "./typeahead-overlay.js";
import { Text } from "ink";
import React from "react";

type Props = {
  /** Current memory enabled state */
  currentMemory: boolean;
  /** Toggle memory enabled, then exit */
  onToggle: (enabled: boolean) => void;
  /** Exit overlay without changes */
  onExit: () => void;
};

/**
 * Overlay to toggle repository-specific memory file usage.
 */
export default function SettingsOverlay({
  currentMemory,
  onToggle,
  onExit,
}: Props): JSX.Element {
  const label = `Use memory file: ${currentMemory ? "On" : "Off"}`;
  const items = [{ label, value: "memory" }];
  return (
    <TypeaheadOverlay
      title="Settings"
      description={<Text>Toggle repository memory feature</Text>}
      initialItems={items}
      currentValue="memory"
      onSelect={() => {
        onToggle(!currentMemory);
      }}
      onExit={onExit}
    />
  );
}
