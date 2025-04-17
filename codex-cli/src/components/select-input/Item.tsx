import * as React from "react";
import { Text } from "ink";

export type Props = {
  readonly isSelected?: boolean;
  readonly label: string;
};

function Item({ isSelected = false, label }: Props) {
  return <Text color={isSelected ? "blue" : undefined}>{label}</Text>;
}

export default Item;
