import TypeaheadOverlay from "./typeahead-overlay.js";
import { AutoApprovalMode } from "../utils/auto-approval-mode.js";
import { Text } from "ink";
import React from "react";

type Props = {
  currentMode: string;
  onSelect: (mode: string) => void;
  onExit: () => void;
};

/**
 * Overlay to switch between the different automatic‑approval policies.
 *
 * The list of available modes is derived from the AutoApprovalMode enum so we
 * stay in sync with the core agent behaviour.  It re‑uses the generic
 * TypeaheadOverlay component for the actual UI/UX.
 */
export default function ApprovalModeOverlay({
  currentMode,
  onSelect,
  onExit,
}: Props): JSX.Element {
  const items = React.useMemo(
    () =>
      Object.values(AutoApprovalMode).map((m) => ({
        label: m,
        value: m,
      })),
    [],
  );

  return (
    <TypeaheadOverlay
      title="Switch approval mode"
      description={
        <Text>
          Current mode: <Text color="greenBright">{currentMode}</Text>
        </Text>
      }
      initialItems={items}
      currentValue={currentMode}
      onSelect={onSelect}
      onExit={onExit}
    />
  );
}
