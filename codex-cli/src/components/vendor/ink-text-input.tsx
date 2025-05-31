import React, { useState } from "react";
import { Text, useInput } from "ink";
import type { Except } from "type-fest";
import { CursorState, TextInputProps } from "./input-handlers";
import { useVimInputHandler } from "./vimInputHandler";

function TextInput(props: TextInputProps) {
  const { focus = true } = props;
  const cursorState = useState<CursorState>({
    cursorOffset: (props.value ?? "").length,
    cursorWidth: 0,
  });

  const { handler, output } = useVimInputHandler({ ...props, cursorState });

  useInput(handler, { isActive: focus });

  return <Text> {output} </Text>;
}

export default TextInput;

type UncontrolledProps = {
  readonly initialValue?: string;
} & Except<TextInputProps, "value" | "onChange">;

export function UncontrolledTextInput({
  initialValue = "",
  ...props
}: UncontrolledProps) {
  const [value, setValue] = useState(initialValue);

  return <TextInput {...props} value={value} onChange={setValue} />;
}
