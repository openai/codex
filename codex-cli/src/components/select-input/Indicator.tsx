import React from "react";
import { Box, Text } from "ink";
import figures from "figures";

export type Props = {
  readonly isSelected?: boolean;
};

function Indicator({ isSelected = false }: Props) {
  return (
    <Box marginRight={1}>
      {isSelected ? (
        <Text color="blue">{figures.pointer}</Text>
      ) : (
        <Text> </Text>
      )}
    </Box>
  );
}

export default Indicator;
