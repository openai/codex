import TypeaheadOverlay from "./typeahead-overlay.js";
import { RECOMMENDED_MODELS } from "../utils/model-utils.js"; // We no longer need getAvailableModels here
import { Box, Text, useInput } from "ink";
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
  onSelect: (model: string) => void; // This onSelect is called when a model is picked
  onExit: () => void;
  availableModels: Array<string>; // ADDED: Receive the list of available models from parent
};

export default function ModelOverlay({
  currentModel,
  hasLastResponse,
  onSelect,
  onExit,
  availableModels, // ADDED: Accept availableModels prop
}: Props): JSX.Element {
  const [items, setItems] = useState<Array<{ label: string; value: string }>>(
    [],
  ); // REMOVED: The useEffect that fetched getAvailableModels()
  // Instead, we'll populate items based on the availableModels prop

  useEffect(() => {
    // Filter recommended models to only include those that are actually available
    const recommended = RECOMMENDED_MODELS.filter((m) =>
      availableModels.includes(m),
    );
    // Filter other models to only include those that are actually available and not recommended
    const others = availableModels.filter((m) => !recommended.includes(m));

    // Order: Recommended first, then others alphabetically
    const ordered = [...recommended, ...others.sort()];

    setItems(
      ordered.map((m) => ({
        label: recommended.includes(m) ? `⭐ ${m}` : m,
        value: m,
      })),
    );
  }, [availableModels]); // DEPENDENCY: Re-run this effect when availableModels changes
  // ---------------------------------------------------------------------------
  // If the conversation already contains a response we cannot change the model
  // anymore because the backend requires a consistent model across the entire
  // run.  In that scenario we replace the regular typeahead picker with a
  // simple message instructing the user to start a new chat.  The only
  // available action is to dismiss the overlay (Esc or Enter).
  // ---------------------------------------------------------------------------
  // Always register input handling so hooks are called consistently.

  useInput((_input, key) => {
    if (hasLastResponse && (key.escape || key.return)) {
      onExit();
    }
  });

  if (hasLastResponse) {
    return (
      <Box
        flexDirection="column"
        borderStyle="round"
        borderColor="gray"
        width={80}
      >
               {" "}
        <Box paddingX={1}>
                   {" "}
          <Text bold color="red">
                        Unable to switch model          {" "}
          </Text>
                 {" "}
        </Box>
               {" "}
        <Box paddingX={1}>
                   {" "}
          <Text>
                        You can only pick a model before the assistant sends its
            first             response. To use a different model please start a
            new chat.          {" "}
          </Text>
                 {" "}
        </Box>
               {" "}
        <Box paddingX={1}>
                    <Text dimColor>press esc or enter to close</Text>     
           {" "}
        </Box>
             {" "}
      </Box>
    );
  }

  return (
    <TypeaheadOverlay
      title="Switch model"
      description={
        <Text>
                    Current model:{" "}
          <Text color="greenBright">{currentModel}</Text>       {" "}
        </Text>
      }
      initialItems={items} // Use the items derived from availableModels prop
      currentValue={currentModel}
      onSelect={onSelect} // Pass the selected model up to TerminalChat
      onExit={onExit}
      // Consider adding filtering/disabling in TypeaheadOverlay itself based on availableModels
      // if TypeaheadOverlay supports it, for better UX. Or handle validation entirely in TerminalChat's onSelect handler.
    />
  );
}
