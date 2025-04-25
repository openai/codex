import { Text } from "ink";
import React, { useState } from "react";
import { useInterval } from "use-interval";

const spinnerTypes: Record<string, string[]> = {
  dots: ["⢎ ", "⠎⠁", "⠊⠑", "⠈⠱", " ⡱", "⢀⡰", "⢄⡠", "⢆⡀"],
  ball: [
    "( ●    )",
    "(  ●   )",
    "(   ●  )",
    "(    ● )",
    "(     ●)",
    "(    ● )",
    "(   ●  )",
    "(  ●   )",
    "( ●    )",
    "(●     )",
  ],
};

export default function Spinner({
  type = "dots",
}: {
  type?: string;
}): JSX.Element {
  const frames = spinnerTypes[type || "dots"] || [];
  const interval = 80;
  const [frame, setFrame] = useState(0);
  useInterval(() => {
    setFrame((previousFrame) => {
      const isLastFrame = previousFrame === frames.length - 1;
      return isLastFrame ? 0 : previousFrame + 1;
    });
  }, interval);
  return <Text>{frames[frame]}</Text>;
}
