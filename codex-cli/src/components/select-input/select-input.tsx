import Indicator, { type Props as IndicatorProps } from "./indicator.js";
import ItemComponent, { type Props as ItemProps } from "./item.js";
import isEqual from "fast-deep-equal";
import { Box, useInput } from "ink";
import React, {
  type FC,
  useState,
  useEffect,
  useRef,
  useCallback,
} from "react";
import arrayToRotated from "to-rotated";

type Props<V> = {
  /**
   * Items to display in a list. Each item must be an object and have `label` and `value` props, it may also optionally have a `key` prop.
   * If no `key` prop is provided, `value` will be used as the item key.
   */
  readonly items?: Array<Item<V>>;

  /**
   * Listen to user's input. Useful in case there are multiple input components at the same time and input must be "routed" to a specific component.
   *
   * @default true
   */
  readonly isFocused?: boolean;

  /**
   * Index of initially-selected item in `items` array.
   *
   * @default 0
   */
  readonly initialIndex?: number;

  /**
   * Number of items to display.
   */
  readonly limit?: number;

  /**
   * Custom component to override the default indicator component.
   */
  readonly indicatorComponent?: FC<IndicatorProps>;

  /**
   * Custom component to override the default item component.
   */
  readonly itemComponent?: FC<ItemProps>;

  /**
   * Function to call when user selects an item. Item object is passed to that function as an argument.
   */
  readonly onSelect?: (item: Item<V>) => void;

  /**
   * Function to call when user highlights an item. Item object is passed to that function as an argument.
   */
  readonly onHighlight?: (item: Item<V>) => void;
};

export type Item<V> = {
  key?: string;
  label: string;
  value: V;
};

function SelectInput<V>({
  items = [],
  isFocused = true,
  initialIndex = 0,
  indicatorComponent = Indicator,
  itemComponent = ItemComponent,
  limit: customLimit,
  onSelect,
  onHighlight,
}: Props<V>): JSX.Element {
  const hasLimit =
    typeof customLimit === "number" && items.length > customLimit;
  const limit = hasLimit ? Math.min(customLimit, items.length) : items.length;
  const lastIndex = limit - 1;
  const [rotateIndex, setRotateIndex] = useState(
    initialIndex > lastIndex ? lastIndex - initialIndex : 0,
  );
  const [selectedIndex, setSelectedIndex] = useState(
    initialIndex ? (initialIndex > lastIndex ? lastIndex : initialIndex) : 0,
  );
  const previousItems = useRef<Array<Item<V>>>(items);

  useEffect(() => {
    if (
      !isEqual(
        previousItems.current.map((item) => item.value),
        items.map((item) => item.value),
      )
    ) {
      setRotateIndex(0);
      setSelectedIndex(0);
    }

    previousItems.current = items;
  }, [items]);

  useInput(
    useCallback(
      (input, key) => {
        if (input === "k" || key.upArrow) {
          const lastIndex = (hasLimit ? limit : items.length) - 1;
          const atFirstIndex = selectedIndex === 0;
          const nextIndex = hasLimit ? selectedIndex : lastIndex;
          const nextRotateIndex = atFirstIndex ? rotateIndex + 1 : rotateIndex;
          const nextSelectedIndex = atFirstIndex
            ? nextIndex
            : selectedIndex - 1;

          setRotateIndex(nextRotateIndex);
          setSelectedIndex(nextSelectedIndex);

          const slicedItems = hasLimit
            ? arrayToRotated(items, nextRotateIndex).slice(0, limit)
            : items;

          if (typeof onHighlight === "function") {
            onHighlight(slicedItems[nextSelectedIndex]!);
          }
        }

        if (input === "j" || key.downArrow) {
          const atLastIndex =
            selectedIndex === (hasLimit ? limit : items.length) - 1;
          const nextIndex = hasLimit ? selectedIndex : 0;
          const nextRotateIndex = atLastIndex ? rotateIndex - 1 : rotateIndex;
          const nextSelectedIndex = atLastIndex ? nextIndex : selectedIndex + 1;

          setRotateIndex(nextRotateIndex);
          setSelectedIndex(nextSelectedIndex);

          const slicedItems = hasLimit
            ? arrayToRotated(items, nextRotateIndex).slice(0, limit)
            : items;

          if (typeof onHighlight === "function") {
            onHighlight(slicedItems[nextSelectedIndex]!);
          }
        }

        if (key.return) {
          const slicedItems = hasLimit
            ? arrayToRotated(items, rotateIndex).slice(0, limit)
            : items;

          if (typeof onSelect === "function") {
            onSelect(slicedItems[selectedIndex]!);
          }
        }
      },
      [
        hasLimit,
        limit,
        rotateIndex,
        selectedIndex,
        items,
        onSelect,
        onHighlight,
      ],
    ),
    { isActive: isFocused },
  );

  const slicedItems = hasLimit
    ? arrayToRotated(items, rotateIndex).slice(0, limit)
    : items;

  return (
    <Box flexDirection="column">
      {slicedItems.map((item, index) => {
        const isSelected = index === selectedIndex;

        return (
          <Box key={item.key ?? String(item.value)}>
            {React.createElement(indicatorComponent, { isSelected })}
            {React.createElement(itemComponent, { ...item, isSelected })}
          </Box>
        );
      })}
    </Box>
  );
}

export default SelectInput;
