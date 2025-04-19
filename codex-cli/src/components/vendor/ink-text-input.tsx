import React, { useState } from "react";
import { Text, useInput } from "ink";
import type { Except } from "type-fest";
import { useDefaultInputHandler } from "./defaultInputHandler";
import { TextInputProps } from "./input-handlers";

function TextInput(props: TextInputProps) {
  const { focus = true } = props;

  const { handler, output } = useDefaultInputHandler(props);

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
