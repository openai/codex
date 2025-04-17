import figures from "figures";
import { Box, Text } from "ink";
import React from "react";

export type Props = {
  readonly isSelected?: boolean;
};

function Indicator({ isSelected = false }: Props): JSX.Element {
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
