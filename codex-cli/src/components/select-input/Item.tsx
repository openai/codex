import { Text } from "ink";
import * as React from "react";

export type Props = {
  readonly isSelected?: boolean;
  readonly label: string;
};

function Item({ isSelected = false, label }: Props): JSX.Element {
  return <Text color={isSelected ? "blue" : undefined}>{label}</Text>;
}

export default Item;
