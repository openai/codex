import TypeaheadOverlay from "./typeahead-overlay.js";
import {
  getAvailableModels,
  RECOMMENDED_MODELS,
} from "../utils/model-utils.js";
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
  onSelect: (model: string) => void;
  onExit: () => void;
};

export default function ModelOverlay({
  currentModel,
  hasLastResponse,
  onSelect,
  onExit,
}: Props): JSX.Element {
  // Group models into local recommended and OpenAI‑fetched lists
  const OPENAI_GROUP_VALUE = '__OPENAI_GROUP__';
  const BACK_VALUE = '__BACK__';
  const [recommendedModels, setRecommendedModels] = useState<string[]>([]);
  const [openaiModels, setOpenaiModels] = useState<string[]>([]);
  const [view, setView] = useState<'root' | 'openai'>('root');

  useEffect(() => {
    (async () => {
      const models = await getAvailableModels();
      // Split the list into recommended and “other” (OpenAI) models
      const recommended = RECOMMENDED_MODELS.filter((m) => models.includes(m));
      const others = models.filter((m) => !recommended.includes(m));
      setRecommendedModels(recommended);
      setOpenaiModels(others);
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
        <Box paddingX={1}>
          <Text bold color="red">
            Unable to switch model
          </Text>
        </Box>
        <Box paddingX={1}>
          <Text>
            You can only pick a model before the assistant sends its first
            response. To use a different model please start a new chat.
          </Text>
        </Box>
        <Box paddingX={1}>
          <Text dimColor>press esc or enter to close</Text>
        </Box>
      </Box>
    );
  }
  // Prepare items for the typeahead based on the current view (root or OpenAI group)
  const initialItems = (() => {
    if (view === 'root') {
      const items = recommendedModels.map((m) => ({ label: `⭐ ${m}`, value: m }));
      if (openaiModels.length > 0) {
        items.push({ label: 'OpenAI', value: OPENAI_GROUP_VALUE });
      }
      return items;
    } else {
      return [
        { label: '← Back', value: BACK_VALUE },
        ...openaiModels.map((m) => ({ label: m, value: m })),
      ];
    }
  })();

  const handleSelect = (value: string) => {
    if (view === 'root' && value === OPENAI_GROUP_VALUE) {
      setView('openai');
    } else if (view === 'openai' && value === BACK_VALUE) {
      setView('root');
    } else {
      onSelect(value);
    }
  };

  return (
    <TypeaheadOverlay
      key={view}
      title="Switch model"
      description={
        <Text>
          Current model: <Text color="greenBright">{currentModel}</Text>
        </Text>
      }
      initialItems={initialItems}
      currentValue={currentModel}
      onSelect={handleSelect}
      onExit={onExit}
    />
  );
}
