import React from "react";
import { Box, Text } from "ink";
import figures from "figures";
import { styles } from "./theme";
export function SelectOption({ isFocused, isSelected, children }) {
  return React.createElement(
    Box,
    { ...styles.option({ isFocused }) },
    isFocused &&
      React.createElement(
        Text,
        { ...styles.focusIndicator() },
        figures.pointer,
      ),
    React.createElement(
      Text,
      { ...styles.label({ isFocused, isSelected }) },
      children,
    ),
    isSelected &&
      React.createElement(
        Text,
        { ...styles.selectedIndicator() },
        figures.tick,
      ),
  );
}
