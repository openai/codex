import SelectInput from "./select-input/select-input.js";
import TextInput from "./vendor/ink-text-input.js";
import { Box, Text, useInput } from "ink";
import React, { useState } from "react";

export type TypeaheadItem = { label: string; value: string };

type Props = {
  title: string;
  description?: React.ReactNode;
  initialItems: Array<TypeaheadItem>;
  currentValue?: string;
  limit?: number;
  onSelect: (value: string) => void;
  onExit: () => void;
};

/**
 * Generic overlay that combines a TextInput with a filtered SelectInput.
 * It is intentionally dependency‑free so it can be re‑used by multiple
 * overlays (model picker, command picker, …).
 */
export default function TypeaheadOverlay({
  title,
  description,
  initialItems,
  currentValue,
  limit = 10,
  onSelect,
  onExit,
}: Props): JSX.Element {
  const [value, setValue] = useState("");
  const [items, setItems] = useState<Array<TypeaheadItem>>(initialItems);

  // Keep internal items list in sync when the caller provides new options
  // (e.g. ModelOverlay fetches models asynchronously).
  React.useEffect(() => {
    setItems(initialItems);
  }, [initialItems]);

  /* ------------------------------------------------------------------ */
  /* Exit on ESC                                                         */
  /* ------------------------------------------------------------------ */
  useInput((_input, key) => {
    if (key.escape) {
      onExit();
    }
  });

  /* ------------------------------------------------------------------ */
  /* Filtering & Ranking                                                 */
  /* ------------------------------------------------------------------ */
  const q = value.toLowerCase();
  const filtered =
    q.length === 0
      ? items
      : items.filter((i) => i.label.toLowerCase().includes(q));

  /*
   * Sort logic:
   *   1. Keep the currently‑selected value at the very top so switching back
   *      to it is always a single <enter> press away.
   *   2. When the user has not typed anything yet (q === ""), keep the
   *      original order provided by `initialItems`.  This allows callers to
   *      surface a hand‑picked list of recommended / frequently‑used options
   *      at the top while still falling back to a deterministic alphabetical
   *      order for the rest of the list (they can simply pre‑sort the array
   *      before passing it in).
   *   3. As soon as the user starts typing we revert to the previous ranking
   *      mechanism that tries to put the best match first and then sorts the
   *      remainder alphabetically.
   */

  const ranked = filtered.sort((a, b) => {
    if (a.value === currentValue) {
      return -1;
    }
    if (b.value === currentValue) {
      return 1;
    }

    // Preserve original order when no query is present so we keep any caller
    // defined prioritisation (e.g. recommended models).
    if (q.length === 0) {
      return 0;
    }

    const ia = a.label.toLowerCase().indexOf(q);
    const ib = b.label.toLowerCase().indexOf(q);
    if (ia !== ib) {
      return ia - ib;
    }
    return a.label.localeCompare(b.label);
  });

  const selectItems = ranked;

  if (
    process.env["DEBUG_TYPEAHEAD"] === "1" ||
    process.env["DEBUG_TYPEAHEAD"] === "true"
  ) {
    // eslint-disable-next-line no-console
    console.log(
      "[TypeaheadOverlay] value=",
      value,
      "items=",
      items.length,
      "visible=",
      selectItems.map((i) => i.label),
    );
  }
  const initialIndex = selectItems.findIndex((i) => i.value === currentValue);

  return (
    <Box
      flexDirection="column"
      borderStyle="round"
      borderColor="gray"
      width={80}
    >
      <Box paddingX={1}>
        <Text bold>{title}</Text>
      </Box>

      <Box flexDirection="column" paddingX={1} gap={1}>
        {description}
        <TextInput
          value={value}
          onChange={setValue}
          onSubmit={(submitted) => {
            // If there are items in the SelectInput, let its onSelect handle the submission.
            // Only submit from TextInput if the list is empty.
            if (selectItems.length === 0) {
              const target = submitted.trim();
              if (target) {
                onSelect(target);
              } else {
                // If submitted value is empty and list is empty, just exit.
                onExit();
              }
            }
            // If selectItems.length > 0, do nothing here; SelectInput's onSelect will handle Enter.
          }}
        />
        {selectItems.length > 0 && (
          <SelectInput
            limit={limit}
            items={selectItems}
            initialIndex={initialIndex === -1 ? 0 : initialIndex}
            isFocused
            onSelect={(item: TypeaheadItem) => {
              if (item.value) {
                onSelect(item.value);
              }
            }}
          />
        )}
      </Box>

      <Box paddingX={1}>
        {/* Slightly more verbose footer to make the search behaviour crystal‑clear */}
        <Text dimColor>type to search · enter to confirm · esc to cancel</Text>
      </Box>
    </Box>
  );
}
