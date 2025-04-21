import TypeaheadOverlay from "./typeahead-overlay.js";
import {
  getAvailableModels,
  RECOMMENDED_MODELS,
} from "../utils/model-utils.js";
import { Text, useInput } from "ink";
import React, { useEffect, useState } from "react";

/**
 * Props for <ModelOverlay>.
 *
 * When `hasLastResponse` is true the user has already received at least one
 * assistant response in the current session which means switching models is no
 * longer supported – the overlay should therefore show an error and only allow
 * the user to close it.
 */
type Props = {
  currentModel: string;
  hasLastResponse: boolean;
  onSelect: (model: string) => void;
  onExit: () => void;
};

export default function ModelOverlay({
  currentModel,
  onSelect,
  onExit,
}: Props): JSX.Element {
  const [items, setItems] = useState<Array<{ label: string; value: string }>>(
    [],
  );

  useEffect(() => {
    (async () => {
      const models = await getAvailableModels();

      // Split the list into recommended and “other” models.
      const recommended = RECOMMENDED_MODELS.filter((m) => models.includes(m));
      const others = models.filter((m) => !recommended.includes(m));

      const ordered = [...recommended, ...others.sort()];

      setItems(
        ordered.map((m) => ({
          label: recommended.includes(m) ? `⭐ ${m}` : m,
          value: m,
        })),
      );
    })();
  }, []);

  // ---------------------------------------------------------------------------
  // If the conversation already contains a response we cannot change the model
  // anymore because the backend requires a consistent model across the entire
  // run.  In that scenario we replace the regular typeahead picker with a
  // simple message instructing the user to start a new chat.  The only
  // available action is to dismiss the overlay (Esc or Enter).
  // ---------------------------------------------------------------------------

  // Always register input handling so hooks are called consistently.
  // Dismiss overlay on escape/enter
  useInput((_input, key) => {
    if (key.escape || key.return) {
      onExit();
    }
  });

  // Always allow switching models in-session
  return (
    <TypeaheadOverlay
      title="Switch model"
      description={
        <Text>
          Current model: <Text color="greenBright">{currentModel}</Text>
        </Text>
      }
      initialItems={items}
      currentValue={currentModel}
      onSelect={onSelect}
      onExit={onExit}
    />
  );
}
