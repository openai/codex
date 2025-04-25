import { isDeepStrictEqual } from "node:util";
import { useReducer, useCallback, useMemo, useState, useEffect } from "react";
import OptionMap from "./option-map";
const reducer = (state, action) => {
  switch (action.type) {
    case "focus-next-option": {
      if (!state.focusedValue) {
        return state;
      }
      const item = state.optionMap.get(state.focusedValue);
      if (!item) {
        return state;
      }
      // eslint-disable-next-line prefer-destructuring
      const next = item.next;
      if (!next) {
        return state;
      }
      const needsToScroll = next.index >= state.visibleToIndex;
      if (!needsToScroll) {
        return {
          ...state,
          focusedValue: next.value,
        };
      }
      const nextVisibleToIndex = Math.min(
        state.optionMap.size,
        state.visibleToIndex + 1,
      );
      const nextVisibleFromIndex =
        nextVisibleToIndex - state.visibleOptionCount;
      return {
        ...state,
        focusedValue: next.value,
        visibleFromIndex: nextVisibleFromIndex,
        visibleToIndex: nextVisibleToIndex,
      };
    }
    case "focus-previous-option": {
      if (!state.focusedValue) {
        return state;
      }
      const item = state.optionMap.get(state.focusedValue);
      if (!item) {
        return state;
      }
      // eslint-disable-next-line prefer-destructuring
      const previous = item.previous;
      if (!previous) {
        return state;
      }
      const needsToScroll = previous.index <= state.visibleFromIndex;
      if (!needsToScroll) {
        return {
          ...state,
          focusedValue: previous.value,
        };
      }
      const nextVisibleFromIndex = Math.max(0, state.visibleFromIndex - 1);
      const nextVisibleToIndex =
        nextVisibleFromIndex + state.visibleOptionCount;
      return {
        ...state,
        focusedValue: previous.value,
        visibleFromIndex: nextVisibleFromIndex,
        visibleToIndex: nextVisibleToIndex,
      };
    }
    case "select-focused-option": {
      return {
        ...state,
        previousValue: state.value,
        value: state.focusedValue,
      };
    }
    case "reset": {
      return action.state;
    }
  }
};
const createDefaultState = ({
  visibleOptionCount: customVisibleOptionCount,
  defaultValue,
  options,
}) => {
  const visibleOptionCount =
    typeof customVisibleOptionCount === "number"
      ? Math.min(customVisibleOptionCount, options.length)
      : options.length;
  const optionMap = new OptionMap(options);
  return {
    optionMap,
    visibleOptionCount,
    focusedValue: optionMap.first?.value,
    visibleFromIndex: 0,
    visibleToIndex: visibleOptionCount,
    previousValue: defaultValue,
    value: defaultValue,
  };
};
export const useSelectState = ({
  visibleOptionCount = 5,
  options,
  defaultValue,
  onChange,
}) => {
  const [state, dispatch] = useReducer(
    reducer,
    { visibleOptionCount, defaultValue, options },
    createDefaultState,
  );
  const [lastOptions, setLastOptions] = useState(options);
  if (options !== lastOptions && !isDeepStrictEqual(options, lastOptions)) {
    dispatch({
      type: "reset",
      state: createDefaultState({ visibleOptionCount, defaultValue, options }),
    });
    setLastOptions(options);
  }
  const focusNextOption = useCallback(() => {
    dispatch({
      type: "focus-next-option",
    });
  }, []);
  const focusPreviousOption = useCallback(() => {
    dispatch({
      type: "focus-previous-option",
    });
  }, []);
  const selectFocusedOption = useCallback(() => {
    dispatch({
      type: "select-focused-option",
    });
  }, []);
  const visibleOptions = useMemo(() => {
    return options
      .map((option, index) => ({
        ...option,
        index,
      }))
      .slice(state.visibleFromIndex, state.visibleToIndex);
  }, [options, state.visibleFromIndex, state.visibleToIndex]);
  useEffect(() => {
    if (state.value && state.previousValue !== state.value) {
      onChange?.(state.value);
    }
  }, [state.previousValue, state.value, options, onChange]);
  return {
    focusedValue: state.focusedValue,
    visibleFromIndex: state.visibleFromIndex,
    visibleToIndex: state.visibleToIndex,
    value: state.value,
    visibleOptions,
    focusNextOption,
    focusPreviousOption,
    selectFocusedOption,
  };
};
